use std::{cell::UnsafeCell, sync::LazyLock};
use std::{i32, usize};

use rustc_hash::FxHashMap;

use crate::indexing::{SEDIndex, SEDIndexWithStructure};

pub fn sed(t1: &SEDIndex, t2: &SEDIndex) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.preorder.len() > t2.preorder.len() {
        (t1, t2) = (t2, t1);
    }

    let pre_dist = string_edit_distance(&t1.preorder, &t2.preorder);
    let post_dist = string_edit_distance(&t1.postorder, &t2.postorder);

    std::cmp::max(pre_dist, post_dist)
}

/// Implements fastest known way to compute exact string edit between two strings
fn string_edit_distance(s1: &[i32], s2: &[i32]) -> usize {
    use std::cmp::min;
    // assumes size of s2 is smaller or equal than s1
    let s2len = s2.len();
    let mut cache: Vec<usize> = (1..s2len + 1).collect();
    let mut result = s2len;
    for (i, ca) in s1.iter().enumerate() {
        let mut dist_b = i;
        result = i + 1;

        for (j, cb) in s2.iter().enumerate() {
            let dist_a = dist_b + usize::from(ca != cb);
            unsafe {
                dist_b = *cache.get_unchecked(j);
                result = min(result + 1, min(dist_a, dist_b + 1));
                *cache.get_unchecked_mut(j) = result;
            }
        }
    }

    result
}

pub struct BerghelRoachSed<'a, T: Eq> {
    fkp_matrix: Vec<i32>,
    max_diag: i32,
    // must be + 2 to accommodate for "virtual -1" and 0 cost
    max_cost: i32,
    max_computable_cost: i32,
    zero_diagonal_offset: i32,
    /// Column-major stride: number of cost entries per diagonal column.
    /// Equals max_cost + 2 (cost range: -1..=max_cost).
    cost_stride: i32,
    query: &'a [T],
}

// Use a power-of-two alignment (16 elements = 64 bytes)
const ALIGNMENT: i32 = 16;

impl<'a, T: Eq> BerghelRoachSed<'a, T> {
    pub fn initialize_query(query: &'a [T], threshold: i32) -> Self {
        // initialize the control structure for the query
        // our MAX_P (or max cost) is threshold + 2
        // +2 to accommodate for the "virtual -1" edit and the edit cost +1 where the algorithm ends
        let max_p = threshold + 2;

        // since we have strict threshold on the edit distance, we only need to consider diagonals in the range of
        // [-threshold, threshold] plus the diagonal for zero edits, plus one extra at each end for the case when we exceed k
        // that is why we add + 3
        let max_k = threshold * 2 + 3;

        // zero_k offset (or the 0th diagonal offset) is the index in the FROW array where the diagonal for zero edits is stored
        let zero_k_offset = max_k / 2;

        let cost_stride = max_p + 2;
        let fkp_matrix = Self::initialize_fkp_matrix(zero_k_offset, max_k, max_p);

        Self {
            query,
            fkp_matrix,
            max_computable_cost: threshold,
            max_diag: max_k,
            max_cost: max_p,
            zero_diagonal_offset: zero_k_offset,
            cost_stride,
        }
    }

    pub fn reinitialize_query(&mut self, query: &'a [T], threshold: i32) {
        if self.max_cost >= threshold + 2 && self.max_diag >= threshold * 2 + 3 {
            self.query = query;
            self.max_computable_cost = threshold;
            return;
        }

        *self = Self::initialize_query(query, threshold);
    }

    pub fn compute_distance(&mut self, target: &[T]) -> usize {
        let mut s1 = self.query;
        let mut s2 = target;

        if s1.len() >= s2.len() {
            (s1, s2) = (s2, s1);
        }

        let m = s1.len() as i32;
        let n = s2.len() as i32;
        let cs = self.cost_stride as usize;
        let zero_diagonal_offset = self.zero_diagonal_offset;

        let target_diagonal = n - m;

        if target_diagonal > self.max_computable_cost {
            return usize::MAX;
        }

        // Column-major layout: each diagonal's cost values are stored contiguously.
        // f(k, p) is at index: (k + zero_offset) * cost_stride + (p + 1)
        let mut greedy_extend = |diag: i32, cost: i32, matrix: &mut Vec<i32>| {
            use std::cmp::max;

            let diag_off = (diag + zero_diagonal_offset) as usize;
            // Index for f(k, cost-1): offset = (cost - 1) + 1 = cost
            let prev_cost = cost as usize;

            let mut max_row = unsafe {
                max(
                    *matrix.get_unchecked(diag_off * cs + prev_cost) + 1, // f(k, p-1) + 1: substitution
                    max(
                        *matrix.get_unchecked((diag_off - 1) * cs + prev_cost), // f(k-1, p-1): deletion
                        *matrix.get_unchecked((diag_off + 1) * cs + prev_cost) + 1, // f(k+1, p-1) + 1: insertion
                    ),
                )
            };

            // Greedy extension along the diagonal (Ukkonen's optimization)
            unsafe {
                while max_row >= 0
                    && max_row < m
                    && max_row + diag < n
                    && s1.get_unchecked(max_row as usize)
                        == s2.get_unchecked((max_row + diag) as usize)
                {
                    max_row += 1;
                }
            }

            // Write f(k, p) at (diag_off * cs + (cost + 1))
            unsafe {
                *matrix.get_unchecked_mut(diag_off * cs + (cost + 1) as usize) = max_row;
            }
        };

        let mut cost = target_diagonal;
        let fkp_matrix = &mut self.fkp_matrix;

        loop {
            // Afterword alternative from Berghel & Roach:
            // Directly enumerate (diagonal, cost) pairs radiating outward from target_diagonal.
            // Right of target diagonal: delta + i at cost p - i
            for i in (1..=(cost - target_diagonal) / 2).rev() {
                greedy_extend(target_diagonal + i, cost - i, fkp_matrix);
            }
            // Left of target diagonal: delta - i at cost p - i
            for i in (1..=(cost + target_diagonal) / 2).rev() {
                greedy_extend(target_diagonal - i, cost - i, fkp_matrix);
            }

            greedy_extend(target_diagonal, cost, fkp_matrix);
            cost += 1;

            // Check f(target_diagonal, cost - 1)
            let check_idx = (target_diagonal + zero_diagonal_offset) as usize * cs + cost as usize;

            unsafe {
                if *fkp_matrix.get_unchecked(check_idx) == m {
                    // println!("Total FKP matrix accesses: {}", fkp_write_accesses);
                    return (cost - 1) as usize;
                } else if cost > self.max_computable_cost {
                    // println!("Total FKP matrix accesses: {}", fkp_write_accesses);

                    return usize::MAX;
                }
            }
        }
    }

    fn get_max_diag(size_diff: i32) -> i32 {
        size_diff * 2 + 3
    }

    /// Column-major access: f(k, p) = matrix[(k + zero_offset) * cost_stride + (p + 1)]
    #[inline(always)]
    fn access_fkp_matrix(
        cost: i32,
        diag: i32,
        cost_stride: i32,
        zero_diagonal_offset: i32,
    ) -> usize {
        ((diag + zero_diagonal_offset) * cost_stride + (cost + 1)) as usize
    }

    #[inline(always)]
    fn fkp_matrix_at(&self, cost: i32, diag: i32) -> usize {
        ((diag + self.zero_diagonal_offset) * self.cost_stride + (cost + 1)) as usize
    }

