use crate::indexing::InvertedListLabelPostorderIndex;

pub fn label_intersection(t1: &InvertedListLabelPostorderIndex, t2: &InvertedListLabelPostorderIndex) -> usize {
    use std::cmp::{max, min};
    let mut intersection_size = 0;
    for (label, postings) in t1.inverted_list.iter() {
        let Some(t2postings) = t2.inverted_list.get(label) else {
            continue;
        };
        intersection_size += min(t2postings.len(), postings.len());
    }

    max(t1.c.tree_size, t2.c.tree_size) - intersection_size
}


pub fn label_intersection_k(t1: &InvertedListLabelPostorderIndex, t2: &InvertedListLabelPostorderIndex, k: usize) -> usize {
    use std::cmp::{max, min};
    let mut intersection_size = 0;
    let bigger_tree = max(t1.c.tree_size, t2.c.tree_size);
    for (label, postings) in t1.inverted_list.iter() {
        let Some(t2postings) = t2.inverted_list.get(label) else {
            continue;
        };
        intersection_size += min(t2postings.len(), postings.len());

        if bigger_tree - intersection_size < k {
            return bigger_tree - intersection_size
        }
    }

    bigger_tree - intersection_size
}
