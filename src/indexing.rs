use crate::parsing::{LabelDict, LabelId, ParsedTree};
use indextree::NodeId;

use rustc_hash::FxHashMap;

pub trait Indexer {
    fn index_tree(tree: &ParsedTree, label_dict: &LabelDict) -> Self
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct ConstantsIndex {
    pub tree_size: usize,
}

#[derive(Debug)]
pub struct SEDIndex {
    pub preorder: Vec<i32>,
    pub postorder: Vec<i32>,
    pub c: ConstantsIndex,
}

impl Indexer for SEDIndex {
    fn index_tree(tree: &ParsedTree, _label_dict: &LabelDict) -> Self {
        let Some(root) = tree.iter().next() else {
            panic!("Unable to get root but tree is not empty!");
        };
        let root_id = tree.get_node_id(root).unwrap();

        let mut pre = Vec::with_capacity(tree.count());
        let mut post = Vec::with_capacity(tree.count());

        traverse(root_id, tree, &mut pre, &mut post);

        Self {
            postorder: post,
            preorder: pre,
            c: ConstantsIndex {
                tree_size: tree.count(),
            },
        }
    }
}

fn traverse(nid: NodeId, tree: &ParsedTree, pre: &mut Vec<i32>, post: &mut Vec<i32>) {
    // i am here at the current root
    let label = tree.get(nid).unwrap().get();
    pre.push(*label);
    for cnid in nid.children(tree) {
        traverse(cnid, tree, pre, post);
    }
    post.push(*label);
}

pub type InvListLblPost = FxHashMap<LabelId, Vec<i32>>;

/// Inverted list of nodes, key is index which is the label id in label dict
/// and postings list contains postorder traversal number
#[derive(Debug)]
pub struct InvertedListLabelPostorderIndex {
    pub inverted_list: InvListLblPost,
    pub c: ConstantsIndex,
}

impl Indexer for InvertedListLabelPostorderIndex {
    fn index_tree(tree: &ParsedTree, _label_dict: &LabelDict) -> Self {
        let Some(root) = tree.iter().next() else {
            panic!("Unable to get root but tree is not empty!");
        };
        let mut inverted_list = InvListLblPost::default();
        let root_id = tree.get_node_id(root).unwrap();
        traverse_inverted(root_id, tree, &mut inverted_list, 0);

        Self {
            inverted_list,
            c: ConstantsIndex {
                tree_size: tree.count(),
            },
        }
    }
}

fn traverse_inverted(
    nid: NodeId,
    tree: &ParsedTree,
    inverted_list: &mut InvListLblPost,
    start_postorder: i32,
) -> i32 {
    let label = tree.get(nid).unwrap().get();
    let mut postorder_id = start_postorder;
    let mut children = 0;
    for cnid in nid.children(tree) {
        postorder_id += traverse_inverted(cnid, tree, inverted_list, postorder_id);
        children += 1;
    }
    inverted_list
        .entry(*label)
        .and_modify(|postings| postings.push(postorder_id))
        .or_insert(vec![postorder_id]);
    children + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsing::parse_tree;

    #[test]
    fn test_pre_and_preorder() {
        use crate::parsing::parse_tree;
        let tree_str = "{1{2{5}{6}}{3{7}}{4{8}{9}}}".to_owned();
        // parsed labels will be
        // 1 -> 0
        // 2 -> 1
        // 5 -> 2
        // 6 -> 3
        // 3 -> 4
        // 7 -> 5
        // 4 -> 6
        // 8 -> 7
        // 9 -> 8
        let mut label_dict = LabelDict::new();
        let parse_result = parse_tree(Ok(tree_str), &mut label_dict);
        assert!(parse_result.is_ok(), "Tree parsing failed, which shouldn't");
        let parsed_tree = parse_result.unwrap();

        let sed_index = SEDIndex::index_tree(&parsed_tree, &label_dict);
        assert_eq!(sed_index.preorder, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(sed_index.postorder, vec![2, 3, 1, 5, 4, 7, 8, 6, 0]);
    }

    #[test]
    fn test_inverted_list_postorder_index() {
        let tree_str = "{a{a{f}{b}{x}}{b}{y}}".to_owned();
        /*
        Parsed labels will be:
        a -> 0
        f -> 1
        b -> 2
        x -> 3
        y -> 4
         */
        let mut label_dict = LabelDict::new();
        let parse_result = parse_tree(Ok(tree_str), &mut label_dict);
        assert!(parse_result.is_ok(), "Tree parsing failed, which shouldn't");
        let tree = parse_result.unwrap();
        let idx = InvertedListLabelPostorderIndex::index_tree(&tree, &label_dict);

        let kvs = [
            (0, vec![3, 6]),
            (1, vec![0]),
            (2, vec![1, 4]),
            (3, vec![2]),
            (4, vec![5]),
        ];

        let mut qh = InvListLblPost::default();

        for (k, v) in kvs {
            qh.insert(k, v);
        }

        assert_eq!(idx.inverted_list, qh);
    }
}