    /// Initialize the FKP matrix in column-major layout.
    /// Each diagonal's cost entries are stored contiguously.
    fn initialize_fkp_matrix(zero_diagonal_offset: i32, max_diag: i32, max_cost: i32) -> Vec<i32> {
        let cost_stride = max_cost + 2;
        let mut matrix = vec![i32::MIN; ((max_diag + 1) * cost_stride) as usize];
        for diag in -zero_diagonal_offset..(max_diag - zero_diagonal_offset) {
            let diag_base = ((diag + zero_diagonal_offset) * cost_stride) as usize;
            for cost in -1..(max_cost + 1) {
                if cost == diag.abs() - 1 {
                    let idx = diag_base + (cost + 1) as usize;
                    matrix[idx] = if diag < 0 { diag.abs() - 1 } else { -1 };
                }
            }
        }
        matrix
    }
}

pub struct BerghelRoachSedStruct<'a> {
    fkp_matrix: Vec<(i32, bool)>,
    max_diag: i32,
    // must be + 2 to accommodate for "virtual -1" and 0 cost
    max_cost: i32,
    max_computable_cost: i32,
    zero_diagonal_offset: i32,
    query: &'a [TraversalCharacter],
}

impl<'a> BerghelRoachSedStruct<'a> {
    pub fn reinitialize_query(&mut self, query: &'a [TraversalCharacter], threshold: i32) {
        if self.max_cost >= threshold + 2 && self.max_diag >= threshold * 2 + 3 {
            self.query = query;
            self.max_computable_cost = threshold;
            return;
        }

        *self = Self::initialize_query(query, threshold);
    }

    pub fn compute_distance(&mut self, target: &[TraversalCharacter]) -> usize {
        let mut s1 = self.query;
        let mut s2 = target;

        if s1.len() >= s2.len() {
            (s1, s2) = (s2, s1);
        }

        let m = s1.len() as i32;
        let n = s2.len() as i32;
        let max_diag = self.max_diag;
        let zero_diagonal_offset = self.zero_diagonal_offset;

        let target_diagonal = n - m;
        let threshold = self.max_computable_cost;

        if target_diagonal > self.max_computable_cost {
            return usize::MAX;
        }

        let greedy_extend = |diag: i32, cost: i32, matrix: &mut Vec<(i32, bool)>| {
            use std::cmp::max;

            let previous_row = &matrix[Self::access_fkp_matrix(
                cost - 1,
                -zero_diagonal_offset,
                max_diag,
                zero_diagonal_offset,
            )
                ..Self::access_fkp_matrix(
                    cost - 1,
                    max_diag - zero_diagonal_offset,
                    max_diag,
                    zero_diagonal_offset,
                )];

            let offset_diag = (diag + self.zero_diagonal_offset) as usize;
            let mut struct_ok = false;

            let mut max_row = unsafe {
                max(
                    previous_row.get_unchecked(offset_diag).0 + 1, // substitution
                    max(
                        previous_row.get_unchecked(offset_diag - 1).0, // deletion
                        previous_row.get_unchecked(offset_diag + 1).0 + 1, // insertion
                    ),
                )
            };
            let allowed_edits = cost - 1;

            // While loop to extend the match (Ukkonen's optimization)
            // Added safe bounds check (t >= 0) just in case initialization used -999
            unsafe {
                while max_row < m && max_row + diag < n {
                    let c1 = s1.get_unchecked(max_row as usize);
                    let c2 = s2.get_unchecked((max_row + diag) as usize);

                    let char_eq = c1.char == c2.char;
                    struct_ok = (allowed_edits + (c1.sum - c2.sum).abs() <= threshold)
                        && (allowed_edits + (c1.diff - c2.diff).abs() <= threshold);

                    if !char_eq || !struct_ok {
                        break;
                    }
                    max_row += 1;
                }
            }

            //
            unsafe {
                *matrix.get_unchecked_mut(Self::access_fkp_matrix(
                    cost,
                    diag,
                    max_diag,
                    zero_diagonal_offset,
                )) = (max_row, struct_ok);
            }
        };

        let mut cost = target_diagonal;
        let fkp_matrix = &mut self.fkp_matrix;

        loop {
            let mut inc = cost;

            for temp_cost in 0..cost {
                if ((n - m) - inc).abs() <= temp_cost {
                    greedy_extend((n - m) - inc, temp_cost, fkp_matrix);
                }
                if ((n - m) + inc).abs() <= temp_cost {
                    greedy_extend((n - m) + inc, temp_cost, fkp_matrix);
                }

                inc -= 1;
            }

            greedy_extend((n - m), cost, fkp_matrix);
            cost += 1;

            // print matrix row by row

            // eprintln!("FKP Matrix:");
            // for (idx, val) in fkp_matrix.iter().enumerate() {
            //     eprint!("{:>12} ", val);

            //     if idx % (max_diag + 1) as usize == max_diag as usize {
            //         eprintln!("");
            //     }
            // }

            // eprintln!("");

            let current_target_diag_idx =
                Self::access_fkp_matrix(cost - 1, target_diagonal, max_diag, zero_diagonal_offset);

            unsafe {
                if fkp_matrix.get_unchecked(current_target_diag_idx).0 == m {
                    return (cost - 1) as usize;
                } else if cost > self.max_computable_cost {
                    return usize::MAX;
                }
            }
        }
    }

    fn get_max_diag(size_diff: i32) -> i32 {
        size_diff * 2 + 3
    }

    fn fkp_matrix_access(&self, row: usize, col: usize, cols: usize) -> (i32, bool) {
        self.fkp_matrix[row * cols + col]
    }

    #[inline(always)]
    fn access_fkp_matrix(cost: i32, diag: i32, max_diag: i32, zero_diagonal_offset: i32) -> usize {
        ((cost + 1) * (max_diag + 1) + diag + zero_diagonal_offset) as usize
    }

    #[inline(always)]
    fn fkp_matrix_at(&self, cost: i32, diag: i32) -> usize {
        ((cost + 1) * (self.max_diag + 1) + diag + self.zero_diagonal_offset) as usize
    }

    fn initialize_fkp_matrix(
        zero_diagonal_offset: i32,
        max_diag: i32,
        max_cost: i32,
    ) -> Vec<(i32, bool)> {
        let mut matrix = vec![(i32::MIN, true); ((max_diag + 1) * (max_cost + 2)) as usize];
        for diag in -zero_diagonal_offset..(max_diag - zero_diagonal_offset) {
            for cost in -1..(max_cost + 1) {
                if cost == diag.abs() - 1 {
                    if diag < 0 {
                        matrix
                            [Self::access_fkp_matrix(cost, diag, max_diag, zero_diagonal_offset)] =
                            (diag.abs() - 1, true);
                    } else {
                        matrix
                            [Self::access_fkp_matrix(cost, diag, max_diag, zero_diagonal_offset)] =
                            (-1, true);
                    }
                } else {
                    matrix[Self::access_fkp_matrix(cost, diag, max_diag, zero_diagonal_offset)] =
                        (i32::MIN, true);
                }
            }
        }

        matrix
    }

    pub fn initialize_query(query: &'a [TraversalCharacter], threshold: i32) -> Self {
        // initialize the control structure for the query
        // our MAX_P (or max cost) is threshold + 2
        // +2 to accommodate for the "virtual -1" edit and the edit cost +1 where the algorithm ends
        let max_p = threshold + 2;

        // since we have strict threshold on the edit distance, we only need to consider diagonals in the range of
        // [-threshold, threshold] plus the diagonal for zero edits, plus one extra at each end for the case when we exceed k
        // that is why we add + 3
        let max_k = threshold * 2 + 3;

        // zero_k offset (or the 0th diagonal offset) is the index in the FROW array where the diagonal for zero edits is stored
        let zero_k_offset = max_k / 2;

        let fkp_matrix = Self::initialize_fkp_matrix(zero_k_offset, max_k, max_p);

        Self {
            query,
            fkp_matrix,
            max_computable_cost: threshold,
            max_diag: max_k,
            max_cost: max_p,
            zero_diagonal_offset: zero_k_offset,
        }
    }
}

