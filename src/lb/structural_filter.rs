use crate::parsing::{LabelDict, LabelId, ParsedTree};
use indextree::NodeId;
use itertools::Itertools;
use std::collections::HashMap;
use std::collections::BTreeMap;
use rustc_hash::{FxHashMap, FxHashSet};


type StructHashMap = BTreeMap<LabelId, LabelSetElement>;
type StructHashMapKeys = FxHashSet<LabelId>;

/// The building block for structural filter, holds information about
/// the count of ancestral nodes, descendants nodes, to the left and to the right
// difference between children and descendants? Children nodes are only 1 level below current node level
// while descendants are all nodes below the current node
#[derive(Debug, Default)]
pub struct StructuralVec {
    /// Id of postorder tree traversal
    pub postorder_id: usize,
    /// Number of nodes to left of this node
    pub nodes_left: usize,
    /// Number of nodes to right of this node
    pub nodes_right: usize,
    /// Number of ancestral nodes
    pub nodes_ancestors: usize,
    /// Number of descendants nodes
    pub nodes_descendants: usize,
}

/// This is an element holding relevant data of a set.

#[derive(Debug, Default)]
pub struct LabelSetElement {
    pub id: LabelId,
    pub weight: usize,
    pub weigh_so_far: usize,

    pub struct_vec: Vec<StructuralVec>,
}

/// Base struct tuple for structural filter
pub struct StructuralFilterTuple(usize, StructHashMap, StructHashMapKeys);

/// Takes a collection of trees and converts them into a collection of label
/// sets. A label set consists of labels and each label holds all nodes with that
/// label. The labels are substituted with their inverted label frequency number.
/// The labels in the sets are sorted by the global inverted frequency ordering
/// of the input collection.
#[derive(Debug, Default)]
pub struct LabelSetConverter {
    actual_depth: usize,
    actual_pre_order_number: usize,
    next_token_id: usize,
}

impl LabelSetConverter {
    pub fn create(&mut self, trees: &[ParsedTree]) -> Vec<StructuralFilterTuple> {
        // add one because range are end exclusive
        // frequency vector of pair (label weight, labelId)
        let mut sets_collection = Vec::with_capacity(trees.len());
        for tree in trees.iter() {
            // contains structural vectors for the current tree
            // is it a hash map of Label -> Vec<StructVec>
            let mut record_labels = StructHashMap::default();
            // nodes in a tree
            let tree_size = tree.count();

            let Some(root) = tree.iter().next() else {
                panic!("tree is empty");
            };
            let root_id = tree.get_node_id(root).unwrap();
            // for recursive postorder traversal
            let mut postorder_id = 0;
            // array of records stored in sets_collection
            self.create_record(
                &root_id,
                tree,
                &mut postorder_id,
                tree_size,
                &mut record_labels,
            );

            // reset state variables needed for positional evaluation
            self.actual_depth = 0;
            self.actual_pre_order_number = 0;
            let keys = record_labels.keys().cloned().collect::<FxHashSet<LabelId>>();
            sets_collection.push(StructuralFilterTuple(tree_size, record_labels, keys));
        }
        sets_collection
    }

    pub fn create_with_frequency(
        &mut self,
        trees: &[ParsedTree],
        label_dict: &LabelDict,
    ) -> Vec<(usize, Vec<LabelSetElement>)> {
        // add one because range are end exclusive
        let max_label_id = label_dict.values().max().unwrap() + 1;
        // frequency vector of pair (label weight, labelId)
        let mut label_freq_count = (0..max_label_id as usize).map(|lid| (0, lid)).collect_vec();
        let mut sets_collection = Vec::with_capacity(trees.len());

        // for each tree in collection create a structural vector records for each tree's labels
        for tree in trees.iter() {
            let mut record: Vec<LabelSetElement> = Vec::new();
            // contains structural vectors for the current tree
            // is it a hash map of Label -> Vec<StructVec>
            let mut record_labels = StructHashMap::default();
            // nodes in a tree
            let tree_size = tree.count();

            let Some(root) = tree.iter().next() else {
                panic!("tree is empty");
            };
            let root_id = tree.get_node_id(root).unwrap();
            // for recursive postorder traversal
            let mut postorder_id = 0;
            // array of records stored in sets_collection
            self.create_record(
                &root_id,
                tree,
                &mut postorder_id,
                tree_size,
                &mut record_labels,
            );

            // reset state variables needed for positional evaluation
            self.actual_depth = 0;
            self.actual_pre_order_number = 0;

            for (_, r) in record_labels {
                record.push(r);
            }

            sets_collection.push((tree_size, record));
        }

        for (_, record) in sets_collection.iter() {
            for label_set_element in record.iter() {
                label_freq_count[label_set_element.id as usize].0 += label_set_element.weight;
            }
        }
        // sort the vector based on label frequency
        label_freq_count.sort_by(|a, b| a.0.cmp(&b.0));

        // label map list [labelId] = frequencyId
        let mut label_map_list = Vec::with_capacity(label_freq_count.len());
        for (i, (_, lbl_cnt_)) in label_freq_count.iter().enumerate() {
            label_map_list[*lbl_cnt_] = i as LabelId;
        }

        for (_, record) in sets_collection.iter_mut() {
            for i in 0..record.len() {
                record[i].id = label_map_list[record[i].id as usize];
            }

            record.sort_by(|a, b| a.id.cmp(&b.id));

            let mut weight_sum = 0;
            for se in record.iter_mut() {
                weight_sum += se.weight;
                se.weigh_so_far = weight_sum;
            }
        }

        sets_collection
    }

