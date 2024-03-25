use crate::parsing::{LabelDict, LabelId, ParsedTree};
use indextree::NodeId;
use itertools::Itertools;
use std::collections::HashMap;

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

/// Takes a collection of trees and converts them into a collection of label
/// sets. A label set consists of labels and each label holds all nodes with that
/// label. The labels are substituted with their inverted label frequency number.
/// The labels in the sets are sorted by the global inverted frequency ordering
/// of the input collection.
pub struct LabelSetConverter {
    actual_depth: usize,
    actual_pre_order_number: usize,
    next_token_id: usize,
}

impl LabelSetConverter {
    pub fn create_with_frequency(
        &mut self,
        trees: &[ParsedTree],
        label_dict: &LabelDict,
    ) -> Vec<(usize, Vec<LabelSetElement>)> {
        let max_label_id = label_dict.values().max().unwrap();
        // vector where the index corresponds to labelId and the value there is the frequency of the labelId
        let mut token_count = vec![0; *max_label_id as usize + 1];

        for tree in trees.iter() {
            let mut record: Vec<LabelSetElement> = Vec::new();
            let mut record_labels = HashMap::new();
            let tree_size = tree.count();

            let Some(root) = tree.iter().next() else {
                panic!("tree is empty");
            };
            let root_id = tree.get_node_id(root).unwrap();

            let postorder_id = 0;
            self.create_record(&root_id, tree, postorder_id, tree_size, &mut record_labels);
        }

        vec![]
    }

    fn create_record(
        &mut self,
        root_id: &NodeId,
        tree: &ParsedTree,
        mut postorder_id: usize,
        tree_size: usize,
        record_labels: &mut HashMap<LabelId, LabelSetElement>,
    ) -> usize {
        // number of children = subtree_size - 1
        // subtree_size = 1 -> actual node + sum of children
        let mut subtree_size = 1;

        self.actual_depth += 1;

        for cid in root_id.children(tree) {
            subtree_size += self.create_record(&cid, tree, postorder_id, tree_size, record_labels);
        }

        postorder_id += 1;
        self.actual_depth -= 1;
        self.actual_pre_order_number += 1;

        let root_label = tree.get(*root_id).unwrap().get();

        if let Some(se) = record_labels.get_mut(root_label) {
            se.weight += 1;
            se.struct_vec.push(StructuralVec {
                postorder_id,
                nodes_left: self.actual_pre_order_number - subtree_size,
                nodes_right: tree_size - (self.actual_pre_order_number + self.actual_depth),
                nodes_ancestors: self.actual_depth,
                nodes_descendants: subtree_size - 1,
            });
        } else {
            let mut se = LabelSetElement {
                id: *tree.get(*root_id).unwrap().get(),
                weight: 1,
                ..LabelSetElement::default()
            };
            se.struct_vec.push(StructuralVec {
                postorder_id,
                nodes_left: self.actual_pre_order_number - subtree_size,
                nodes_right: tree_size - (self.actual_pre_order_number + self.actual_depth),
                nodes_ancestors: self.actual_depth,
                nodes_descendants: subtree_size - 1,
            });
            record_labels.insert(*root_label, se);
        }
        subtree_size
    }
}

pub fn ted() {}