struct SEDParameters {
    target_diagonal: usize,
    threshold: usize,
    offset_0th_diagonal: usize,
    array_size: usize,
}

fn compute_sed_parameters(s1_len: &usize, s2_len: &usize, k: &usize) -> SEDParameters {
    let size_diff = s2_len - s1_len;
    // Per Berghel & Roach, the threshold is the min of s2 length and k
    let threshold = *std::cmp::min(s2_len, k);

    // The target diagonal where we need to reach end of s1
    let target_diagonal = size_diff;
    // The offset for indexing the diagonals in the FROW array, which allows us to handle negative indices
    // This is also referred to as ZERO_K in the Berghel & Roach paper, as it represents the diagonal corresponding to zero edits
    let offset_0th_diagonal = threshold + 1;

    // the maximum number of diagonals we need to consider
    // is 2*k (-k to +k) plus the diagonal for zero edits, plus one extra at each end for the case when we exceed k
    // that is why we add + 3
    let array_size = (2 * threshold + 3) as usize;

    SEDParameters {
        target_diagonal: size_diff,
        threshold,
        offset_0th_diagonal,
        array_size,
    }
}

macro_rules! prepare_sed_inputs {
    ($t1:expr, $t2:expr, $k:expr) => {{
        let (mut t1, mut t2) = ($t1, $t2);
        if t1.preorder.len() > t2.preorder.len() {
            (t1, t2) = (t2, t1);
        }
        let params = compute_sed_parameters(&t1.c.tree_size, &t2.c.tree_size, &$k);
        (t1, t2, params)
    }};
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalOrder {
    Preorder,
    Postorder,
    ReversedPreorder,
    ReversedPostorder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SedTraversalConfig {
    pub first: TraversalOrder,
    pub second: TraversalOrder,
}

impl Default for SedTraversalConfig {
    fn default() -> Self {
        Self {
            first: TraversalOrder::ReversedPreorder,
            second: TraversalOrder::Preorder,
        }
    }
}

#[inline(always)]
fn select_traversal_i32<'a>(index: &'a SEDIndex, order: TraversalOrder) -> &'a [i32] {
    match order {
        TraversalOrder::Preorder => &index.preorder,
        TraversalOrder::Postorder => &index.postorder,
        TraversalOrder::ReversedPreorder => &index.reversed_preorder,
        TraversalOrder::ReversedPostorder => &index.reversed_postorder,
    }
}

#[inline(always)]
fn select_traversal_struct<'a>(
    index: &'a SEDIndexWithStructure,
    order: TraversalOrder,
) -> &'a [TraversalCharacter] {
    match order {
        TraversalOrder::Preorder => &index.preorder,
        TraversalOrder::Postorder => &index.postorder,
        TraversalOrder::ReversedPreorder => &index.reversed_preorder,
        TraversalOrder::ReversedPostorder => &index.reversed_postorder,
    }
}

/// Computes bounded string edit distance with known maximal threshold
/// with runtime-configurable traversal pairs.
pub fn sed_k_with_config(
    t1: &SEDIndex,
    t2: &SEDIndex,
    k: usize,
    config: SedTraversalConfig,
) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    if t1.preorder.len() > t2.preorder.len() {
        (t1, t2) = (t2, t1);
    }

    let first_dist = bounded_string_edit_distance(
        select_traversal_i32(t1, config.first),
        select_traversal_i32(t2, config.first),
        k,
    );

    if first_dist > k {
        return first_dist;
    }

    let second_dist = bounded_string_edit_distance(
        select_traversal_i32(t1, config.second),
        select_traversal_i32(t2, config.second),
        k,
    );
    std::cmp::max(first_dist, second_dist)
}

/// Computes bounded string edit distance with known maximal threshold.
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
pub fn sed_k(t1: &SEDIndex, t2: &SEDIndex, k: usize) -> usize {
    sed_k_with_config(t1, t2, k, SedTraversalConfig::default())
}

/// Computes bounded string edit distance with known maximal threshold
/// with runtime-configurable traversal pairs.
pub fn sed_struct_k_with_config(
    t1: &SEDIndexWithStructure,
    t2: &SEDIndexWithStructure,
    k: usize,
    config: SedTraversalConfig,
) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    let (t1, t2, params) = prepare_sed_inputs!(t1, t2, k);

    let first_dist = bounded_string_edit_distance_with_structure(
        select_traversal_struct(t1, config.first),
        select_traversal_struct(t2, config.first),
        params.threshold,
        params.array_size,
        params.offset_0th_diagonal as i32,
        params.target_diagonal as i32,
        params.threshold as i32,
    );
    if first_dist > k {
        return first_dist;
    }

    let second_dist = bounded_string_edit_distance_with_structure(
        select_traversal_struct(t1, config.second),
        select_traversal_struct(t2, config.second),
        params.threshold,
        params.array_size,
        params.offset_0th_diagonal as i32,
        params.target_diagonal as i32,
        params.threshold as i32,
    );
    std::cmp::max(first_dist, second_dist)
}

/// Computes bounded string edit distance with known maximal threshold.
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
pub fn sed_struct_k(t1: &SEDIndexWithStructure, t2: &SEDIndexWithStructure, k: usize) -> usize {
    sed_struct_k_with_config(t1, t2, k, SedTraversalConfig::default())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraversalCharacter {
    pub char: i32,
    pub preorder_following_postorder_preceding: i32,
    pub preorder_descendant_postorder_ancestor: i32,

    pub sum: i32,
    pub diff: i32,
}

impl std::hash::Hash for TraversalCharacter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.char.hash(state);
    }
}

/// Implements fastest known way to compute exact string edit between two strings
fn string_edit_distance_with_structure(
    s1: &[TraversalCharacter],
    s2: &[TraversalCharacter],
    k: u32,
) -> usize {
    use std::cmp::min;
    // assumes size of s2 is bigger or equal than s1
    let s2len = s2.len() as u32;

    let mut cache: Vec<u32> = (1..s2len + 1).collect::<Vec<u32>>();

    // let mut matrix = vec![];
    // matrix.push(cache.clone());
    // dbg!(&cache);
    let mut result = s2len as u32;
    for (i, ca) in s1.iter().enumerate() {
        let mut insert_dist = i as u32;
        result = i as u32 + 1;

        for (j, cb) in s2.iter().enumerate() {
            let replace_dist = insert_dist + u32::from(ca.char != cb.char);
            unsafe {
                // TODO: If ca.info.abs_diff(cb.info) > k mark the cell as invalid, thus no computations
                // can be done from that cell
                insert_dist = *cache.get_unchecked(j);

                // TODO: if replace_dist + struct_diff > k
                result = if replace_dist
                    + (ca
                        .preorder_following_postorder_preceding
                        .abs_diff(cb.preorder_following_postorder_preceding)
                        + ca.preorder_descendant_postorder_ancestor
                            .abs_diff(cb.preorder_descendant_postorder_ancestor))
                    > k
                {
                    min(insert_dist + 1, result + 1)
                } else {
                    min(replace_dist, min(insert_dist + 1, result + 1))
                };
                *cache.get_unchecked_mut(j) = result;
            }
        }
        // matrix.push(cache.clone());
        #[cfg(debug_assertions)]
        {
            dbg!(&cache);
        }
    }

    // #[cfg(debug_assertions)]
    // {
    //     println!("");
    //     for j in 0..cache.len() {
    //         // print!("Row  {:>3}: [", j);
    //         for i in 0..matrix.len() {
    //             if i > 0 {
    //                 print!(",");
    //             }
    //             print!("{:>3}", matrix[i][j]);
    //         }
    //         println!("]");
    //     }
    //     dbg!(&matrix);
    // }
    // Print matrix by columns
    // println!("Matrix by columns:");
    // print!("Row    S1   ");
    // for c in s1.iter() {
    //     print!("  {:>3}", c.char);
    // }

    result as usize
}

