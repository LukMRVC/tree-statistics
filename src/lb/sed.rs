use std::usize;

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

/// Computes bounded string edit distance with known maximal threshold.
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
pub fn sed_struct_k(t1: &SEDIndexWithStructure, t2: &SEDIndexWithStructure, k: usize) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    if t1.preorder.len() > t2.preorder.len() {
        (t1, t2) = (t2, t1);
    }
    let pre_dist = bounded_string_edit_distance_with_structure(&t1.preorder, &t2.preorder, k);
    if pre_dist > k {
        return pre_dist;
    }
    let post_dist = bounded_string_edit_distance_with_structure(
        &t1.reversed_preorder,
        &t2.reversed_preorder,
        k,
    );
    std::cmp::max(pre_dist, post_dist)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraversalCharacter {
    pub char: i32,
    pub preorder_following_postorder_preceding: i32,
    pub preorder_descendant_postorder_ancestor: i32,
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
    let post_dist = bounded_string_edit_distance(&t1.postorder, &t2.postorder, k);

    if post_dist > k {
        return post_dist;
    }
    let pre_dist = bounded_string_edit_distance(&t1.preorder, &t2.preorder, k);
    std::cmp::max(pre_dist, post_dist)
}

pub fn bounded_string_edit_distance(s1: &[i32], s2: &[i32], k: usize) -> usize {
    use std::cmp::{max, min};
    // assumes size of s2 is bigger or equal than s1
    let mut s1len = s1.len();
    let mut s2len = s2.len();
    // perform suffix trimming
    for _ in s1
        .iter()
        .rev()
        .zip(s2.iter().rev())
        .take_while(|(s1c, s2c)| s1c == s2c)
    {
        s1len -= 1;
        s2len -= 1;
        if s1len == 0 {
            break;
        }
    }

    let mut common_prefix = 0;

    // now prefix trimming
    for _ in s1.iter().zip(s2.iter()).take_while(|(s1c, s2c)| s1c == s2c) {
        common_prefix += 1;
        if common_prefix >= s1len {
            break;
        }
    }

    if s1len == 0 {
        return s2len;
    }

    // prefix trimming done
    let s1 = &s1[common_prefix..s1len];
    let s2 = &s2[common_prefix..s2len];

    s1len -= common_prefix;
    s2len -= common_prefix;
    // one string is gone by suffix and prefix trimming, so just return the remaining size
    if s1len == 0 {
        return s2len;
    }
    let s1len = s1len as i64;
    let s2len = s2len as i64;

    let threshold = min(s2len, k as i64);
    let size_diff = s2len - s1len;

    if threshold < size_diff {
        return threshold as usize;
    }

    let zero_k: i64 = ((if s1len < threshold { s1len } else { threshold }) >> 1) + 2;

    let arr_len = size_diff + (zero_k) * 2 + 2;

    let mut current_row = vec![-1i64; arr_len as usize];
    let mut next_row = vec![-1i64; arr_len as usize];
    let mut i = 0;
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
) -> usize {
    use std::cmp::{max, min};
    // assumes size of s2 is bigger or equal than s1
    let s1len = s1.len() as i32;
    let s2len = s2.len() as i32;
    let size_diff = s2len - s1len;
    // Per Berghel & Roach, the threshold is the min of s2 length and k
    let threshold = min(s2len, k as i32);

    // zero_k represents the initial diagonal (0th/main diagonal of the SED matrix) in the edit distance matrix
    // The shift by 1 and addition of 2 ensures sufficient buffer space
    // as described in the Berghel & Roach paper
    let zero_k: i32 = ((if s1len < threshold { s1len } else { threshold }) >> 1) + 2;

    // Calculate array length needed to store diagonal values
    let arr_len = size_diff + (zero_k) * 2 + 2;

    // Instead of storing the full DP matrix, Ukkonen's algorithm only stores
    // the current and next row (optimization described in the paper)
    let mut current_row = vec![(-1i32, true); arr_len as usize];
    let mut next_row = vec![(-1i32, true); arr_len as usize];
    let mut i = 0;
    // condition_diagonal is the diaogonal on which the resulting SED lies.
    // we will be checking this diagonal to determine if we can stop early
    let condition_diagonal = size_diff + zero_k;
    let end_max = condition_diagonal << 1;

    #[cfg(debug_assertions)]
    {
        println!("Searching for first value: {s1len} on {condition_diagonal} with max k={threshold} on ZERO_K={zero_k}");
        print!(" --   |");
        for i in 0..arr_len {
            print!(" {i:>4} |");
        }
        println!("");
    }

    // prepare a simple test function if characters are eligible for substitution
    #[inline]
    fn struct_diff(t1: &TraversalCharacter, t2: &TraversalCharacter) -> i32 {
        (t1.preorder_following_postorder_preceding
            .abs_diff(t2.preorder_following_postorder_preceding)
            + t1.preorder_descendant_postorder_ancestor
                .abs_diff(t2.preorder_descendant_postorder_ancestor)) as i32
    }

    let mut next_allowed_substitution = true;
    loop {
        // i here is the current allowed edit distance
        i += 1;
        std::mem::swap(&mut next_row, &mut current_row);

        let start: i32;
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
        let end: i32;
        if i <= condition_diagonal {
            end = i;
            unsafe {
                *next_row.get_unchecked_mut((zero_k + i) as usize) = (-1, true);
            }
        } else {
            end = end_max - i;
        }
        let current_edit_distance = (i - 1) as u32;
        let mut diagonal_index = (start + zero_k) as usize;

        let mut max_row_number;
        let allowed_edits = i - 1;

        // Process each diagonal in the band for this iteration
        for diag_offset in start..end {
            let mut can_substitute: bool;

            // Per Ukkonen's algorithm, we're tracking three values to compute each cell:
            // previous_cell, current_cell, and next_cell from the previous row

            // f(d-1, p-1) - insertion - row remains
            previous_cell = current_cell;
            // f(d, p-1) - substitution of character
            current_cell = next_cell;
            can_substitute = next_allowed_substitution;
            unsafe {
                can_substitute = current_row.get_unchecked(diagonal_index).1;
                // f(d+1, p-1) - deletion - max row index adds by +1
                (next_cell, next_allowed_substitution) =
                    *current_row.get_unchecked(diagonal_index + 1);
            }

            // Calculate the max of three possible operations (delete, insert, replace)
            // This is the standard dynamic programming recurrence relation for edit distance

            // however replacement can not occur in all cases, only if the mapping is possible

            // current_cell is basically the row in the matrix

            unsafe {
                // TODO: Maybe this is allowing substitution on the first ever step, when it is not allowed?

                // TODO: checking if current_cell + 1 < s1len if not correct... I need to be able to
                // do a current_cell + 1
                max_row_number = max(current_cell + 1, max(previous_cell, next_cell + 1));

                // If substitution is not allowed, treat as insertion/deletion (not diagonal move)
                if !can_substitute {
                    // pokud nemuzu delat substituci a previous a next nedaji vetsi cislo, tak jen vezmu cislo
                    // current_cell, rovnou zapisu a nemusim se ani pokouset delat extension - zda se mi to zvetsi

                    max_row_number = max(max(previous_cell, current_cell), next_cell + 1);

                    if max_row_number == current_cell {
                        // TODO: jen zapsat a continue
                        *next_row.get_unchecked_mut(diagonal_index) = (max_row_number, false);
                        diagonal_index += 1;
                        continue;
                    }
                }

                // if i32::from(
                //     !(current_cell + 1 < s1len
                //         && (current_cell + 1 + diag_offset) < s2len
                //         && !can_be_substituted(
                //             s1.get_unchecked((current_cell + 1) as usize),
                //             s2.get_unchecked((current_cell + 1 + diag_offset) as usize),
                //             k,
                //         )),
                // ) {
                //     // well here's the problem
                //     // if we are not allowing substitution due tue to structure, there is
                //     // a possibility on when to actually allowed the substitution but at higher cost...
                //     // which means that we need to check:
                //     // 1. (diagonal + 1) cell, which got on the same row with I edits
                //     // 2. (diagonal - 1) cell, which got on the same (row - 1) with I edits

                //     // max(previous_cell, next_cell + 1)

                //     max_row_number = max(max(previous_cell, next_cell + 1), current_cell);
                // }
            }
            // can_substitute = true;
            // let mut max_row_number = max_row_number as usize;
            unsafe {
                let k = k as i32;
                // The core extension to the original algorithm: match characters while possible
                // and consider both character equality AND structural constraints
                // This is the diagonal extension from Ukkonen's algorithm

                // Branchless optimization: Instead of breaking on structural constraint violation,
                // we compute how many characters we can advance before hitting the constraint.
                // This eliminates the inner branch and reduces pipeline stalls.

                // First, find the maximum possible advance based on character equality

                let mut char_eq = false;
                let mut struct_ok = false;
                let mut struct_match_count = 0i32;
                while max_row_number + struct_match_count < s1len
                    && (max_row_number + struct_match_count + diag_offset) < s2len
                {
                    char_eq = s1
                        .get_unchecked((max_row_number + struct_match_count) as usize)
                        .char
                        == s2
                            .get_unchecked(
                                (max_row_number + struct_match_count + diag_offset) as usize,
                            )
                            .char;
                    struct_ok = (allowed_edits
                        + struct_diff(
                            s1.get_unchecked((max_row_number + struct_match_count) as usize),
                            s2.get_unchecked(
                                (max_row_number + struct_match_count + diag_offset) as usize,
                            ),
                        ))
                        <= k as i32;

                    if (!char_eq || !struct_ok) {
                        break;
                    }

                    struct_match_count += 1;
                }

                // Branchless update: advance by the minimum of character and structural constraints
                max_row_number += struct_match_count;

                // disable substitution if we hit the big sturctural diff. If the problem is only character mismatch, it should be true
                // Update substitution flag without branching: can substitute if we matched all characters
                // that were equal (no structural constraint violation occurred)
                can_substitute = struct_ok;
                *next_row.get_unchecked_mut(diagonal_index) = (max_row_number, can_substitute);
            }

            diagonal_index += 1;
        }

        // dbg!(&next_row);
        #[cfg(debug_assertions)]
        {
            print!("p={:>3} |", i - 1);
            for (v, sub) in next_row.iter() {
                print!(" {v:>3}{s}|", s = if !sub { "x" } else { "" });
            }
            println!(" -- cond: {condition_diagonal}");
        }

        // Check termination condition: either we've computed enough rows
        // to determine the distance is > threshold, or we've reached the
        // threshold itself - this follows the "cutoff" principle in the paper
        unsafe {
            if !(next_row.get_unchecked(condition_diagonal as usize).0 < s1len as i32
                && i <= threshold)
            {
                if !(next_row.get_unchecked(condition_diagonal as usize).0 >= s1len as i32)
                    && i > threshold
                {
                    break usize::MAX;
                }

                break (i - 1) as usize;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::process::Output;

    use crate::{
        indexing::Indexer,
        parsing::{parse_single, tree_to_string, LabelDict, TreeOutput},
    };

    use super::*;

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
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        // avery
        let v2 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];

        let result = string_edit_distance_with_structure(&v2, &v1, 3);
        dbg!(&result);
        assert!(result <= 2);
        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 3);
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
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 7,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        // kitten
        let v2 = vec![
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 4,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 5,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 6,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];

        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 3);
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
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 1,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let result = string_edit_distance_with_structure(&v2, &v1, 1);
        assert_eq!(result, 4);

        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 1);
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
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 3,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let result = string_edit_distance_with_structure(&v1, &v2, 2);
        assert_eq!(result, 2);
        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 2);
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
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];

        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 1);
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
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 2,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 2,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];

        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 1);
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
            },
            TraversalCharacter {
                char: 2,
                preorder_descendant_postorder_ancestor: 0,
                preorder_following_postorder_preceding: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_descendant_postorder_ancestor: 0,
                preorder_following_postorder_preceding: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 3,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];

        let result = bounded_string_edit_distance_with_structure(&v1, &v2, 2);
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
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 2,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];

        let result = string_edit_distance_with_structure(&v1, &v2, 1);
        assert_eq!(result, 2);
    }

    #[test]
    fn test_sed_preorder_structure() {
        let t1str = "{a{a{b{a{a}}}}}".to_owned();
        let t2str = "{a{b{b{b}}{a{a}}}}".to_owned();
        let mut ld = LabelDict::new();
        let qt = parse_single(t1str, &mut ld);
        let tt = parse_single(t2str, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        dbg!(&qs);
        dbg!(&ts);

        assert_eq!(
            qs.preorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 4,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 3,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 2,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0,
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
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 3,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 2,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0,
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
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 4,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 2,
                    preorder_descendant_postorder_ancestor: 1,
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 2,
                    preorder_descendant_postorder_ancestor: 0,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1,
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0,
                },
            ]
        );
    }

    #[test]
    fn test_sed_query_and_tree() {
        let qstr = "{4{3{2 A}{3{2{3 rare}{2 and}}{3{2 lightly}{4 entertaining}}}}{3{2{2 look}{2{2 behind}{2{2{2 the}{2 curtain}}{2{2 that}{3{2{2 separates}{2 comics}}{3{2 from}{3{2{2 the}{2 people}}{3{4 laughing}{2{2 in}{2{2 the}{2 crowd}}}}}}}}}}}}}".to_owned();
        let tstr = "{3{2 Rehearsals}{2{2{2{2 are}{2 frequently}}{3{2 more}{3{3 fascinating}{2{2 than}{2{2 the}{2 results}}}}}}{2 .}}}"
            .to_owned();
        let mut ld = LabelDict::new();
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
        let mut ld = LabelDict::new();
        let qstr = "{a{b}{a{a}}}".to_owned();
        let tstr = "{b{b{a}}}".to_owned();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        let sed = string_edit_distance_with_structure(&ts.preorder, &qs.preorder, 3);
        let bsed = bounded_string_edit_distance_with_structure(&ts.preorder, &qs.preorder, 3);

        assert!(
            sed > bsed,
            "SED is not worse than bounded SED: {sed} <= {bsed}"
        );
    }

    #[test]
    fn test_sed_struct_correctness() {
        let qstr = "{a{a{a}{a}}{a{a}}}".to_owned();
        let tstr = "{a{a}{a{a}}}".to_owned();
        let mut ld = LabelDict::new();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        let v1 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 5,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 2,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 3,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 1,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 3,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 2,
                preorder_descendant_postorder_ancestor: 0,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 1,
            },
            TraversalCharacter {
                char: 1,
                preorder_following_postorder_preceding: 0,
                preorder_descendant_postorder_ancestor: 0,
            },
        ];
        assert_eq!(qs.preorder, v1);
        assert_eq!(ts.preorder, v2);
        let result = bounded_string_edit_distance_with_structure(&v2, &v1, 2);

        let result = sed_struct_k(&qs, &ts, 2);
        dbg!(result);
        assert_eq!(result, 2, "SED result is not as expected: {result} != 2");
    }

    #[test]
    fn test_sed_struct_correctness_2() {
        let qstr = "{a{b}{a{a}}}".to_owned();
        let tstr = "{a{a{a}}}".to_owned();
        let mut ld = LabelDict::new();
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
        let mut ld = LabelDict::new();
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
        let mut ld = LabelDict::new();
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
