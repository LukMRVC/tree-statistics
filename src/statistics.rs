use crate::parsing::ParsedTree;

use itertools::Itertools;
use num_traits::Num;
use rayon::prelude::*;
use std::fmt;
use std::fmt::Formatter;
use std::iter::Sum;

#[derive(Default, Debug, Clone)]
pub struct TreeStatistics {
    /// Slice of degrees of tree - useful for histograms and average degree
    pub degrees: Vec<usize>,
    /// Tree depths - length of each path from root to leaf
    pub depths: Vec<usize>,
    /// number of nodes in a tree
    pub size: usize,
    /// average node degree
    pub avg_degree: f64,
    /// average path len from root to leaf,
    pub avg_depth: f64,
    /// Max path from root to leaf
    pub height: usize,
}

#[derive(Default, Debug, Clone)]
pub struct CollectionStatistics {
    /// min tree size in collection
    pub min_tree_size: usize,
    /// max tree size in collection
    pub max_tree_size: usize,
    /// average number of nodes per tree in collection
    pub avg_tree_size: usize,
    /// number of distinct labels in collection
    pub trees: usize,
}

impl fmt::Display for CollectionStatistics {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{},{},{},{}",
            self.min_tree_size, self.max_tree_size, self.avg_tree_size, self.trees
        )
    }
}

pub fn gather(tree: &ParsedTree) -> TreeStatistics {
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

    #[inline]
    fn is_leaf(children: &usize) -> bool {
        *children == 0
    }

    for nid in root_id.descendants(tree) {
        let n = tree.get(nid).unwrap();
        let mut degree = nid.children(tree).count();

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

    let height = depths.iter().max().copied().unwrap_or(0);
    let avg_degree = degrees.iter().sum::<usize>() as f64 / degrees.len() as f64;
    let avg_depth = depths.iter().sum::<usize>() as f64 / depths.len() as f64;

    TreeStatistics {
        degrees,
        depths,
        size: tree.count(),
        height,
        avg_degree,
        avg_depth,
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
    let avg_size = avg_size.round() as usize;

    CollectionStatistics {
        min_tree_size: min,
        max_tree_size: max,
        avg_tree_size: avg_size,
        trees: all_statistics.len(),
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
        let stats = gather(&a);

        assert_eq!(stats.depths, vec![3]);
        assert_eq!(stats.degrees, vec![1, 2, 2, 1]);
        assert_eq!(stats.height, 3);
        assert_eq!(stats.avg_depth, 3f64);
        assert_eq!(stats.avg_degree, 1.5f64);
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

        let stats = gather(&a);

        assert_eq!(stats.depths, vec![3, 3, 2]);
        assert_eq!(stats.degrees, vec![2, 2, 3, 1, 1, 2, 1]);
    }
}