pub fn sed_k_br<'a, T: Eq>(br: &'a mut BerghelRoachSed<T>, target: &'a [T]) -> usize {
    br.compute_distance(&target)
}

pub fn bounded_string_edit_distance(s1: &[i32], s2: &[i32], k: usize) -> usize {
    use std::cmp::{max, min};
    let (s1, s2) = if s1.len() <= s2.len() {
        (s1, s2)
    } else {
        (s2, s1)
    };

    let s1len = s1.len() as i64;
    let s2len = s2.len() as i64;

    let threshold = min(s2len, k as i64);
    let size_diff = s2len - s1len;
    if size_diff > threshold {
        return usize::MAX;
    }

    let zero_k: i64 = ((if s1len < threshold { s1len } else { threshold }) >> 1) + 2;

    let arr_len = size_diff + (zero_k) * 2 + 2;

    let mut current_row = vec![-1i64; arr_len as usize];
    let mut next_row = vec![-1i64; arr_len as usize];
    let mut i: i64 = 0;
    let condition_diag = size_diff + zero_k;
    let end_max = condition_diag << 1;

    loop {
        i += 1;
        std::mem::swap(&mut next_row, &mut current_row);

        // Calculate original band boundaries from Berghel-Roach algorithm
        let original_start: i64;
        if i <= zero_k {
            original_start = -i + 1;
        } else {
            original_start = i - (zero_k << 1) + 1;
        }

        let original_end: i64;
        if i <= condition_diag {
            original_end = i;
            unsafe {
                *next_row.get_unchecked_mut((zero_k + i) as usize) = -1;
            }
        } else {
            original_end = end_max - i;
        }

        // Precompute valid diagonal range based on budget
        let budget = threshold - (i - 1);

        // If budget is negative or zero, only the target diagonal is valid
        let (min_valid_diag, max_valid_diag) = if budget <= 0 {
            (size_diff, size_diff)
        } else {
            (size_diff - budget, size_diff + budget)
        };

        // Intersect the original band with the budget-constrained range
        let start = max(original_start, min_valid_diag);
        let end = min(original_end, max_valid_diag + 1); // +1 because range is exclusive

        // Initialize cell variables for the adjusted starting position
        // These represent values from the previous cost level (i-1):
        // - current_cell: value at diagonal (start - 1)
        // - next_cell: value at diagonal (start)
        let mut current_cell: i64;
        let mut next_cell: i64;
        let mut previous_cell: i64;

        // Load initial values from previous row based on adjusted start position
        if i <= zero_k && start == original_start {
            // Original initialization for the standard case
            current_cell = -1;
            next_cell = i - 2i64;
        } else {
            // When start is adjusted, load values from the appropriate positions
            unsafe {
                let start_idx = (zero_k + start) as usize;
                current_cell = if start > original_start && start_idx > 0 {
                    *current_row.get_unchecked(start_idx - 1)
                } else {
                    -1
                };
                next_cell = *current_row.get_unchecked(start_idx);
            }
        }

        let mut row_index = (start + zero_k) as usize - 1;

        let mut t;

        for q in start..end {
            row_index += 1;
            previous_cell = current_cell;
            current_cell = next_cell;
            unsafe {
                next_cell = *current_row.get_unchecked(row_index + 1);
            }

            // max()
            t = max(max(current_cell + 1, previous_cell), next_cell + 1);

            unsafe {
                while t < s1len
                    && (t + q) < s2len
                    && s1.get_unchecked(t as usize) == s2.get_unchecked((t + q) as usize)
                {
                    t += 1;
                }
            }

            unsafe {
                *next_row.get_unchecked_mut(row_index) = t;
            }
        }

        unsafe {
            let condition_value = *next_row.get_unchecked(condition_diag as usize);
            if !(condition_value < s1len && i <= threshold) {
                if !(condition_value >= s1len) && i > threshold {
                    break usize::MAX;
                }
                break (i - 1) as usize;
            }
        }
    }
}

