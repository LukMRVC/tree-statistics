use itertools::Itertools;
use rustc_hash::FxHashMap;

use crate::{indexing::InvertedListLabelPostorderIndex, parsing::LabelId};

pub fn label_intersection(
    t1: &InvertedListLabelPostorderIndex,
    t2: &InvertedListLabelPostorderIndex,
) -> usize {
    use std::cmp::{max, min};
    let mut intersection_size = 0;
    for (label, postings) in t1.inverted_list.iter() {
        if let Some(t2postings) = t2.inverted_list.get(label) {
            intersection_size += min(t2postings.len(), postings.len());
        }
    }

    max(t1.c.tree_size, t2.c.tree_size) - intersection_size
}

pub fn label_intersection_k(
    t1: &InvertedListLabelPostorderIndex,
    t2: &InvertedListLabelPostorderIndex,
    k: usize,
) -> usize {
    use std::cmp::{max, min};
    let mut intersection_size = 0;
    let bigger_tree = max(t1.c.tree_size, t2.c.tree_size);

    // if all labels matched, but just the size difference was too much, just exit
    if t1.c.tree_size.abs_diff(t2.c.tree_size) > k {
        return k + 1;
    }

    for (label, postings) in t1.inverted_list.iter() {
        let Some(t2postings) = t2.inverted_list.get(label) else {
            continue;
        };
        intersection_size += min(t2postings.len(), postings.len());

        if bigger_tree - intersection_size < k {
            return bigger_tree - intersection_size;
        }
    }

    bigger_tree - intersection_size
}

pub struct LabelIntersectionIndex {
    index: FxHashMap<LabelId, Vec<(usize, usize, usize)>>,
    size_index: Vec<usize>,
}

impl LabelIntersectionIndex {
    // asserts trees are in sorted order by tree size when creating a new index
    pub fn new(trees: &[InvertedListLabelPostorderIndex]) -> Self {
        let mut index: FxHashMap<LabelId, Vec<(usize, usize, usize)>> = FxHashMap::default();
        let mut size_index = vec![];
        for (tid, t) in trees.iter().enumerate() {
            for (label, lbl_count) in t.inverted_list.iter() {
                index
                    .entry(*label)
                    .and_modify(|postings| postings.push((tid, t.c.tree_size, lbl_count.len())))
                    .or_insert(vec![(tid, t.c.tree_size, lbl_count.len())]);
            }
            size_index.push(t.c.tree_size);
        }

        LabelIntersectionIndex { index, size_index }
    }

    pub fn query_index(
        &self,
        query_tree: &InvertedListLabelPostorderIndex,
        k: usize,
        query_id: Option<usize>,
    ) -> Vec<(usize, usize)> {
        let query_id = query_id.unwrap_or(0);
        // for each TID stores the current intersection size
        let mut tree_intersections = FxHashMap::default();
        for (lbl, query_label_cnt) in query_tree.inverted_list.iter() {
            let query_label_cnt = query_label_cnt.len();
            if let Some(posting_list) = self.index.get(&lbl) {
                for (tid, tree_size, label_cnt) in posting_list
                    .iter()
                    .skip_while(|(_, size, _)| query_tree.c.tree_size.abs_diff(*size) > k)
                    .take_while(|(_, size, _)| query_tree.c.tree_size.abs_diff(*size) <= k)
                {
                    tree_intersections
                        .entry(*tid)
                        .and_modify(|(intersection_size, _)| {
                            *intersection_size += std::cmp::min(query_label_cnt, *label_cnt);
                        })
                        .or_insert((std::cmp::min(query_label_cnt, *label_cnt), *tree_size));
                }
            }
        }

        let mut candidates = vec![];
        // find candidates that have no label overlap but can fit by size because of threshold
        for (cid, tree_size) in self
            .size_index
            .iter()
            .enumerate()
            .take_while(|(_, ts)| !(query_tree.c.tree_size.abs_diff(**ts) > k))
        {
            if let None = tree_intersections.get(&cid) {
                if std::cmp::max(query_tree.c.tree_size, *tree_size) <= k {
                    candidates.push((query_id, cid));
                }
            }
        }
        candidates.extend(
            tree_intersections
                .iter()
                .filter(|(_, (intersection_size, tree_size))| {
                    std::cmp::max(query_tree.c.tree_size, *tree_size) - intersection_size <= k
                })
                .map(|(tid, _)| (query_id, *tid)),
        );
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::{Indexer, InvertedListLabelPostorderIndex};
    use crate::parsing::*;

    #[test]
    fn test_lblint() {
        let mut ld = LabelDict::new();

        let t2 = parse_tree(Ok("{b{e}{d{a}}}".to_owned()), &mut ld).unwrap();
        let t3 = parse_tree(Ok("{d{c}{b{a}{d{a}}}}".to_owned()), &mut ld).unwrap();
        let t5 = parse_tree(Ok("{a{b{a}{c{d}}}{d}}".to_owned()), &mut ld).unwrap();

        let t2i = InvertedListLabelPostorderIndex::index_tree(&t2, &ld);
        let t3i = InvertedListLabelPostorderIndex::index_tree(&t3, &ld);
        let t5i = InvertedListLabelPostorderIndex::index_tree(&t5, &ld);

        let t2t3_lb = label_intersection(&t2i, &t3i);
        let t3t5_lb = label_intersection(&t3i, &t5i);

        assert_eq!(3, t2t3_lb, "Label diff between t2 and t3 should be 2!");
        assert_eq!(0, t3t5_lb, "Label diff between t3 and t5 should be 0!");
    }

    #[test]
    fn test_missing_label_lb() {
        let i1 = "{pietro gobetti str.{8}{10}}".to_owned();
        let i2 = "{wendelsteinstrasse{1{{1}{2}{3}{4}{5}{6}{7}{14}}}}".to_owned();
        let mut ld = LabelDict::new();
        let t1 = parse_tree(Ok(i1), &mut ld).unwrap();
        let t2 = parse_tree(Ok(i2), &mut ld).unwrap();

        let t1i = InvertedListLabelPostorderIndex::index_tree(&t1, &ld);
        let t2i = InvertedListLabelPostorderIndex::index_tree(&t2, &ld);

        let lb = label_intersection(&t1i, &t2i);

        assert_eq!(lb, 11, "Lower bound is 10");
    }

    #[test]
    fn test_correctness_index() {
        let i = "{0{1 Abysmally}{0 pathetic}}".to_owned();
        let q = "{3{2{2 Unfolds}{3{2 in}{2{2{2{2 a}{2 series}}{2{2 of}{2{2 achronological}{2 vignettes}}}}{3{2{2{2 whose}{2 cumulative}}{2 effect}}{2{2 is}{3 chilling}}}}}}{2 .}}".to_owned();
        let mut ld = LabelDict::new();
        let t1 = parse_tree(Ok(i), &mut ld).unwrap();
        let t2 = parse_tree(Ok(q), &mut ld).unwrap();
        let t1i = InvertedListLabelPostorderIndex::index_tree(&t1, &ld);
        let t2i = InvertedListLabelPostorderIndex::index_tree(&t2, &ld);

        let lb = label_intersection_k(&t1i, &t2i, 25);
        assert!(lb <= 25, "Lower bound is less than 25");

        let lblint_index = LabelIntersectionIndex::new(&vec![t1i]);
        let candidates = lblint_index.query_index(&t2i, 25, Some(0));
        assert_eq!(candidates.len(), 1, "No candidates found")
    }
}
