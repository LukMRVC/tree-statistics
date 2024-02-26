use crate::indexing::InvertedListLabelPostorderIndex;

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::*;
    use crate::indexing::{Indexer, InvertedListLabelPostorderIndex};

    #[test]
    fn test_lblint() {
        let mut ld = LabelDict::new();

        let t2 = parse_tree(Ok(
            "{b{e}{d{a}}}".to_owned()
        ), &mut ld).unwrap();
        let t3 = parse_tree(Ok(
            "{d{c}{b{a}{d{a}}}}".to_owned()
        ), &mut ld).unwrap();
        let t5 = parse_tree(Ok(
            "{a{b{a}{c{d}}}{d}}".to_owned()
        ), &mut ld).unwrap();

        let t2i = InvertedListLabelPostorderIndex::index_tree(&t2, &ld);
        let t3i = InvertedListLabelPostorderIndex::index_tree(&t3, &ld);
        let t5i = InvertedListLabelPostorderIndex::index_tree(&t5, &ld);

        let t2t3_lb = label_intersection(&t2i, &t3i);
        let t3t5_lb = label_intersection(&t3i, &t5i);

        assert_eq!(3, t2t3_lb, "Label diff between t2 and t3 should be 2!");
        assert_eq!(0, t3t5_lb, "Label diff between t3 and t5 should be 0!");
    }

}