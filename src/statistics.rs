use crate::parsing::{LabelFreqOrdering, ParsedTree};

use itertools::Itertools;
use num_traits::Num;
use rayon::prelude::*;
use std::fmt;
use std::fmt::Formatter;
use std::iter::Sum;
use std::num::NonZeroUsize;

#[derive(Default, Debug, Clone)]
pub struct TreeStatistics {
    /// Slice of degrees of tree - useful for histograms and average degree
    pub degrees: Vec<usize>,
    /// Tree depths - length of each path from root to leaf
    pub depths: Vec<usize>,
    /// number of nodes in a tree
    pub size: usize,
    /// distinct labels in current tree
    pub distinct_labels: usize,
}

#[derive(Default, Debug, Clone)]
pub struct CollectionStatistics {
    /// min tree size in collection
    pub min_tree_size: usize,
    /// max tree size in collection
    pub max_tree_size: usize,
    /// average number of nodes per tree in collection
    pub avg_tree_size: f64,
    /// number of distinct labels in collection
    pub trees: usize,
    /// distinct labels per tree
    pub avg_distinct_label_per_tree: f64,
}

impl fmt::Display for CollectionStatistics {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{},{},{:.6},{},{:.6}",
            self.min_tree_size,
            self.max_tree_size,
            self.avg_tree_size,
            self.trees,
            self.avg_distinct_label_per_tree,
        )
    }
}

pub fn gather(tree: &ParsedTree, freq_ordering: &LabelFreqOrdering) -> TreeStatistics {
    if tree.is_empty() {
        return TreeStatistics::default();
    }

    let Some(root) = tree.iter().next() else {
        panic!("Unable to get root but tree is not empty!");
    };

    let mut node_stack = vec![];

    let root_id = tree.get_node_id(root).unwrap();
    let mut degrees = vec![];
    let mut depths = vec![];
    let mut distinct_labels = 0;

    if let Some(&freq) = freq_ordering.get(NonZeroUsize::new(*root.get() as usize).unwrap()) {
        distinct_labels += usize::from(freq == 1);
    }

    #[inline]
    fn is_leaf(children: &usize) -> bool {
        *children == 0
    }

    for nid in root_id.descendants(tree) {
        let n = tree.get(nid).unwrap();
        let mut degree = nid.children(tree).count();

        if let Some(&freq) = freq_ordering.get(NonZeroUsize::new(*n.get() as usize).unwrap()) {
            distinct_labels += usize::from(freq == 1);
        }

        // pop node ids from stack to get into
        while !node_stack.is_empty()
            && *node_stack.last().unwrap() != tree.get(nid).unwrap().parent().unwrap()
        {
            node_stack.pop();
        }

        if is_leaf(&degree) {
            depths.push(node_stack.len());
        } else {
            node_stack.push(nid);
        }

        degree += if n.parent().is_some() { 1 } else { 0 };
        degrees.push(degree);
    }

    TreeStatistics {
        degrees,
        depths,
        size: tree.count(),
        distinct_labels,
    }
}

pub fn summarize(all_statistics: &[TreeStatistics]) -> CollectionStatistics {
    use itertools::MinMaxResult as MMR;

    let (min, max) = match all_statistics.iter().minmax_by_key(|s| s.size) {
        MMR::NoElements => (0, 0),
        MMR::OneElement(m) => (m.size, m.size),
        MMR::MinMax(mi, mx) => (mi.size, mx.size),
    };

    let avg_size = all_statistics.par_iter().map(|s| s.size).sum::<usize>() as f64
        / all_statistics.len() as f64;
    let avg_distinct_per_tree = all_statistics
        .par_iter()
        .map(|s| s.distinct_labels)
        .sum::<usize>() as f64
        / all_statistics.len() as f64;

    CollectionStatistics {
        min_tree_size: min,
        max_tree_size: max,
        avg_tree_size: avg_size,
        trees: all_statistics.len(),
        avg_distinct_label_per_tree: avg_distinct_per_tree,
    }
}

pub fn mean<T>(list: &[T]) -> f64
where
    T: Num + Sum + Copy,
    f64: Sum<T>,
{
    list.iter().copied().sum::<f64>() / list.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use indextree::Arena;

    #[test]
    fn test_simple_statistics() {
        let mut a = Arena::new();
        let n1 = a.new_node(1);
        let n2 = a.new_node(2);
        let n3 = a.new_node(3);
        let n4 = a.new_node(4);

        n1.append(n2, &mut a);
        n2.append(n3, &mut a);
        n3.append(n4, &mut a);
        let ordering = LabelFreqOrdering::new(vec![1, 1, 1, 1]);
        let stats = gather(&a, &ordering);

        assert_eq!(stats.depths, vec![3]);
        assert_eq!(stats.degrees, vec![1, 2, 2, 1]);
        assert_eq!(stats.size, 4);
    }

    #[test]
    fn test_branched_stats() {
        let mut a = Arena::new();
        let n1 = a.new_node(1);
        let n2 = a.new_node(2);
        let n3 = a.new_node(3);
        let n4 = a.new_node(4);
        let n5 = a.new_node(3);
        let n6 = a.new_node(2);
        let n7 = a.new_node(5);

        n1.append(n2, &mut a);
        n2.append(n3, &mut a);
        n3.append(n4, &mut a);
        n3.append(n5, &mut a);

        n1.append(n6, &mut a);
        n6.append(n7, &mut a);
        let ordering = LabelFreqOrdering::new(vec![1, 2, 2, 1, 1]);

        let stats = gather(&a, &ordering);

        assert_eq!(stats.depths, vec![3, 3, 2]);
        assert_eq!(stats.degrees, vec![2, 2, 3, 1, 1, 2, 1]);
    }
}
