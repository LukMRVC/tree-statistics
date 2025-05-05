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
    let pre_dist = string_edit_distance_with_structure(&t1.preorder, &t2.preorder, k as u32);

    // if pre_dist > k {
    //     return pre_dist;
    // }

    // let post_dist = string_edit_distance_with_structure(&t1.postorder, &t2.postorder, k as u32 + 1);
    return pre_dist;
    // std::cmp::max(pre_dist, post_dist)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TraversalCharacter {
    pub char: i32,
    pub following: i32,
}

/// Implements fastest known way to compute exact string edit between two strings
fn string_edit_distance_with_structure(
    s1: &[TraversalCharacter],
    s2: &[TraversalCharacter],
    k: u32,
) -> usize {
    use std::cmp::min;
    // assumes size of s2 is smaller or equal than s1
    let s2len = s2.len();
    // let mut matrix = vec![];
    let mut cache: Vec<usize> = (1..s2len + 1).collect();
    // matrix.push(cache.clone());
    // dbg!(&cache);
    let mut result = s2len;
    for (i, ca) in s1.iter().enumerate() {
        let mut insert_dist = i;
        result = i + 1;

        for (j, cb) in s2.iter().enumerate() {
            let mut replace_dist = insert_dist + usize::from(ca.char != cb.char);
            unsafe {
                // TODO: If ca.info.abs_diff(cb.info) > k mark the cell as invalid, thus no computations
                // can be done from that cell
                insert_dist = *cache.get_unchecked(j);
                result = min(
                    replace_dist + usize::from(ca.following.abs_diff(cb.following) > k),
                    min(insert_dist + 1, result + 1),
                );

                // result = match (replace_dist, insert_dist, result) {
                //     (usize::MAX, usize::MAX, usize::MAX) => continue, //usize::MAX,
                //     (usize::MAX, usize::MAX, res) => res + 1,
                //     (usize::MAX, ins, usize::MAX) => ins + 1,
                //     (repl, usize::MAX, usize::MAX) => repl,
                //     (repl, usize::MAX, res) => min(res + 1, repl),
                //     (repl, ins, usize::MAX) => min(repl, ins + 1),
                //     (usize::MAX, ins, res) => min(res + 1, ins + 1),
                //     (repl, ins, res) => min(min(repl, ins + 1), res + 1),
                // };

                *cache.get_unchecked_mut(j) = result;
                // if result != usize::MAX && ca.following.abs_diff(cb.following) > k {
                //     // dbg!(&ca, &cb);
                //     // result = result + 1;
                //     *cache.get_unchecked_mut(j) = usize::MAX;
                // }
            }
        }
        // matrix.push(cache.clone());
        // dbg!(&cache);
    }

    // Print matrix by columns
    // println!("Matrix by columns:");
    // print!("Row    S1   ");
    // for c in s1.iter() {
    //     print!("  {:>3}", c.char);
    // }
    // println!("");
    // for j in 0..cache.len() {
    //     print!("Row  {:>3}: [", s2[j].char);
    //     for i in 0..matrix.len() {
    //         if i > 0 {
    //             print!(",");
    //         }
    //         print!("{:>3}", matrix[i][j]);
    //     }
    //     println!("]");
    // }
    // dbg!(&matrix);

    result
}

/// Computes bounded string edit distance with known maximal threshold.
/// Returns distance at max of K. Algorithm by Hal Berghel and David Roach
pub fn sed_k(t1: &SEDIndex, t2: &SEDIndex, k: usize) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    if t1.preorder.len() > t2.preorder.len() {
        (t1, t2) = (t2, t1);
    }
    let k = k + 1;
    let pre_dist = bounded_string_edit_distance(&t1.preorder, &t2.preorder, k);

    // if pre_dist > k {
    //     return pre_dist;
    // }

    // let post_dist = bounded_string_edit_distance(&t1.postorder, &t2.postorder, k);

    // std::cmp::max(pre_dist, post_dist)
    // post_dist
    pre_dist
}

