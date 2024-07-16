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

pub struct AptedIndex {
    pub c: ConstantsIndex,
    /// Stores label id of each node in a tree.
    /** Labels are inserted into a dictionary in their left-to-right preorder
       appearance.
       Indexed in left-to-right preorder.
     */
    pub prel_to_label_id_: Vec<i32>,
    /// Stores label id of each node in a tree.
    /**
     * Labels are inserted into a dictionary in their left-to-right preorder
     * appearance.
     * Indexed in left-to-right postorder.
     */
    pub postl_to_label_id_: Vec<i32>,

    /// Stores label id of each node in a tree.
    /**
     * Labels are inserted into a dictionary in their left-to-right preorder
     * appearance.
     * Indexed in right-to-left postorder.
     */
    pub postr_to_label_id_: Vec<i32>,

    /// Stores subtree size of each node in a tree.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_size_: Vec<i32>,

    /// Stores left-to-right preorder id of the parent node.
    /**
     * Indexed in left-to-right preorder.
     * `-1` represents no parent.
     */
    pub prel_to_parent_: Vec<i32>,

    /// Stores left-to-right preorder ids of each node's children.
    /**
     * Indexed in left-to-right preorder.
     */
    pub rel_to_children_: Vec<Vec<i32>>,

    /// Stores left-to-right postorder id of the leftmost leaf descendant of a node.
    /**
     * Indexed in left-to-right postorder.
     */
    pub postl_to_lld_: Vec<i32>,

    /// Stores right-to-left postorder id of the rightmost leaf descendant of a node.
    /**
     * Indexed in right-to-left postorder.
     * Depends on: PreLToSize, PostRToPreL, PreLToPostR, PreLToChildren.
     */
    pub postr_to_rld_: Vec<i32>,

    /// Stores left-to-right preorder id of the leftmost leaf descendant of a node.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_lld_: Vec<i32>,

    /// Stores left-to-right preorder id of the rightmost leaf descendant of a node.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_rld_: Vec<i32>,

    /// Stores preorder id of the first leaf node to the left/right.
    /**
     * prel_to_ln_: left-to-right preorder of the first leaf to the left.
     * prer_to_ln_: right-to-left preorder of the first leaf to the right.
     * `-1` represents no such node.
     * Depends on: PreLToSize, PreRToPreL.
     */
    pub prel_to_ln_: Vec<i32>,
    pub prer_to_ln_: Vec<i32>,

    /// Stores true if a node is leftmost child of its parent.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_type_left_: Vec<bool>,



    /// Stores true if a node is rightmost child of its parent.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_type_right_: Vec<bool>,

    /// Stores right-to-left preorder id of each node.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_prer_: Vec<i32>,


    /// Stores left-to-right preorder id of each node.
    /**
     * Indexed in right-to-left preorder.
     */
    pub prer_to_prel_: Vec<i32>,


    /// Stores left-to-right postorder id of each node.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_postl_: Vec<i32>,

    /// Stores left-to-right preorder id of each node.
    /**
     * Indexed in left-to-right postorder.
     */
    pub postl_to_prel_: Vec<i32>,


    /// Stores right-to-left postorder id of each node.
    /**
     * Indexed in left-to-right preorder.
     */
    pub prel_to_postr_: Vec<i32>,

    /// Stores left-to-right preorder id of each node.
    /**
     * Indexed in right-to-left postorder.
     */
    pub postr_to_prel_: Vec<i32>,


    /// Stores cost of a single-path function for each node [1, Section 5.2].
    /**
     * prel_to_cost_all_: spf_A - single-path function using inner path
     * prel_to_cost_left_: spf_L - single-path function using left path
     * prel_to_cost_right_: spf_R - single-path function using right path
     * Indexed in left-to-right preorder.
     */
    pub prel_to_cost_all_: Vec<i64>,
    pub prel_to_cost_left_: Vec<i64>,
    pub prel_to_cost_right_: Vec<i64>,


    /// Stores cost of deleting/inserting entire subtree for each node.
    /**
     * prel_to_subtree_del_cost_: cost of deleting entire subtree
     * prel_to_subtree_ins_cost_: cost of inserting entire subtree
     * Indexed in left-to-right preorder.
     */
    pub prel_to_subtree_del_cost_: Vec<f64>,
    pub prel_to_subtree_ins_cost_: Vec<f64>,
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

        assert_eq!(
            idx.inverted_list,
            qh
        );
    }
}