/// Performs bounded string edit distance with known maximal threshold
/// based on the algorithm by Hal Berghel and David Roach
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
/// Assumes size of s2 is bigger or equal than s1
pub fn bounded_string_edit_distance_with_structure(
    s1: &[TraversalCharacter],
    s2: &[TraversalCharacter],
    k: usize,
    arr_len: usize,
    zero_k: i32,
    size_diff: i32,
    threshold: i32,
) -> usize {
    // assumes size of s2 is bigger or equal than s1
    let s1len = s1.len() as i32;
    let s2len = s2.len() as i32;
    use std::cmp::{max, min};

    // Instead of storing the full DP matrix, Ukkonen's algorithm only stores
    // the current and next row (optimization described in the paper)
    // SCRATCH.with(|(slots)| {
    // let mut current_row: &mut Vec<(i32, bool)> = unsafe { &mut (*slots.get()).0 };
    // let mut next_row: &mut Vec<(i32, bool)> = unsafe { &mut (*slots.get()).1 };
    // current_row.clear();
    // current_row.resize(arr_len as usize, (-1, true));
    // next_row.clear();
    // next_row.resize(arr_len as usize, (-1, true));

    let mut current_row = vec![(-1, true); arr_len as usize];
    let mut next_row = vec![(-1, true); arr_len as usize];

    // println!("Initialized rows with length: {}", arr_len);
    let mut i = 0i32;
    // condition_diagonal is the diaogonal on which the resulting SED lies.
    // we will be checking this diagonal to determine if we can stop early
    let condition_diagonal = size_diff + zero_k;
    let condition_diagonal_idx = condition_diagonal as usize;
    let end_max = condition_diagonal << 1;

    // #[cfg(debug_assertions)]
    // {
    //     println!("Searching for first value: {s1len} on {condition_diagonal} with max k={threshold} on ZERO_K={zero_k}");
    //     print!(" --   |");
    //     for i in 0..arr_len {
    //         print!(" {i:>4} |");
    //     }
    //     println!("");
    // }

    let mut next_allowed_substitution = true;
    loop {
        // i here is the current allowed edit distance
        i += 1;
        std::mem::swap(&mut next_row, &mut current_row);

        // Calculate original band boundaries from Berghel-Roach algorithm
        let original_start: i32;
        if i <= zero_k {
            original_start = -i + 1;
        } else {
            original_start = i - (zero_k << 1) + 1;
        }

        let original_end: i32;
        if i <= condition_diagonal {
            original_end = i;
            unsafe {
                *next_row.get_unchecked_mut((zero_k + i) as usize) = (-1, true);
            }
        } else {
            original_end = end_max - i;
        }

        // Precompute valid diagonal range based on budget
        // Use k (not threshold) for budget calculation since k is the actual distance limit
        let budget = k as i32 - (i - 1);

        // If budget is negative or zero, only the target diagonal is valid
        let (min_valid_diag, max_valid_diag) = if budget <= 0 {
            (size_diff, size_diff)
        } else {
            (size_diff - budget, size_diff + budget)
        };

        // Intersect the original band with the budget-constrained range
        let start = max(original_start, min_valid_diag);
        let end = min(original_end, max_valid_diag + 1); // +1 because range is exclusive

        // Initialize cell variables for the adjusted starting position
        // These represent values from the previous cost level (i-1):
        // - current_cell: value at diagonal (start - 1)
        // - next_cell: value at diagonal (start)
        let mut current_cell: i32;
        let mut next_cell: i32;
        let mut previous_cell: i32;
        let mut next_allowed_substitution: bool;

        // Load initial values from previous row based on adjusted start position
        if i <= zero_k && start == original_start {
            // Original initialization for the standard case
            current_cell = -1;
            next_cell = i - 2i32;
            next_allowed_substitution = true;
        } else {
            // When start is adjusted, load values from the appropriate positions
            unsafe {
                let start_idx = (zero_k + start) as usize;
                current_cell = if start > original_start && start_idx > 0 {
                    current_row.get_unchecked(start_idx - 1).0
                } else {
                    -1
                };
                (next_cell, next_allowed_substitution) = *current_row.get_unchecked(start_idx);
            }
        }

        let current_edit_distance = (i - 1) as u32;
        let mut diagonal_index: usize = (start + zero_k).try_into().unwrap();

        let mut max_row_number;
        let allowed_edits = i - 1;

        // Process each diagonal in the band for this iteration
        let mut can_substitute: bool;
        for diag_offset in start..end {
            // Per Ukkonen's algorithm, we're tracking three values to compute each cell:
            // previous_cell, current_cell, and next_cell from the previous row

            // f(d-1, p-1) - insertion - row remains
            previous_cell = current_cell;
            // f(d, p-1) - substitution of character
            current_cell = next_cell;
            can_substitute = next_allowed_substitution;
            unsafe {
                // f(d+1, p-1) - deletion - max row index adds by +1
                (next_cell, next_allowed_substitution) =
                    *current_row.get_unchecked(diagonal_index + 1);
            }

            // Calculate the max of three possible operations (delete, insert, replace)
            // This is the standard dynamic programming recurrence relation for edit distance

            // however replacement can not occur in all cases, only if the mapping is possible

            // current_cell is basically the row in the matrix

            unsafe {
                // do a current_cell + 1
                // If substitution is not allowed, treat as insertion/deletion (not diagonal move) current_cell + 0
                max_row_number = max(
                    current_cell + (if can_substitute { 1 } else { 0 }),
                    max(previous_cell, next_cell + 1),
                );

                if !can_substitute {
                    // pokud nemuzu delat substituci a previous a next nedaji vetsi cislo, tak jen vezmu cislo
                    // current_cell, rovnou zapisu a nemusim se ani pokouset delat extension - zda se mi to zvetsi

                    if max_row_number == current_cell {
                        *next_row.get_unchecked_mut(diagonal_index) = (max_row_number, false);
                        diagonal_index += 1;
                        continue;
                    }
                }
            }
            // can_substitute = true;
            // let mut max_row_number = max_row_number as usize;
            unsafe {
                let k = threshold as i32;
                // The core extension to the original algorithm: match characters while possible
                // and consider both character equality AND structural constraints
                // This is the diagonal extension from Ukkonen's algorithm

                // Branchless optimization: Instead of breaking on structural constraint violation,
                // we compute how many characters we can advance before hitting the constraint.
                // This eliminates the inner branch and reduces pipeline stalls.

                // First, find the maximum possible advance based on character equality

                let mut struct_ok = false;

                // Optimized: fetch once, reuse
                while max_row_number < s1len && (max_row_number + diag_offset) < s2len {
                    let c1 = s1.get_unchecked(max_row_number as usize);
                    let c2 = s2.get_unchecked((max_row_number + diag_offset) as usize);

                    // TODO: change computation back without translated coordinates for a bit
                    let char_eq = c1.char == c2.char;
                    struct_ok = (allowed_edits + (c1.sum - c2.sum).abs() <= k)
                        && (allowed_edits + (c1.diff - c2.diff).abs() <= k);

                    // struct_ok = (allowed_edits
                    //     + c1.preorder_descendant_postorder_ancestor
                    //         .abs_diff(c2.preorder_descendant_postorder_ancestor)
                    //         as i32
                    //     + c1.preorder_following_postorder_preceding
                    //         .abs_diff(c2.preorder_following_postorder_preceding)
                    //         as i32)
                    //     <= k;

                    if !char_eq || !struct_ok {
                        break;
                    }
                    max_row_number += 1;
                }

                // Branchless update: advance by the minimum of character and structural constraints

                // disable substitution if we hit the big sturctural diff. If the problem is only character mismatch, it should be true
                // Update substitution flag without branching: can substitute if we matched all characters
                // that were equal (no structural constraint violation occurred)
                *next_row.get_unchecked_mut(diagonal_index) = (max_row_number, struct_ok);
            }

            diagonal_index += 1;
        }

        // dbg!(&next_row);
        // #[cfg(debug_assertions)]
        // {
        //     print!("p={:>3} |", i - 1);
        //     for (v, sub) in next_row.iter() {
        //         print!(" {v:>3}{s}|", s = if !sub { "x" } else { "" });
        //     }
        //     println!(" -- cond: {condition_diagonal}");
        // }

        // Check termination condition: either we've computed enough rows
        // to determine the distance is > threshold, or we've reached the
        // threshold itself - this follows the "cutoff" principle in the paper
        unsafe {
            if !(next_row.get_unchecked(condition_diagonal_idx).0 < s1len as i32 && i <= threshold)
            {
                if threshold < k as i32 {
                    break ((i - 1) + k as i32 - threshold) as usize;
                }

                if !(next_row.get_unchecked(condition_diagonal_idx).0 >= s1len as i32)
                    && i > threshold
                {
                    break usize::MAX;
                }

                break (i - 1) as usize;
            }
        }
    }
    // })
}

