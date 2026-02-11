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
    query: &'a [T],
}

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
        let max_diag = self.max_diag;
        let zero_diagonal_offset = self.zero_diagonal_offset;

        let target_diagonal = n - m;

        if target_diagonal > self.max_computable_cost {
            return usize::MAX;
        }

        let mut greedy_extend = |diag: i32, cost: i32, matrix: &mut Vec<i32>| {
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

            let mut max_row = unsafe {
                max(
                    previous_row.get_unchecked(offset_diag) + 1, // substitution
                    max(
                        *previous_row.get_unchecked(offset_diag - 1), // deletion
                        previous_row.get_unchecked(offset_diag + 1) + 1, // insertion
                    ),
                )
            };
            // While loop to extend the match (Ukkonen's optimization)
            // Added safe bounds check (t >= 0) just in case initialization used -999
            while max_row < m && max_row + diag < n {
                unsafe {
                    if s1.get_unchecked(max_row as usize)
                        != s2.get_unchecked((max_row + diag) as usize)
                    {
                        break;
                    }
                }
                max_row += 1;
            }

            //
            unsafe {
                *matrix.get_unchecked_mut(Self::access_fkp_matrix(
                    cost,
                    diag,
                    max_diag,
                    zero_diagonal_offset,
                )) = max_row;
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
                if *fkp_matrix.get_unchecked(current_target_diag_idx) == m {
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

    fn fkp_matrix_access(&self, row: usize, col: usize, cols: usize) -> i32 {
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

    fn initialize_fkp_matrix(zero_diagonal_offset: i32, max_diag: i32, max_cost: i32) -> Vec<i32> {
        let mut matrix = vec![i32::MIN; ((max_diag + 1) * (max_cost + 2)) as usize];
        for diag in -zero_diagonal_offset..(max_diag - zero_diagonal_offset) {
            for cost in -1..(max_cost + 1) {
                if cost == diag.abs() - 1 {
                    if diag < 0 {
                        matrix
                            [Self::access_fkp_matrix(cost, diag, max_diag, zero_diagonal_offset)] =
                            diag.abs() - 1;
                    } else {
                        matrix
                            [Self::access_fkp_matrix(cost, diag, max_diag, zero_diagonal_offset)] =
                            -1;
                    }
                } else {
                    matrix[Self::access_fkp_matrix(cost, diag, max_diag, zero_diagonal_offset)] =
                        i32::MIN;
                }
            }
        }

        matrix
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
    let array_size = (2 * k + 3) as usize;

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

/// Computes bounded string edit distance with known maximal threshold.
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
pub fn sed_struct_k(t1: &SEDIndexWithStructure, t2: &SEDIndexWithStructure, k: usize) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    // Usage in sed_struct_k:
    let (t1, t2, params) = prepare_sed_inputs!(t1, t2, k);

    let pre_dist = berghel_roach_distance(&t1.reversed_preorder, &t2.reversed_preorder, &params);
    if pre_dist > k {
        return pre_dist;
    }
    let post_dist = berghel_roach_distance(&t1.preorder, &t2.preorder, &params);
    std::cmp::max(pre_dist, post_dist)
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

/// Computes bounded string edit distance with known maximal threshold.
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
pub fn sed_k(t1: &SEDIndex, t2: &SEDIndex, k: usize) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    // if size of t1 is bigger than t2, swap them
    if t1.preorder.len() > t2.preorder.len() {
        (t1, t2) = (t2, t1);
    }
    let post_dist = bounded_string_edit_distance(&t1.preorder, &t2.preorder, k);

    if post_dist > k {
        return post_dist;
    }
    let pre_dist = bounded_string_edit_distance(&t1.postorder, &t2.postorder, k);
    std::cmp::max(pre_dist, post_dist)
}

pub fn sed_k_br<'a, T: Eq>(br: &'a mut BerghelRoachSed<T>, target: &'a [T]) -> usize {
    br.compute_distance(&target)
}

pub fn bounded_string_edit_distance(s1: &[i32], s2: &[i32], k: usize) -> usize {
    use std::cmp::{max, min};
    // assumes size of s2 is bigger or equal than s1
    // let mut s1len = s1.len();
    // let mut s2len = s2.len();
    // // perform suffix trimming
    // for _ in s1
    //     .iter()
    //     .rev()
    //     .zip(s2.iter().rev())
    //     .take_while(|(s1c, s2c)| s1c == s2c)
    // {
    //     s1len -= 1;
    //     s2len -= 1;
    //     if s1len == 0 {
    //         break;
    //     }
    // }

    // let mut common_prefix = 0;

    // // now prefix trimming
    // for _ in s1.iter().zip(s2.iter()).take_while(|(s1c, s2c)| s1c == s2c) {
    //     common_prefix += 1;
    //     if common_prefix >= s1len {
    //         break;
    //     }
    // }

    // if s1len == 0 {
    //     return s2len;
    // }

    // // prefix trimming done
    // let s1 = &s1[common_prefix..s1len];
    // let s2 = &s2[common_prefix..s2len];

    // s1len -= common_prefix;
    // s2len -= common_prefix;
    // // one string is gone by suffix and prefix trimming, so just return the remaining size
    // if s1len == 0 {
    //     return s2len;
    // }
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
    let condition_row = size_diff + zero_k;
    let end_max = condition_row << 1;

    loop {
        i += 1;
        std::mem::swap(&mut next_row, &mut current_row);

        let start: i64;
        let mut next_cell: i64;
        let mut previous_cell: i64;
        let mut current_cell: i64 = -1;

        if i <= zero_k {
            start = -i + 1;
            next_cell = i - 2i64;
        } else {
            start = i - (zero_k << 1) + 1;
            unsafe {
                next_cell = *current_row.get_unchecked((zero_k + start) as usize);
            }
        }

        let end: i64;
        if i <= condition_row {
            end = i;
            unsafe {
                *next_row.get_unchecked_mut((zero_k + i) as usize) = -1;
            }
        } else {
            end = end_max - i;
        }

        let mut row_index = (start + zero_k) as usize;

        let mut t;

        for q in start..end {
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
            row_index += 1;
        }

        unsafe {
            if !(*next_row.get_unchecked(condition_row as usize) < s1len && i <= threshold) {
                if !(*next_row.get_unchecked(condition_row as usize) >= s1len) && i > threshold {
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

        let mut start: i32;
        let mut next_cell: i32;
        let mut previous_cell: i32;
        let mut current_cell: i32 = -1;

        // Calculate the starting diagonal for this iteration
        // This follows Berghel & Roach's band algorithm approach
        if i <= zero_k {
            start = -i + 1;
            next_cell = i - 2i32;
        } else {
            // 2 if i = 11 and zero_k = 10
            start = i - (zero_k << 1) + 1;
            unsafe {
                (next_cell, next_allowed_substitution) =
                    *current_row.get_unchecked((zero_k + start) as usize);
            }
        }

        // Calculate the ending diagonal for this iteration
        let mut end: i32;
        if i <= condition_diagonal {
            end = i;
            unsafe {
                *next_row.get_unchecked_mut((zero_k + i) as usize) = (-1, true);
            }
        } else {
            end = end_max - i;
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

                    let char_eq = c1.char == c2.char;
                    struct_ok = (allowed_edits + (c1.sum - c2.sum).abs() <= k)
                        && (allowed_edits + (c1.diff - c2.diff).abs() <= k);

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
        let zero_diagonal_offset = max_diag / 2;
        let matrix = BerghelRoachSed::<i32>::initialize_fkp_matrix(
            zero_diagonal_offset,
            max_diag,
            max_cost + 2,
        );

        // at p = -1, diag = 0
        let mut index_calc = |row: usize, col: usize| row * max_diag as usize + col;

        let mut rv1 = vec![i32::MIN; max_diag as usize + 1];
        rv1[zero_diagonal_offset as usize] = -1;

        let mut rv2 = vec![i32::MIN; max_diag as usize + 1];
        rv2[zero_diagonal_offset as usize - 1] = 0;
        rv2[zero_diagonal_offset as usize + 1] = -1;
        let mut rv3 = vec![i32::MIN; max_diag as usize + 1];
        rv3[zero_diagonal_offset as usize - 2] = 1;
        rv3[zero_diagonal_offset as usize + 2] = -1;
        let mut rv4 = vec![i32::MIN; max_diag as usize + 1];
        rv4[zero_diagonal_offset as usize - 3] = 2;
        rv4[zero_diagonal_offset as usize + 3] = -1;
        let mut rv5 = vec![i32::MIN; max_diag as usize + 1];
        rv5[zero_diagonal_offset as usize - 4] = 3;
        rv5[zero_diagonal_offset as usize + 4] = -1;
        let mut rv6 = vec![i32::MIN; max_diag as usize + 1];
        rv6[zero_diagonal_offset as usize - 5] = 4;
        let mut rv7 = vec![i32::MIN; max_diag as usize + 1];

        // combine all rv vectors into single vector
        let mut combined = vec![];
        combined.extend(rv1);
        combined.extend(rv2);
        combined.extend(rv3);
        combined.extend(rv4);
        combined.extend(rv5);
        combined.extend(rv6);
        combined.extend(rv7);

        // Check some key values in the matrix
        assert_eq!(matrix, combined);
        // assert_eq!(matrix, initialized_fkp_target);
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
