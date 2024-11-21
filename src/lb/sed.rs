use crate::indexing::SEDIndex;

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
        result = i + 1;
        let mut dist_b = i;

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
pub fn sed_k(t1: &SEDIndex, t2: &SEDIndex, k: usize) -> usize {
    let (mut t1, mut t2) = (t1, t2);
    // if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
    //     return k + 1;
    // }

    if t1.preorder.len() > t2.preorder.len() {
        (t1, t2) = (t2, t1);
    }
    let k = k + 1;
    let pre_dist = bounded_string_edit_distance(&t1.preorder, &t2.preorder, k);

    if pre_dist > k {
        return pre_dist;
    }

    let post_dist = bounded_string_edit_distance(&t1.postorder, &t2.postorder, k);

    std::cmp::max(pre_dist, post_dist)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sed() {
        let v1 = vec![1, 2, 3, 4, 5, 5, 6];
        let v2 = vec![1, 2, 3, 5, 6, 7, 6];

        let result = string_edit_distance(&v1, &v2);
        assert_eq!(result, 3);
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
