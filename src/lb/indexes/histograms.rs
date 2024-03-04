use crate::parsing::{LabelDict, LabelId, ParsedTree};
use indextree::NodeId;
use std::collections::HashMap;
use itertools::Itertools;

type Histogram = HashMap<u32, u32>;

pub fn index_lookup(
    tree_collection: &[ParsedTree],
    label_dict: &LabelDict,
    k: usize,
) -> Vec<(usize, usize)> {
    // assumes tree collection is sorted by tree size
    let (leaf_hist, degree_hist, label_hist) = create_collection_histograms(tree_collection);
    let mut candidates = vec![];
    // this is the inverted index, that will be indexed by labelId, and contains a vector of pairs
    // (tree_id, labelId_count_in_tree)
    let mut il_index = vec![vec![]; label_dict.len() + 1];

    // label intersections counter for each tree. Counts with how many other trees it has an intersection
    // this is here to compute the symmetric difference faster
    let mut intersections_count = vec![0; label_hist.len()];

    for (tree_id, (tree_size, tree_label_histogram)) in label_hist.iter().enumerate() {
        let mut pre_candidates = vec![];

        // if the tree size is smaller than distance threshold k
        // we can safely increase all smaller trees intersections count
        if *tree_size <= k {
            intersections_count[..tree_id]
                .iter_mut()
                .enumerate()
                .for_each(|(other_tree_id, count)| {
                    pre_candidates.push(other_tree_id);
                    *count += 1
                });
        }

        // get pre-candidates by looking up the inverted index and doing the label intersection
        for (label_id, label_count) in tree_label_histogram.iter() {
            for (other_tree_id, other_label_count) in il_index[*label_id as usize].iter() {
                let intersection_size = *std::cmp::min(other_label_count, label_count);
                if intersections_count[*other_tree_id] == 0 && intersection_size > 0 {
                    pre_candidates.push(*other_tree_id);
                }
                intersections_count[*other_tree_id] = std::cmp::min(
                    intersections_count[*other_tree_id] + intersection_size,
                    *tree_size as u32
                )
            }
            il_index[*label_id as usize].push((tree_id, *label_count));
        }

        // verify precandidates
        for pre_cand_id in pre_candidates.iter() {
            let other_tree_size = label_hist[*pre_cand_id].0;
            // compute the symmetric difference (union - intersection size) and divide by 2 to get the label lower bound
            // if (tree_size + other_tree_size - (2 * intersections_count[*pre_cand_id] as usize)) / 2 <= k {
            //     candidates.push((tree_id, *pre_cand_id));
            // }

            if std::cmp::max(*tree_size, other_tree_size) - intersections_count[*pre_cand_id] as usize <= k {
                candidates.push((tree_id, *pre_cand_id));
            }

            intersections_count[*pre_cand_id] = 0;
        }
    }


    let candidates = candidates.iter().cloned()
        .filter(|(t1, t2)| {
            let intersection_size = degree_hist[*t1].1.iter()
                .fold(0, |intersection, (degree, count)| {
                intersection + std::cmp::min(
                    count,
                    degree_hist[*t2].1.get(degree).unwrap_or(&0)
                )
            }) as usize;

            let (t1size, t2size) = (
                degree_hist[*t1].0,
                degree_hist[*t2].0
                );
            (t1size + t2size - (2 * intersection_size)) / 5 <= k
        })
        .filter(|(t1, t2)| {
            let intersection_size = leaf_hist[*t1].1.iter()
                .fold(0, |intersection, (degree, count)| {
                intersection + std::cmp::min(
                    count,
                    leaf_hist[*t2].1.get(degree).unwrap_or(&0)
                )
            }) as usize;

            let (t1size, t2size) = (
                degree_hist[*t1].0,
                degree_hist[*t2].0
            );

            ((t1size + t2size) - (2 * intersection_size)) <= k
        })
        .collect();

    candidates
}


/// Creates and returns Leaf, Degree and Label histogram collections
/// the first usize in vec pair is the tree size
pub fn create_collection_histograms(
    tree_collection: &[ParsedTree],
) -> (
    Vec<(usize, Histogram)>,
    Vec<(usize, Histogram)>,
    Vec<(usize, HashMap<LabelId, u32>)>,
) {
    let (mut leaf_hists, mut degree_hists, mut label_hists) = (
        Vec::with_capacity(tree_collection.len()),
        Vec::with_capacity(tree_collection.len()),
        Vec::with_capacity(tree_collection.len()),
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
pub fn create_tree_histograms(tree: &ParsedTree) -> (Histogram, Histogram, HashMap<LabelId, u32>) {
    let Some(root) = tree.iter().next() else {
        panic!("Unable to get tree root, but tree is not empty!");
    };
    let (mut label, mut degree, mut leaf) = (
        HashMap::<LabelId, u32>::new(),
        Histogram::new(),
        Histogram::new(),
    );
    let root_id = tree.get_node_id(root).unwrap();
    traverse_tree(&root_id, tree, &mut label, &mut degree, &mut leaf);

    (leaf, degree, label)
}

fn traverse_tree(
    node_id: &NodeId,
    tree: &ParsedTree,
    label_hist: &mut HashMap<LabelId, u32>,
    degree_hist: &mut Histogram,
    leaf_hist: &mut Histogram,
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
    degree_hist
        .entry(degree)
        .and_modify(|count| *count += 1)
        .or_insert(1);
    max_child_leaf_dist += 1;
    leaf_hist
        .entry(max_child_leaf_dist)
        .and_modify(|count| *count += 1)
        .or_insert(1);

    let label = tree.get(*node_id).unwrap().get();
    label_hist
        .entry(*label)
        .and_modify(|count| *count += 1)
        .or_insert(1);
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

        let (leaf, degree, label) = create_tree_histograms(&pt);

        assert_eq!(leaf, HashMap::from([(1, 5), (2, 2), (3, 1), (4, 1),]));

        assert_eq!(degree, HashMap::from([(0, 5), (1, 1), (2, 2), (3, 1),]));

        assert_eq!(
            label,
            HashMap::from([(0, 1), (1, 2), (2, 2), (3, 1), (4, 1), (5, 1), (6, 1),])
        );
    }
}