/// Berghel & Roach bounded string edit distance algorithm
/// Returns the edit distance if <= k, otherwise returns usize::MAX
/// Assumes s2.len() >= s1.len()
pub fn berghel_roach_distance(
    s1: &[TraversalCharacter],
    s2: &[TraversalCharacter],
    params: &SEDParameters,
) -> usize {
    let s1len = s1.len();
    let s2len = s2.len();
    let k = params.threshold as i32;
    assert!(
        s2len >= s1len,
        "Berghel & Roach algorithm assumes s2 is longer or equal to s1"
    );

    // If length difference exceeds threshold, distance must be > k
    if params.target_diagonal > params.threshold {
        return usize::MAX;
    }

    // The target diagonal where we need to reach m (end of s1)
    // and is the same as (s2 - s1) length difference, which is the diagonal offset we need to reach
    let target_diagonal = params.target_diagonal as i32;

    // FROW array: stores the farthest row reached on each diagonal for current p
    // Index mapping: diagonal d is stored at index (d + k + 1)
    // We need range [-k-1, n-m+k+1], but we'll allocate conservatively
    let offset = params.offset_0th_diagonal as i32;

    let mut frow_curr = vec![-1i32; params.array_size];
    let mut frow_prev = vec![-1i32; params.array_size];

    #[inline(always)]
    fn greedy_extend(
        s1: &[TraversalCharacter],
        s2: &[TraversalCharacter],
        start_row: i32,
        diag_offset: i32,
    ) -> i32 {
        let mut row = start_row;
        let max_allowed_extend = std::cmp::min(s1.len() as i32, s2.len() as i32 - (diag_offset));
        while row < max_allowed_extend
            && s1[row as usize].char == s2[(row + diag_offset) as usize].char
        {
            row += 1;
        }
        row
    }

    #[cfg(debug_assertions)]
    {
        dbg!(s1.iter().map(|c| c.char).collect::<Vec<_>>());
        dbg!(s2.iter().map(|c| c.char).collect::<Vec<_>>());

        print!("p=0    |");
        for (i, v) in frow_curr.iter().enumerate() {
            print!(
                " {v:>3}{s}|",
                s = if i == (target_diagonal + offset) as usize {
                    "*"
                } else {
                    ""
                }
            );
        }
        println!(" -- diag range [0, 0]");
    }

    // Main loop: iterate over number of edits p
    for p in target_diagonal..=k {
        std::mem::swap(&mut frow_curr, &mut frow_prev);

        // Berghel & Roach bounds: only process diagonals within reach
        // With p edits, we can reach diagonals in range:
        // [target_diagonal - (k - p), target_diagonal + (k - p)]

        let remaining = k - p;
        let diag_min = target_diagonal - remaining;
        let diag_max = target_diagonal + remaining;

        // Also constrained by matrix boundaries and edit budget
        let diag_start = std::cmp::max(diag_min, -p);
        let diag_end = std::cmp::min(diag_max, target_diagonal + p);

        for diag_offset in diag_start..=diag_end {
            let idx = (offset + diag_offset) as usize;

            // Three possibilities for reaching diagonal d with p edits:
            // 1. Deletion: from diagonal d+1, advance row by 1
            // 2. Insertion: from diagonal d-1, keep same row
            // 3. Substitution: from diagonal d, advance row by 1

            let from_deletion = frow_prev[idx + 1] + 1;

            let from_insertion = frow_prev[idx - 1];

            let from_substitution = frow_prev[idx] + 1;
            // Take the maximum row we can reach
            let mut max_row = std::cmp::max(
                from_deletion,
                std::cmp::max(from_insertion, from_substitution),
            );

            // Greedy extension: match as many characters as possible
            // On diagonal d, position (row, col) where col = row + d
            let m_i32 = s2len as i32;
            let n_i32 = s1len as i32;

            max_row = greedy_extend(&s1, &s2, max_row, diag_offset);

            frow_curr[idx] = max_row;

            // Early termination: if we reached the end of s1 on target diagonal
            if diag_offset == target_diagonal && max_row >= n_i32 {
                #[cfg(debug_assertions)]
                {
                    print!("p={p:>3} |");
                    for (i, v) in frow_curr.iter().enumerate() {
                        print!(
                            " {v:>3}{s}|",
                            s = if i == (target_diagonal + offset) as usize {
                                "*"
                            } else {
                                ""
                            }
                        );
                    }
                    println!(" -- diag range [{diag_start}, {diag_end}]");
                }
                return p as usize;
            }
        }

        #[cfg(debug_assertions)]
        {
            print!("p={p:>3} |");
            for (i, v) in frow_curr.iter().enumerate() {
                print!(
                    " {v:>3}{s}|",
                    s = if i == (target_diagonal + offset) as usize {
                        "*"
                    } else {
                        ""
                    }
                );
            }
            println!(" -- diag range [{diag_start}, {diag_end}]");
        }
    }

    // If we exhausted k edits without reaching the target
    usize::MAX
}

#[cfg(test)]
mod tests {
    use std::process::Output;

    use num_traits::zero;
    use rayon::vec;

    use crate::{
        indexing::Indexer,
        parsing::{parse_single, tree_to_string, LabelDict, TreeOutput},
    };

    #[test]
    fn test_initialize_fkp_matrix() {
        let max_diag = 10;
        let max_cost = 3;
        let zero_diagonal_offset = max_diag / 2; // = 5
        let matrix = BerghelRoachSed::<i32>::initialize_fkp_matrix(
            zero_diagonal_offset,
            max_diag,
            max_cost + 2, // max_cost param = 5
        );

        // Column-major layout: each diagonal's cost entries are contiguous.
        // cost_stride = max_cost_param + 2 = 7
        // For each diag d, base case is at cost = |d| - 1:
        //   negative d → value = |d| - 1
        //   positive d → value = -1
        let cost_stride = (max_cost + 2 + 2) as usize; // 7

        // diag = -5 (idx 0): base case at cost=4, val=4
        let mut col0 = vec![i32::MIN; cost_stride];
        col0[5] = 4; // cost=4 → index 4+1=5

        // diag = -4 (idx 1): base case at cost=3, val=3
        let mut col1 = vec![i32::MIN; cost_stride];
        col1[4] = 3;

        // diag = -3: base case at cost=2, val=2
        let mut col2 = vec![i32::MIN; cost_stride];
        col2[3] = 2;

        // diag = -2: base case at cost=1, val=1
        let mut col3 = vec![i32::MIN; cost_stride];
        col3[2] = 1;

        // diag = -1: base case at cost=0, val=0
        let mut col4 = vec![i32::MIN; cost_stride];
        col4[1] = 0;

        // diag = 0: base case at cost=-1, val=-1
        let mut col5 = vec![i32::MIN; cost_stride];
        col5[0] = -1;

        // diag = +1: base case at cost=0, val=-1
        let mut col6 = vec![i32::MIN; cost_stride];
        col6[1] = -1;

        // diag = +2: base case at cost=1, val=-1
        let mut col7 = vec![i32::MIN; cost_stride];
        col7[2] = -1;

        // diag = +3: base case at cost=2, val=-1
        let mut col8 = vec![i32::MIN; cost_stride];
        col8[3] = -1;

        // diag = +4: base case at cost=3, val=-1
        let mut col9 = vec![i32::MIN; cost_stride];
        col9[4] = -1;

        // diag = +5: never set in init loop (loop range -5..5)
        let col10 = vec![i32::MIN; cost_stride];

        let mut combined = vec![];
        combined.extend(col0);
        combined.extend(col1);
        combined.extend(col2);
        combined.extend(col3);
        combined.extend(col4);
        combined.extend(col5);
        combined.extend(col6);
        combined.extend(col7);
        combined.extend(col8);
        combined.extend(col9);
        combined.extend(col10);

        assert_eq!(matrix, combined);
    }

    #[test]
    fn test_br_first_case() {
        let query = "garvey".chars().map(|c| c as char).collect::<Vec<_>>();
        let target = "avery".chars().map(|c| c as char).collect::<Vec<_>>();

        let mut br = BerghelRoachSed::<char>::initialize_query(&query, 3);
        let result = br.compute_distance(&target);
        assert_eq!(
            result, 3,
            "Expected edit distance of 3 between 'garvey' and 'avery' with k=3"
        );

        br.reinitialize_query(&query, 2);
        let result = br.compute_distance(&target);
        assert_eq!(
            result,
            usize::MAX,
            "Expected edit non computable (distance > k) between 'garvey' and 'avery' with k=2"
        );

        let query = "abcde".chars().map(|c| c as char).collect::<Vec<_>>();
        let target = "fghij".chars().map(|c| c as char).collect::<Vec<_>>();

        br.reinitialize_query(&query, 5);
        let result = br.compute_distance(&target);
        assert_eq!(
            result, 5,
            "Expected edit distance of 5 between 'abcde' and 'fghij' with k=5"
        );

        let query = "kitten".chars().map(|c| c as char).collect::<Vec<_>>();
        let target = "sitting".chars().map(|c| c as char).collect::<Vec<_>>();

        br.reinitialize_query(&query, 3);
        let result = br.compute_distance(&target);
        assert_eq!(
            result, 3,
            "Expected edit distance of 3 between 'kitten' and 'sitting' with k=5"
        );

        let query = "123456452abc"
            .chars()
            .map(|c| c as char)
            .collect::<Vec<_>>();
        let target = "173829526452abc"
            .chars()
            .map(|c| c as char)
            .collect::<Vec<_>>();

        br.reinitialize_query(&query, 4);
        let result = br.compute_distance(&target);
        assert_eq!(
            result,
            usize::MAX,
            "Expected edit distance of 3 between 'kitten' and 'sitting' with k=5"
        );

        // assert_eq!(matrix, initialized_fkp_target);
    }