pub fn bounded_string_edit_distance(s1: &[i32], s2: &[i32], k: usize) -> usize {
    use std::cmp::{max, min};
    // assumes size of s2 is smaller or equal than s1
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
                break (i - 1) as usize;
            }
        }
    }
}

pub fn bounded_string_edit_distance_with_structure(
    s1: &[i32],
    s2: &[i32],
    info1: &[i32],
    info2: &[i32],
    k: usize,
) -> usize {
    use std::cmp::{max, min};
    // assumes size of s2 is smaller or equal than s1
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
    let info1 = &info1[common_prefix..s1len];
    let info2 = &info2[common_prefix..s2len];

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
                following: 0,
            },
            TraversalCharacter {
                char: 2,
                following: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                following: 0,
            },
            TraversalCharacter {
                char: 2,
                following: 0,
            },
            TraversalCharacter {
                char: 3,
                following: 0,
            },
        ];

        let result = string_edit_distance_with_structure(&v1, &v2, 2);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_sed_wt_structure() {
        // preorder traversal of simple tree with info about preceding nodes
        let v1: Vec<TraversalCharacter> = vec![
            TraversalCharacter {
                char: 1,
                following: 0,
            },
            TraversalCharacter {
                char: 1,
                following: 0,
            },
            TraversalCharacter {
                char: 2,
                following: 0,
            },
            TraversalCharacter {
                char: 1,
                following: 0,
            },
            TraversalCharacter {
                char: 1,
                following: 0,
            },
        ];
        let v2 = vec![
            TraversalCharacter {
                char: 1,
                following: 0,
            },
            TraversalCharacter {
                char: 2,
                following: 0,
            },
            TraversalCharacter {
                char: 2,
                following: 0,
            },
            TraversalCharacter {
                char: 1,
                following: 0,
            },
            TraversalCharacter {
                char: 1,
                following: 2,
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
        assert_eq!(
            qs.preorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
                TraversalCharacter {
                    char: 2,
                    following: 0
                },
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
            ]
        );
        assert_eq!(
            ts.preorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
                TraversalCharacter {
                    char: 2,
                    following: 0
                },
                TraversalCharacter {
                    char: 2,
                    following: 2
                },
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
                TraversalCharacter {
                    char: 1,
                    following: 0
                },
            ]
        );

        let result = sed_struct_k(&qs, &ts, 1);
        assert_eq!(result, 3);
    }

    #[test]
    fn test_sed_query_and_tree() {
        let qstr = "{4{3{2 A}{3{2{3 rare}{2 and}}{3{2 lightly}{4 entertaining}}}}{3{2{2 look}{2{2 behind}{2{2{2 the}{2 curtain}}{2{2 that}{3{2{2 separates}{2 comics}}{3{2 from}{3{2{2 the}{2 people}}{3{4 laughing}{2{2 in}{2{2 the}{2 crowd}}}}}}}}}}}}}".to_owned();
        let tstr = "{2{2 Who}{2{2{2 is}{2{2{2 the}{2 audience}}{2{2 for}{2{2 Cletis}{2 Tout}}}}}}}"
            .to_owned();
        let mut ld = LabelDict::new();
        let qt = parse_single(qstr, &mut ld);
        let tt = parse_single(tstr, &mut ld);
        dbg!(tree_to_string(&qt, TreeOutput::BracketNotation));
        dbg!(tree_to_string(&tt, TreeOutput::BracketNotation));

        let qs = SEDIndexWithStructure::index_tree(&qt, &ld);
        let ts = SEDIndexWithStructure::index_tree(&tt, &ld);

        let result = sed_struct_k(&qs, &ts, 30);

        assert!(result <= 30, "SED result is not as expected: {result} > 30");
    }

    #[test]
    fn test_sed_struct_correctness() {
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
        assert_eq!(result, 2);

        let result = bounded_string_edit_distance(&v1, &v2, 4);
        assert_eq!(result, 3);
    }
}
