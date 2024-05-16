use crate::parsing::{LabelDict, LabelId, ParsedTree};
use indextree::NodeId;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::cmp::max;

type StructHashMap = FxHashMap<LabelId, LabelSetElement>;

const REGION_LEFT_IDX: usize = 0;
/// ancestors
const REGION_ANC_IDX: usize = 1;

const REGION_RIGHT_IDX: usize = 2;
/// descendants
const REGION_DESC_IDX: usize = 3;

/// The building block for structural filter, holds information about
/// the count of ancestral nodes, descendants nodes, to the left and to the right
// difference between children and descendants? Children nodes are only 1 level below current node level
// while descendants are all nodes below the current node
#[derive(Debug, Default, Clone, PartialEq)]
pub struct StructuralVec {
    /// Id of postorder tree traversal
    pub postorder_id: usize,
    pub preorder_id: usize,
    /// Vector of number of nodes to the left, ancestors, nodes to right and descendants
    pub mapping_region: [i32; 4],

    // regions according to postorder and preorder ID are relative to current node
    // left region -> smaller pre and post IDs, ancestor region -> bigger post, smaller pre
    // right region -> bigger pre and post IDS, descendants region -> smaller post, bigger pre
    pub unmapped_mapping_region: [i32; 4],
    pub mapped: Option<Vec<usize>>,
}

/// This is an element holding relevant data of a set.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct LabelSetElement {
    pub id: LabelId,
    pub weight: usize,
    pub weigh_so_far: usize,

    pub struct_vec: Vec<RefCell<StructuralVec>>,
}

/// Base struct tuple for structural filter
#[derive(Clone, Debug)]
pub struct StructuralFilterTuple(usize, StructHashMap);

/// Takes a collection of trees and converts them into a collection of label
/// sets. A label set consists of labels and each label holds all nodes with that
/// label. The labels are substituted with their inverted label frequency number.
/// The labels in the sets are sorted by the global inverted frequency ordering
/// of the input collection.
#[derive(Debug, Default)]
pub struct LabelSetConverter {
    actual_depth: usize,
    actual_pre_order_number: usize,
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
                1,
                &mut postorder_id,
                tree_size,
                &mut record_labels,
            );

            // reset state variables needed for positional evaluation
            self.actual_depth = 0;
            self.actual_pre_order_number = 0;
            sets_collection.push(StructuralFilterTuple(tree_size, record_labels));
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
                1,
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
        preorder_id: usize,
        mut postorder_id: &mut usize,
        tree_size: usize,
        record_labels: &mut StructHashMap,
    ) -> usize {
        // number of children = subtree_size - 1
        // subtree_size = 1 -> actual node + sum of children
        let mut subtree_size = 1;

        self.actual_depth += 1;

        for cid in root_id.children(tree) {
            subtree_size += self.create_record(
                &cid,
                tree,
                preorder_id + subtree_size,
                postorder_id,
                tree_size,
                record_labels,
            );
        }

        *postorder_id += 1;
        self.actual_depth -= 1;
        self.actual_pre_order_number += 1;

        let root_label = tree.get(*root_id).unwrap().get();
        let node_struct_vec = StructuralVec {
            preorder_id,
            postorder_id: *postorder_id,
            mapping_region: [
                (self.actual_pre_order_number - subtree_size) as i32,
                self.actual_depth as i32,
                (tree_size - (self.actual_pre_order_number + self.actual_depth)) as i32,
                (subtree_size - 1) as i32,
            ],
            ..Default::default()
        };

        if let Some(se) = record_labels.get_mut(root_label) {
            se.weight += 1;
            se.struct_vec.push(RefCell::new(node_struct_vec));
        } else {
            let mut se = LabelSetElement {
                id: *tree.get(*root_id).unwrap().get(),
                weight: 1,
                ..LabelSetElement::default()
            };
            se.struct_vec.push(RefCell::new(node_struct_vec));
            record_labels.insert(*root_label, se);
        }
        subtree_size
    }
}

#[inline(always)]
fn svec_l1(n1: &StructuralVec, n2: &StructuralVec) -> u32 {
    n1.mapping_region
        .iter()
        .zip_eq(n2.mapping_region.iter())
        .fold(0, |acc, (a, b)| acc + a.abs_diff(*b))
}