    #[test]
    fn test_sed_boundd_first_case() {
        let query = "garvey".chars().map(|c| c as i32).collect::<Vec<_>>();
        let target = "avery".chars().map(|c| c as i32).collect::<Vec<_>>();

        let result = bounded_string_edit_distance(&query, &target, 3);
        assert_eq!(
            result, 3,
            "Expected edit distance of 3 between 'garvey' and 'avery' with k=3"
        );

        let result = bounded_string_edit_distance(&query, &target, 2);
        assert_eq!(
            result,
            usize::MAX,
            "Expected edit non computable (distance > k) between 'garvey' and 'avery' with k=2"
        );

        let query = "abcde".chars().map(|c| c as i32).collect::<Vec<_>>();
        let target = "fghij".chars().map(|c| c as i32).collect::<Vec<_>>();

        let result = bounded_string_edit_distance(&query, &target, 5);
        assert_eq!(
            result, 5,
            "Expected edit distance of 5 between 'abcde' and 'fghij' with k=5"
        );

        let query = "kitten".chars().map(|c| c as i32).collect::<Vec<_>>();
        let target = "sitting".chars().map(|c| c as i32).collect::<Vec<_>>();

        let result = bounded_string_edit_distance(&query, &target, 3);
        assert_eq!(
            result, 3,
            "Expected edit distance of 3 between 'kitten' and 'sitting' with k=5"
        );

        let query = "123456452abc".chars().map(|c| c as i32).collect::<Vec<_>>();
        let target = "173829526452abc"
            .chars()
            .map(|c| c as i32)
            .collect::<Vec<_>>();

        let result = bounded_string_edit_distance(&query, &target, 4);
        assert_eq!(
            result,
            usize::MAX,
            "Expected edit distance of 3 between 'kitten' and 'sitting' with k=5"
        );

        // assert_eq!(matrix, initialized_fkp_target);
    }

    use super::*;
    macro_rules! prepare_sed_inputs_traversals {
        ($t1:expr, $t2:expr, $k:expr) => {{
            let (mut t1, mut t2) = ($t1, $t2);
            if t1.len() > t2.len() {
                (t1, t2) = (t2, t1);
            }
            let params = compute_sed_parameters(&t1.len(), &t2.len(), &$k);
            (t1, t2, params)
        }};
    }

    #[test]
    fn test_bounded_sed_br_structure() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> g
        // 2 -> a
        // 3 -> r
        // 4 -> v
        // 5 -> e
        // 6 -> y

        // garvey
        let mut v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        // avery
        let mut v2 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 3;

        let (v1, v2, params) = prepare_sed_inputs_traversals!(v1, v2, threshold_k);

        let result = berghel_roach_distance(&v1, &v2, &params);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_bounded_sed_br_worst_case_structure() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> g
        // 2 -> a
        // 3 -> r
        // 4 -> v
        // 5 -> e
        // 6 -> y

        // garvey
        let mut v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        // avery
        let mut v2 = vec![
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 7,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 8,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 9,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 10,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 5;
        let (v1, v2, params) = prepare_sed_inputs_traversals!(v1, v2, threshold_k);
        let result = berghel_roach_distance(&v1, &v2, &params);
        assert_eq!(result, 5);
    }

    #[test]
    fn test_bounded_sed_structure() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> g
        // 2 -> a
        // 3 -> r
        // 4 -> v
        // 5 -> e
        // 6 -> y

        // arvey
        let v1 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        // avery
        let v2 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 3;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;

        let result = string_edit_distance_with_structure(&v2, &v1, threshold_k as u32);
        dbg!(&result);
        assert!(result <= 2);
        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        dbg!(&result);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_bounded_sed_structure_2() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> s
        // 2 -> k
        // 3 -> i
        // 4 -> t
        // 5 -> e
        // 6 -> n
        // 7 -> g

