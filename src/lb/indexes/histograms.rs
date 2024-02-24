use std::collections::HashMap;
use indextree::NodeId;
use crate::parsing::{LabelDict, LabelId, ParsedTree};

type Histogram = HashMap<u32, u32>;

pub fn create_collection_histograms(tree_collection: &[ParsedTree]) -> (
    Vec<(usize, Histogram)>,
    Vec<(usize, Histogram)>,
    Vec<(usize, HashMap<LabelId, u32>)>,
) {
    let (
        mut leaf_hists,
        mut degree_hists,
        mut label_hists
    ) = (
        Vec::with_capacity(tree_collection.len()),
        Vec::with_capacity(tree_collection.len()),
        Vec::with_capacity(tree_collection.len())
    );

    tree_collection.iter().for_each(|tree| {
        let (leaf, degree, label) = create_tree_histograms(tree);
        leaf_hists.push((tree.count(), leaf));
        degree_hists.push((tree.count(), degree));
        label_hists.push((tree.count(), label));
    });

    return (leaf_hists, degree_hists, label_hists);
}

/// Creates and returns Leaf, Degree and Label histograms respectively
pub fn create_tree_histograms(tree: &ParsedTree) -> (
    Histogram,
    Histogram,
    HashMap<LabelId, u32>,
) {
    let Some(root) = tree.iter().next() else {
        panic!("Unable to get tree root, but tree is not empty!");
    };
    let (
        mut label,
        mut degree,
        mut leaf
    ) = (HashMap::<LabelId, u32>::new(), Histogram::new(), Histogram::new());
    let root_id = tree.get_node_id(root).unwrap();
    traverse_tree(&root_id, tree, &mut label, &mut degree, &mut leaf);

    (leaf, degree, label)
}

fn traverse_tree(
    node_id: &NodeId,
    tree: &ParsedTree,
    label_hist: &mut HashMap<LabelId, u32>,
    degree_hist: &mut Histogram,
    leaf_hist: &mut Histogram
) -> u32 {
    use std::cmp::max;
    // Degree histogram is simple - it's just number of children
    // Leaf distance histogram - Leaf distance is the maximum distance from current node
    // to any of its children leaf + 1
    let children_iter = node_id.children(tree);
    let mut degree = 0;
    let mut max_child_leaf_dist = 0;
    for cnid in children_iter {
        degree += 1;
        let child_dist = traverse_tree(&cnid, tree, label_hist, degree_hist, leaf_hist);
        max_child_leaf_dist = max(max_child_leaf_dist, child_dist);
    }
    degree_hist.entry(degree).and_modify(|count| { *count += 1 }).or_insert(1);
    max_child_leaf_dist += 1;
    leaf_hist.entry(max_child_leaf_dist).and_modify(|count| { *count += 1}).or_insert(1);

    let label = tree.get(*node_id).unwrap().get();
    label_hist.entry(*label).and_modify(|count| { *count += 1}).or_insert(1);
    max_child_leaf_dist
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::parse_tree;

    #[test]
    fn test_histogram_traversals() {
        let tree_str = "{a{b{c}{d{c}}{b}}{f{g}{x}}}".to_owned();
        let mut ld = LabelDict::new();
        let pt = parse_tree(Ok(tree_str), &mut ld).unwrap();

        let (leaf, degree, label) = create_histograms(&pt);

        assert_eq!(leaf, HashMap::from([
            (1, 5),
            (2, 2),
            (3, 1),
            (4, 1),
        ]));

        assert_eq!(degree, HashMap::from([
            (0, 5),
            (1, 1),
            (2, 2),
            (3, 1),
        ]));

        assert_eq!(label, HashMap::from([
            (0, 1),
            (1, 2),
            (2, 2),
            (3, 1),
            (4, 1),
            (5, 1),
            (6, 1),
        ]));
    }
}