/// Given two sets
pub fn ted(s1: &StructuralFilterTuple, s2: &StructuralFilterTuple, k: usize) -> usize {
    use std::cmp::max;
    let bigger = max(s1.0, s2.0);

    if s1.0.abs_diff(s2.0) > k {
        return k + 1;
    }

    let overlap = get_nodes_overlap_with_region_distance(s1, s2, k, svec_l1, Some(()));

    bigger - overlap
}

#[inline(always)]
fn svec_l1_unmapped(n1: &StructuralVec, n2: &StructuralVec) -> u32 {
    n1.mapping_region
        .iter()
        .zip_eq(n1.unmapped_mapping_region.iter())
        .zip_eq(
            n2.mapping_region
                .iter()
                .zip_eq(n2.unmapped_mapping_region.iter()),
        )
        .fold(0, |acc, ((n1reg, n1umreg), (n2reg, n2umreg))| {
            acc + max(
                (n1reg - n1umreg).abs_diff(n2reg - n2umreg),
                max(n1umreg.abs() as u32, n2umreg.abs() as u32),
            )
        })
}

pub fn ted_variant(s1: &StructuralFilterTuple, s2: &StructuralFilterTuple, k: usize) -> usize {
    let bigger = max(s1.0, s2.0);

    if s1.0.abs_diff(s2.0) > k {
        return k + 1;
    }

    get_nodes_overlap_with_region_distance(s1, s2, k, svec_l1, None);

    let t1nodes = s1.1.values().flat_map(|se| &se.struct_vec).collect_vec();
    let t2nodes =
        s2.1.values()
            .flat_map(|se| &se.struct_vec)
            .sorted_by_cached_key(|se| se.borrow().postorder_id)
            .collect_vec();
    let mapped = t1nodes
        .iter()
        .filter(|n| n.borrow().mapped.is_some())
        .collect_vec();

    set_unmapped_regions(&t1nodes);
    set_unmapped_regions(&t2nodes);

    // let overlap = get_nodes_overlap_with_region_distance(&mut s1, &mut s2, k, svec_l1);
    let mut overlap = 0;

    for mapped_node_ref in mapped.iter() {
        let mapped_node = mapped_node_ref.borrow();
        let Some(n2post_id_vec) = &mapped_node.mapped else {
            panic!("Filter does not work!");
        };

        for n2post_id in n2post_id_vec.iter() {
            let Ok(n2node_idx) =
                t2nodes.binary_search_by_key(n2post_id, |n2node| n2node.borrow().postorder_id)
            else {
                panic!("Uncorrectly mapped nodes!");
            };

            if svec_l1_unmapped(&mapped_node, &t2nodes[n2node_idx].borrow()) as usize <= k {
                overlap += 1;
                // force 1:1 mapping by only allowing at most one node to have an overlap
                break;
            }
        }
    }

    reset_mappings(&t1nodes);
    reset_mappings(&t2nodes);

    bigger.saturating_sub(overlap)
}

fn reset_mappings(all_nodes: &[&RefCell<StructuralVec>]) {
    all_nodes.iter().for_each(|node| {
        let mut n = (*node).borrow_mut();
        n.mapped = None;
        n.unmapped_mapping_region = [0; 4];
    })
}

fn set_unmapped_regions(all_nodes: &[&RefCell<StructuralVec>]) {
    // let all_nodes = s.1.values().flat_map(|se| &se.struct_vec).collect_vec();
    let unmapped_nodes = all_nodes
        .iter()
        .filter(|un| un.borrow().mapped.is_none())
        .collect_vec();
    for n in all_nodes.iter().filter(|al| {
        let k = al.borrow();
        k.mapped.is_some()
    }) {
        // Vector of unmmaped nodes to the left, ancestors, nodes to right and descendants
        let mut unmapped_regions = [0; 4];
        let (post, pre) = {
            let n1 = n.borrow();
            (n1.postorder_id, n1.preorder_id)
        };

        for unmapped_node in unmapped_nodes.iter() {
            let n2 = unmapped_node.borrow();
            if n2.postorder_id == post {
                continue;
            }
            if n2.postorder_id < post && n2.preorder_id < pre {
                unmapped_regions[REGION_LEFT_IDX] += 1;
            } else if n2.postorder_id > post && n2.preorder_id > pre {
                unmapped_regions[REGION_RIGHT_IDX] += 1;
            } else if n2.postorder_id > post && n2.preorder_id < pre {
                unmapped_regions[REGION_ANC_IDX] += 1;
            } else {
                unmapped_regions[REGION_DESC_IDX] += 1;
            }
        }

        {
            let mut n1 = (*n).borrow_mut();
            n1.unmapped_mapping_region = unmapped_regions;
        }
    }
}