        // sitting
        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 7,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        // kitten
        let v2 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 3;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;

        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        dbg!(&result);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_bounded_sed_structure_simple() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> a
        // 2 -> b

        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 3,
                sum: 3,
                diff: -3,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 1,
                sum: 1,
                diff: -1,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let threshold_k = 1;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let result = string_edit_distance_with_structure(&v2, &v1, threshold_k as u32);
        assert_eq!(result, 4);

        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        assert_eq!(result, 1);
    }

    #[test]
    fn test_bounded_sed_structure_simple_unmatched() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> a
        // 2 -> b

        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 3,
                preorder_descendant_postorder_ancestor: 0,
                sum: 3,
                diff: 3,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let threshold_k = 2;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let result = string_edit_distance_with_structure(&v1, &v2, threshold_k as u32);
        assert_eq!(result, 2);
        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        assert_eq!(result, 2);
    }

    #[test]
    fn test_bounded_sed_structure_simple_test() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> a
        // 2 -> b

        let v1 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 1;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        assert_eq!(result, usize::MAX);
    }

    #[test]
    fn test_bounded_sed_vs_unbouded_sed_edit_distance() {
        // i have simple alphabet mapping for testing purposes
        // 1 -> a
        // 2 -> b

        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 2,
                sum: 4,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 2,
                sum: 4,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 1;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        assert_eq!(result, 1);
    }

    #[test]
    fn test_sed() {
        let v1 = vec![1, 2, 3, 4, 5, 5, 6];
        let v2 = vec![1, 2, 3, 5, 6, 7, 6];

        let result = string_edit_distance(&v1, &v2);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_sed_simple() {
        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_descendant_postorder_ancestor: 0,
                preorder_following_postorder_preceding: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_descendant_postorder_ancestor: 0,
                preorder_following_postorder_preceding: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];

        let threshold_k = 2;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let result = bounded_string_edit_distance_with_structure(
            &v1,
            &v2,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );
        assert_eq!(result, 1);
    }

    #[test]
    fn test_sed_wt_structure() {
        // preorder traversal of simple tree with info about preceding nodes
        let v1: Vec<TraversalCharacter> = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
        ];

        let result = string_edit_distance_with_structure(&v1, &v2, 1);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_sed_preorder_structure() {
        let t1str = "{a{a{b{a{a}}}}}".to_owned();
        let t2str = "{a{b{b{b}}{a{a}}}}".to_owned();
        let mut ld = LabelDict::default();
        let qt = parse_single(t1str, &mut ld);
        let tt = parse_single(t2str, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        assert_eq!(
            qs.preorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 4,
                    sum: 4,
                    diff: -4,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 3,
                    sum: 3,
                    diff: -3,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 2,
                    sum: 2,
                    diff: -2,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1,
                    sum: 1,
                    diff: -1,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0,
                    sum: 0,
                    diff: 0,
                },
            ]
        );

        assert_eq!(
            qs.postorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 4,
                    sum: 4,
                    diff: -4,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 3,
                    sum: 3,
                    diff: -3,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 2,
                    sum: 2,
                    diff: -2,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1,
                    sum: 1,
                    diff: -1,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0,
                    sum: 0,
                    diff: 0,
                },
            ]
        );

        assert_eq!(
            ts.preorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 5,
                    sum: 5,
                    diff: -5,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 4,
                    sum: 4,
                    diff: -4,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 2,
                    preorder_descendant_postorder_ancestor: 1,
                    sum: 3,
                    diff: 1,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 2,
                    preorder_descendant_postorder_ancestor: 0,
                    sum: 2,
                    diff: 2,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1,
                    sum: 1,
                    diff: -1,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0,
                    sum: 0,
                    diff: 0,
                },
            ]
        );
    }

    #[test]
    fn test_sed_query_and_tree() {
        let qstr = "{4{3{2 A}{3{2{3 rare}{2 and}}{3{2 lightly}{4 entertaining}}}}{3{2{2 look}{2{2 behind}{2{2{2 the}{2 curtain}}{2{2 that}{3{2{2 separates}{2 comics}}{3{2 from}{3{2{2 the}{2 people}}{3{4 laughing}{2{2 in}{2{2 the}{2 crowd}}}}}}}}}}}}}".to_owned();
        let tstr = "{3{2 Rehearsals}{2{2{2{2 are}{2 frequently}}{3{2 more}{3{3 fascinating}{2{2 than}{2{2 the}{2 results}}}}}}{2 .}}}"
            .to_owned();
        let mut ld = LabelDict::default();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        dbg!(tree_to_string(&qt, TreeOutput::BracketNotation));
        dbg!(tree_to_string(&tt, TreeOutput::BracketNotation));

        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);
        dbg!(&qs
            .preorder
            .iter()
            .map(|c| char::from_u32(c.char as u32 + 64).unwrap())
            .collect::<Vec<char>>());
        dbg!(&ts
            .preorder
            .iter()
            .map(|c| char::from_u32(c.char as u32 + 64).unwrap())
            .collect::<Vec<char>>());

        let result = sed_struct_k(&qs, &ts, 30);

        assert!(result <= 30, "SED result is not as expected: {result} > 29");
    }

    #[test]
    fn test_bounded_is_worse_than_normal() {
        let mut ld = LabelDict::default();
        let qstr = "{a{b}{a{a}}}".to_owned();
        let tstr = "{b{b{a}}}".to_owned();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        let threshold_k = 3;
        // assumes size of s2 is bigger or equal than s1
        let s1len = ts.c.tree_size;
        let s2len = qs.c.tree_size;
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let sed =
            string_edit_distance_with_structure(&ts.preorder, &qs.preorder, threshold_k as u32);
        let bsed = bounded_string_edit_distance_with_structure(
            &ts.preorder,
            &qs.preorder,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );

        assert!(
            sed > bsed,
            "SED is not worse than bounded SED: {sed} <= {bsed}"
        );
    }

    #[test]
    fn test_bounded_unbalanced_tree() {
        let mut ld = LabelDict::default();
        // 15
        let qstr =
            "{4143{4335}{1291{265}}{2630{1481}{3285}{1220{3926}{2331{26}}{4656{4119}{1492}{2612}}}}}"
                .to_owned();
        // 15
        let tstr =
            "{3631{463}{4470}{1614{1308}{2094{77}}{3756{2713{2645}}{4227}{1086{2948}{4641}{3713}}}}}"
                .to_owned();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        let sed = string_edit_distance_with_structure(&ts.preorder, &qs.preorder, 17);
        // s2 is bigger
        let threshold_k = 17;
        // assumes size of s2 is bigger or equal than s1
        let s1len = ts.c.tree_size;
        let s2len = qs.c.tree_size;
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let bsed = bounded_string_edit_distance_with_structure(
            &ts.preorder,
            &qs.preorder,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );

        assert_eq!(sed, 17, "SED result is not as expected: {sed} != 17");
        assert_eq!(bsed, 17, "SED result is not as expected: {bsed} != 17");
    }

    #[test]
    fn test_sed_struct_correctness() {
        let qstr = "{a{a{a}{a}}{a{a}}}".to_owned();
        let tstr = "{a{a}{a{a}}}".to_owned();
        let mut ld = LabelDict::default();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 5,
                sum: 5,
                diff: -5,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 2,
                sum: 4,
                diff: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 3,
                preorder_descendant_postorder_ancestor: 0,
                sum: 3,
                diff: 3,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 1,
                sum: 1,
                diff: -1,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 3,
                sum: 3,
                diff: -3,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
                sum: 2,
                diff: 2,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 1,
                sum: 1,
                diff: -1,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
                sum: 0,
                diff: 0,
            },
        ];
        assert_eq!(qs.preorder, v1);
        assert_eq!(ts.preorder, v2);
        let threshold_k = 2;
        // assumes size of s2 is bigger or equal than s1
        let s1len = v1.len();
        let s2len = v2.len();
        let size_diff = s2len - s1len;
        // Per Berghel & Roach, the threshold is the min of s2 length and k
        let threshold = std::cmp::min(s2len, threshold_k);

        // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
        // The shift by 1 and addition of 2 ensures sufficient buffer space
        // as described in the Berghel & Roach paper
        let zero_k = (((if s1len < threshold { s1len } else { threshold }) >> 1) + 2);

        // Calculate array length needed to store diagonal values
        let arr_len = (size_diff + (zero_k) * 2 + 2);

        let zero_k = zero_k as i32;
        let result = bounded_string_edit_distance_with_structure(
            &v2,
            &v1,
            threshold_k,
            arr_len,
            zero_k,
            size_diff as i32,
            threshold as i32,
        );

        let result = sed_struct_k(&qs, &ts, threshold_k);
        dbg!(result);
        assert_eq!(result, 2, "SED result is not as expected: {result} != 2");
    }

    #[test]
    fn test_sed_struct_correctness_2() {
        let qstr = "{a{b}{a{a}}}".to_owned();
        let tstr = "{a{a{a}}}".to_owned();
        let mut ld = LabelDict::default();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);
        let result = sed_struct_k(&qs, &ts, 1);
        dbg!(result);
        assert!(result <= 1, "SED result is not as expected: {result} <= 1");
    }

    #[test]
    fn test_sed_struct_correctness_real_data() {
        let qstr = "{S{S{NPSBJ{NNP{Mr.}}{NNP{Coleman}}}{VP{VBD{said}}{NPTMP{DT{this}}{NN{week}}}{SBAR{IN{that}}{S{NPSBJ{PRP{he}}}{VP{MD{would}}{VP{VB{devote}}{NP{NP{DT{the}}{NN{remainder}}}{PP{IN{of}}{NP{DT{the}}{JJ{political}}{NN{season}}}}}{PPCLR{TO{to}}{NP{JJ{positive}}{NN{campaigning}}}}}}}}}}{Interpunction{,}}{CC{but}}{S{NPSBJ{DT{the}}{NN{truce}}}{VP{VBD{lasted}}{NP{RB{only}}{NNS{hours}}}}}{Interpunction{.}}}".to_owned();
        let tstr = "{S{NPSBJ{NP{VBG{Continuing}}{NN{demand}}}{PP{IN{for}}{NP{NNS{dollars}}}}{PP{IN{from}}{NP{JJ{Japanese}}{NNS{investors}}}}}{VP{VBD{boosted}}{NP{DT{the}}{NNP{U.S.}}{NN{currency}}}}{Interpunction{.}}}".to_owned();
        let mut ld = LabelDict::default();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        dbg!(tree_to_string(&qt, TreeOutput::BracketNotation));
        dbg!(tree_to_string(&tt, TreeOutput::BracketNotation));

        let result = sed_struct_k(&qs, &ts, 58);
        assert!(result <= 58, "SED result is not as expected: {result} > 58");
    }

    #[test]
    fn test_sed_string_structure_corectness() {
        let qstr = "{a{a{a{a}}}}".to_owned();
        let tstr = "{a{a}{a}{a}}".to_owned();
        let mut ld = LabelDict::default();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);
        let result = sed_struct_k(&qs, &ts, 1);
        assert!(result > 0, "SED result is not as expected: {result} > 0");
    }

    #[test]
    fn test_sed_k() {
        let v1 = vec![1, 2, 3, 4, 5, 5, 6];
        let v2 = vec![1, 2, 3, 5, 6, 7, 6];

        let result = bounded_string_edit_distance(&v1, &v2, 2);
        assert_eq!(result, usize::MAX);

        let result = bounded_string_edit_distance(&v1, &v2, 4);
        assert_eq!(result, 3);
    }
}
