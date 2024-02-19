use crate::indexing::SEDIndex;

pub fn sed(t1: &SEDIndex, t2: &SEDIndex) -> usize {
    let pre_dist = string_edit_distance(&t1.preorder, &t2.preorder);
    let post_dist = string_edit_distance(&t1.postorder, &t2.postorder);

    std::cmp::max(pre_dist, post_dist)
}

fn string_edit_distance(s1: &[i32], s2: &[i32]) -> usize {
    use std::cmp::min;

    let (mut s1, mut s2) = (s1, s2);
    if s1.len() > s2.len() {
        (s1, s2) = (s2, s1);
    }
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

pub fn sed_k(t1: &SEDIndex, t2: &SEDIndex, k: usize) -> usize {
    0
}