fn get_nodes_overlap_with_region_distance(
    s1: &StructuralFilterTuple,
    s2: &StructuralFilterTuple,
    k: usize,
    region_distance_closure: impl Fn(&StructuralVec, &StructuralVec) -> u32,
    break_on_first_mapping: Option<()>,
) -> usize {
    let mut overlap = 0;

    for (lblid, set1) in s1.1.iter() {
        if let Some(set2) = s2.1.get(lblid) {
            if set1.weight == 1 && set2.weight == 1 {
                let (mut n1, mut n2) = (
                    set1.struct_vec[0].borrow_mut(),
                    set2.struct_vec[0].borrow_mut(),
                );
                let l1_region_distance = region_distance_closure(&n1, &n2);
                if l1_region_distance as usize <= k {
                    n1.mapped = Some(vec![n2.postorder_id]);
                    n2.mapped = Some(vec![n1.postorder_id]);
                    overlap += 1;
                }
                continue;
            }

            let (s1c, s2c) = if set2.weight < set1.weight {
                (set2, set1)
            } else {
                (set1, set2)
            };

            for n1 in s1c.struct_vec.iter() {
                let k_window = n1.borrow().postorder_id.saturating_sub(k);
                let mut n1 = n1.borrow_mut();
                // apply postorder filter
                let s2clen = s2c.struct_vec.len();
                for n2 in s2c.struct_vec.iter() {
                    let mut n2 = n2.borrow_mut();
                    if k_window < s2clen && n2.postorder_id < k_window {
                        continue;
                    }

                    if n2.postorder_id > k + n1.postorder_id {
                        break;
                    }
                    let l1_region_distance = region_distance_closure(&n1, &n2);

                    if l1_region_distance as usize <= k {
                        if let Some(ref mut n1mapped) = &mut n1.mapped {
                            n1mapped.push(n2.postorder_id);
                        } else {
                            n1.mapped = Some(vec![n2.postorder_id]);
                        }
                        if let Some(ref mut n2mapped) = &mut n2.mapped {
                            n2mapped.push(n1.postorder_id);
                        } else {
                            n2.mapped = Some(vec![n1.postorder_id]);
                        }
                        overlap += 1;
                        // already_mapped.push(n2.postorder_id);
                        if break_on_first_mapping.is_some() {
                            break;
                        }
                    }
                }
            }
        }
    }

    overlap
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::parse_tree;

    #[test]
    fn test_set_converting() {
        let t1input = "{a{b}{a{b}{c}{a}}{b}}".to_owned();
        let t2input = "{a{c}{b{a{a}{b}{c}}}}".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);

        // 0 are labels A
        // 1 are labels B
        // 2 are labelcs C

        let lse_for_a = LabelSetElement {
            id: 0,
            weight: 3,
            weigh_so_far: 0,
            struct_vec: vec![
                RefCell::new(StructuralVec {
                    mapped: None,
                    mapping_region: [3, 2, 1, 0],
                    unmapped_mapping_region: [0, 0, 0, 0],
                    postorder_id: 4,
                    preorder_id: 6,
                }),
                RefCell::new(StructuralVec {
                    mapped: None,
                    mapping_region: [1, 1, 1, 3],
                    unmapped_mapping_region: [0, 0, 0, 0],
                    postorder_id: 5,
                    preorder_id: 3,
                }),
                RefCell::new(StructuralVec {
                    mapped: None,
                    mapping_region: [0, 0, 0, 6],
                    unmapped_mapping_region: [0, 0, 0, 0],
                    postorder_id: 7,
                    preorder_id: 1,
                }),
            ],
        };

        let lse_for_b = LabelSetElement {
            id: 1,
            weight: 3,
            weigh_so_far: 0,
            struct_vec: vec![
                RefCell::new(StructuralVec {
                    mapped: None,
                    mapping_region: [0, 1, 5, 0],
                    unmapped_mapping_region: [0, 0, 0, 0],
                    postorder_id: 1,
                    preorder_id: 2,
                }),
                RefCell::new(StructuralVec {
                    mapped: None,
                    mapping_region: [1, 2, 3, 0],
                    unmapped_mapping_region: [0, 0, 0, 0],
                    postorder_id: 2,
                    preorder_id: 4,
                }),
                RefCell::new(StructuralVec {
                    mapped: None,
                    mapping_region: [5, 1, 0, 0],
                    unmapped_mapping_region: [0, 0, 0, 0],
                    postorder_id: 6,
                    preorder_id: 7,
                }),
            ],
        };

        assert_eq!(sets[0].1.get(&0).unwrap(), &lse_for_a);
        assert_eq!(sets[0].1.get(&1).unwrap(), &lse_for_b);

        println!("{}", sets.len());
    }

    #[test]
    fn test_struct_ted() {
        let t1input = "{a{b}{a{b}{c}{a}}{b}}".to_owned();
        let t2input = "{a{c}{b{a{a}{b}{c}}}}".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);

        let lb = ted(&sets[0], &sets[1], 4);

        assert_eq!(lb, 2);
    }

    #[test]
    fn test_struct_ted_variant_simple() {
        let t1input = "{a{b}{a{a{b}{a}{b}}}{b}}".to_owned();
        let t2input = "{a{c}{b{a{a}{b}{b}}}".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);
        let lb = ted_variant(&sets[0], &sets[1], 4);
        assert!(lb <= 4);
        assert_eq!(lb, 1);
    }

    #[test]
    fn test_struct_ted_variant() {
        let t1input = "{20{20{20{1203}{1204}}{20{460}{20{465}{1205}}}}{24}}".to_owned();
        let t2input = "{0{0{0{118}{0{1456}{251}}}{20{460}{20{537}{1457}}}}{2}}".to_owned();
        let t3input = "{20{142}{20{20{375}{376}}{2}}}".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let t3 = parse_tree(Ok(t3input), &mut label_dict).unwrap();
        let v = vec![t1, t2, t3];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);
        let lb = ted_variant(&sets[0], &sets[1], 10);
        assert!(lb <= 10, "T1 and T2 failed");
        let lb = ted_variant(&sets[0], &sets[2], 10);
        assert!(lb <= 10, "T2 and T3 failed");
    }

    #[test]
    fn test_struct_ted_variant_2() {
        let t1input = "{9{20{20{673}{161}}{20{211}{100}}}{13}}".to_owned();
        let t2input = "{0{0{0{106}{9{888}{889}}}{20{460}{353}}}{2}} ".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);
        let lb = ted_variant(&sets[0], &sets[1], 10);
        assert!(lb <= 10);
    }

    #[test]
    fn test_struct_ted_variant_3() {
        let t1input = "{0{0{517}{20{472}{20{518}{519}}}}{24}}".to_owned();
        let t2input = "{0{0{15}{9{271}{9{9{890}{55}}{98}}}}{2}} ".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);
        let lb = ted_variant(&sets[0], &sets[1], 10);
        assert!(lb <= 10);
    }

    #[test]
    fn test_struct_ted_variant_4() {
        let t1input = "{0{74}{0{75}{2}}}".to_owned();
        let t2input = "{0{0{9{891}{892}}{20{591}{624}}}{20{591}{893}}} ".to_owned();
        let mut label_dict = LabelDict::new();
        let t1 = parse_tree(Ok(t1input), &mut label_dict).unwrap();
        let t2 = parse_tree(Ok(t2input), &mut label_dict).unwrap();
        let v = vec![t1, t2];
        let mut sc = LabelSetConverter::default();
        let sets = sc.create(&v);
        let lb = ted_variant(&sets[0], &sets[1], 10);
        assert!(lb <= 10);
    }
}