    fn create_record(
        &mut self,
        root_id: &NodeId,
        tree: &ParsedTree,
        mut postorder_id: &mut usize,
        tree_size: usize,
        record_labels: &mut StructHashMap,
    ) -> usize {
        // number of children = subtree_size - 1
        // subtree_size = 1 -> actual node + sum of children
        let mut subtree_size = 1;

        self.actual_depth += 1;

        for cid in root_id.children(tree) {
            subtree_size +=
                self.create_record(&cid, tree, &mut postorder_id, tree_size, record_labels);
        }

        *postorder_id += 1;
        self.actual_depth -= 1;
        self.actual_pre_order_number += 1;

        let root_label = tree.get(*root_id).unwrap().get();
        let node_struct_vec = StructuralVec {
            postorder_id: *postorder_id,
            nodes_left: self.actual_pre_order_number - subtree_size,
            nodes_right: tree_size - (self.actual_pre_order_number + self.actual_depth),
            nodes_ancestors: self.actual_depth,
            nodes_descendants: subtree_size - 1,
        };

        if let Some(se) = record_labels.get_mut(root_label) {
            se.weight += 1;
            se.struct_vec.push(node_struct_vec);
        } else {
            let mut se = LabelSetElement {
                id: *tree.get(*root_id).unwrap().get(),
                weight: 1,
                ..LabelSetElement::default()
            };
            se.struct_vec.push(node_struct_vec);
            record_labels.insert(*root_label, se);
        }
        subtree_size
    }
}

/// Given two sets
pub fn ted(s1: &StructuralFilterTuple, s2: &StructuralFilterTuple, k: usize) -> usize {
    use std::cmp::max;
    let bigger = max(s1.0, s2.0);
    let mut overlap = 0;

    if s1.0.abs_diff(s2.0) > k {
        return k + 1;
    }


    for (lblid, set1) in s1.1.iter() {
        if let Some(set2) = s2.1.get(lblid) {
            if set1.weight == 1 && set2.weight == 1 {
                let (n1, n2) = (&set1.struct_vec[0], &set2.struct_vec[0]);
                let l1_region_distance = n1.nodes_left.abs_diff(n2.nodes_left)
                    + n1.nodes_right.abs_diff(n2.nodes_right)
                    + n1.nodes_ancestors.abs_diff(n2.nodes_ancestors)
                    + n1.nodes_descendants.abs_diff(n2.nodes_descendants);
                if l1_region_distance <= k {
                    overlap += 1;
                    continue;
                }
            }

            let mut s1c = set1;
            let mut s2c = set2;

            if set2.weight < set1.weight {
                (s1c, s2c) = (s2c, s1c);
            }

            for n1 in s1c.struct_vec.iter() {
                let k_window = n1.postorder_id.saturating_sub(k);

                // apply postorder filter
                for n2 in s2c.struct_vec.iter().skip_while(|n2| {
                    k_window < s2c.struct_vec.len() && n2.postorder_id < k_window
                }) {
                    if n2.postorder_id > k + n1.postorder_id {
                        break;
                    }

                    let l1_region_distance = n1.nodes_left.abs_diff(n2.nodes_left)
                        + n1.nodes_right.abs_diff(n2.nodes_right)
                        + n1.nodes_ancestors.abs_diff(n2.nodes_ancestors)
                        + n1.nodes_descendants.abs_diff(n2.nodes_descendants);

                    if l1_region_distance <= k {
                        overlap += 1;
                        break;
                    }
                }
            }

        }
    }
    
    bigger - overlap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::parse_tree;

    #[test]
    fn test_set_converting() {
        let t1input = "{1{2{2 Beware}{2{2 the}{2{2 quirky}{2 Brit-com}}}}{2 .}}".to_owned();
        let t2input = "{3{3{2{2 A}{2 film}}{3{2 of}{3{2 quiet}{2 power}}}}{2 .}}".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];

        let mut sc = LabelSetConverter::default();
        let sets = sc.create_with_frequency(&v, &label_dict);
        println!("{}", sets.len());
    }
}
