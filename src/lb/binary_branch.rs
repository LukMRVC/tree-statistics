//! This module implements binary branch label converter and lower bound distance

use crate::parsing::{LabelId, ParsedTree};
use indextree::NodeId;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::cmp::min;

pub type BinaryBranchVector = FxHashMap<i32, i32>;
pub struct BinaryBranchTuple(usize, BinaryBranchVector);

// Binary branch tuple (root label, left label, right label)
type BBTuple = (LabelId, Option<LabelId>, Option<LabelId>);

#[derive(Debug, Default, Clone)]
pub struct BinaryBranchConverter {
    binary_branch_id_map: FxHashMap<BBTuple, i32>,
    bb_id: i32,
}

impl BinaryBranchConverter {
    // const EMPTY_NODE: LabelId = -1;

    pub fn create(&mut self, trees: &[ParsedTree]) -> Vec<BinaryBranchTuple> {
        trees
            .iter()
            .map(|tree| {
                let Some(root) = tree.iter().next() else {
                    panic!("tree is empty");
                };
                let root_id = tree.get_node_id(root).unwrap();
                let mut branch_vector = BinaryBranchVector::default();
                self.create_vector(&root_id, tree, None, &mut branch_vector);
                BinaryBranchTuple(tree.count(), branch_vector)
            })
            .collect_vec()
    }

    fn create_vector(
        &mut self,
        root_id: &NodeId,
        tree: &ParsedTree,
        right_sibling_label: Option<LabelId>,
        branch_vector: &mut BinaryBranchVector,
    ) {
        let children = root_id.children(tree).collect_vec();
        let mut left_label = None;
        if let Some(left_child) = children.first() {
            left_label = Some(*tree.get(*left_child).unwrap().get())
        }

        let bb_tuple: BBTuple = (
            *tree.get(*root_id).unwrap().get(),
            left_label,
            right_sibling_label,
        );

        let bb_id = self
            .binary_branch_id_map
            .entry(bb_tuple)
            .or_insert_with(|| {
                self.bb_id += 1;
                self.bb_id
            });

        branch_vector
            .entry(*bb_id)
            .and_modify(|count| *count += 1)
            .or_insert(1);

        for (i, cnode) in children.iter().enumerate() {
            let right_sibling_l = if i < children.len() - 1 {
                Some(*tree.get(children[i + 1]).unwrap().get())
            } else {
                None
            };
            self.create_vector(cnode, tree, right_sibling_l, branch_vector);
        }
    }
}

pub fn ted(t1: &BinaryBranchTuple, t2: &BinaryBranchTuple, k: usize) -> usize {
    let (t1s, t2s) = (t1.0, t2.0);
    if t1s.abs_diff(t2s) > k {
        return k + 1;
    }
    let mut intersection_size = 0usize;

    for (label, postings) in t1.1.iter() {
        let Some(t2postings) = t2.1.get(label) else {
            continue;
        };
        intersection_size += min(*t2postings, *postings) as usize;
    }

    // l1_diff / 5
    ((t1s + t2s) - (2 * intersection_size)) / 5
    // ((t1s + t2s) - (l1_diff)) / 5
}
