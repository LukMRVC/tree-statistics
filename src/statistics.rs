use std::collections::HashSet;
use std::num::{NonZeroU32};
use indextree::Arena;

#[derive(Default, Debug)]
struct TreeStatistics<'a> {
    /// Slice of degrees of tree - useful for histograms and average degree
    degrees: Vec<NonZeroU32>,
    /// Tree depths - length of each path from root to leaf
    depths: Vec<NonZeroU32>,
    /// distinct labels in a tree
    distinct_labels: HashSet<&'a str>,
}


pub fn gather<'a>(tree: &Arena<&'a str>) -> TreeStatistics<'a> {
    if tree.is_empty() {
        return TreeStatistics::default();
    }

    let Some(root) = tree.iter().next() else {
        panic!("Unable to get root but tree is not empty!");
    };

    let root_id = tree.get_node_id(root).unwrap();

    TreeStatistics {
        degrees: vec![],
        depths: vec![],
        distinct_labels: HashSet::default(),
    }
}
