use std::num::NonZeroUsize;

use crate::{
    lb::sed::TraversalCharacter,
    parsing::{LabelDict, LabelFreqOrdering, LabelId, ParsedTree},
};
use indextree::NodeId;

use itertools::Itertools;
use rustc_hash::FxHashMap;

pub trait Indexer {
    fn index_tree(tree: &ParsedTree, label_dict: &LabelDict) -> Self
    where
        Self: Sized;
}

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug)]
pub struct SEDIndexWithStructure {
    pub preorder: Vec<TraversalCharacter>,
    pub postorder: Vec<TraversalCharacter>,

    pub reversed_preorder: Vec<TraversalCharacter>,
    pub reversed_postorder: Vec<TraversalCharacter>,
    pub c: ConstantsIndex,
}

impl Indexer for SEDIndexWithStructure {
    fn index_tree(tree: &ParsedTree, _label_dict: &LabelDict) -> Self {
        let Some(root) = tree.iter().next() else {
            panic!("Unable to get root but tree is not empty!");
        };
        let root_id = tree.get_node_id(root).unwrap();

        let mut pre = Vec::with_capacity(tree.count());
        let mut post = Vec::with_capacity(tree.count());
        let mut reversed_preorder = Vec::with_capacity(tree.count());
        let mut reversed_postorder = Vec::with_capacity(tree.count());

        let mut postorder_id = 0usize;
        let mut preorder_id = 0usize;
        let mut depth = 0usize;
        Self::traverse_with_info(
            root_id,
            tree,
            &mut pre,
            &mut post,
            &mut reversed_preorder,
            &mut reversed_postorder,
            &mut postorder_id,
            &mut preorder_id,
            &mut depth,
        );

        reversed_preorder.reverse();
        reversed_postorder.reverse();
        Self {
            postorder: post,
            preorder: pre,
            reversed_postorder,
            reversed_preorder,
            c: ConstantsIndex {
                tree_size: tree.count(),
            },
        }
    }
}

impl SEDIndexWithStructure {
    fn traverse_with_info(
        nid: NodeId,
        tree: &ParsedTree,
        pre: &mut Vec<TraversalCharacter>,
        post: &mut Vec<TraversalCharacter>,
        rev_pre: &mut Vec<TraversalCharacter>,
        rev_post: &mut Vec<TraversalCharacter>,
        postorder_id: &mut usize,
        preorder_id: &mut usize,
        depth: &mut usize,
    ) -> usize {
        let mut subtree_size = 1;
        *depth += 1;
        // i am here at the current root
        let label = tree.get(nid).unwrap().get();
        pre.push(TraversalCharacter {
            char: *label,
            preorder_following_postorder_preceding: 0,
            preorder_descendant_postorder_ancestor: 0,
        });
        
        // to get reversed postorder traversal we need to reverse the preorder traversal
        rev_post.push(TraversalCharacter {
            char: *label,
            preorder_following_postorder_preceding: 0,
            preorder_descendant_postorder_ancestor: 0,
        });

        let pre_idx = pre.len() - 1;
        // let node_char = pre.last_mut().unwrap();
        for cnid in nid.children(tree) {
            subtree_size += Self::traverse_with_info(
                cnid,
                tree,
                pre,
                post,
                rev_pre,
                rev_post,
                postorder_id,
                preorder_id,
                depth,
            );
        }

        *depth -= 1;
        *postorder_id += 1;
        *preorder_id += 1;

        // preceding
        let preceding = *postorder_id - subtree_size;
        let following = tree.count() - (*postorder_id + *depth);

        post.push(TraversalCharacter {
            char: *label,
            preorder_following_postorder_preceding: following as i32,
            preorder_descendant_postorder_ancestor: *depth as i32,
        });

        // to get a reversed preorder traversal we need to reverse the postorder traversal
        rev_pre.push(TraversalCharacter {
            char: *label,
            preorder_following_postorder_preceding: preceding as i32,
            preorder_descendant_postorder_ancestor: subtree_size as i32 - 1,
        });

        pre[pre_idx].preorder_following_postorder_preceding = following as i32;
        pre[pre_idx].preorder_descendant_postorder_ancestor = subtree_size as i32 - 1;

        rev_post[pre_idx].preorder_following_postorder_preceding = preceding as i32;
        rev_post[pre_idx].preorder_descendant_postorder_ancestor = *depth as i32;
        // node_char.info = following as i32;

        subtree_size
    }
}

pub type InvListLblPost = FxHashMap<LabelId, Vec<i32>>;

/// Inverted list of nodes, key is index which is the label id in label dict
/// and postings list contains postorder traversal number
#[derive(Debug, PartialEq, Eq)]
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

impl InvertedListLabelPostorderIndex {
    pub fn get_sorted_nodes(&self, ordering: &LabelFreqOrdering) -> Vec<(&LabelId, usize)> {
        self.inverted_list
            .iter()
            .sorted_by_key(|(&label, _)| {
                if label as usize >= ordering.len() {
                    return usize::MIN;
                }
                *ordering
                    .get(NonZeroUsize::new(label as usize).unwrap())
                    .unwrap()
            })
            .map(|(l, lc)| (l, lc.len()))
            .collect_vec()
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
    use crate::parsing::{parse_single, LabelDict};

    #[test]
    fn test_pre_and_preorder() {
        use crate::parsing::parse_tree;
        let tree_str = "{1{2{3}{4}}{5{6}}{7{8}{9}}}".to_owned();
        let mut label_dict = LabelDict::new();
        let parsed_tree = parse_single(tree_str, &mut label_dict);

        let sed_index = SEDIndex::index_tree(&parsed_tree, &label_dict);
        assert_eq!(sed_index.preorder, vec![1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(sed_index.postorder, vec![3, 4, 2, 6, 5, 8, 9, 7, 1]);
    }

    #[test]
    fn test_inverted_list_postorder_index() {
        let tree_str = "{a{a{f}{b}{x}}{b}{y}}".to_owned();
        /*
        Parsed labels will be:
        a -> 1
        f -> 2
        b -> 3
        x -> 4
        y -> 5
         */
        let mut label_dict = LabelDict::new();
        let tree = parse_single(tree_str, &mut label_dict);
        let idx = InvertedListLabelPostorderIndex::index_tree(&tree, &label_dict);

        let kvs = [
            (1, vec![3, 6]),
            (2, vec![0]),
            (3, vec![1, 4]),
            (4, vec![2]),
            (5, vec![5]),
        ];

        let mut qh = InvListLblPost::default();

        for (k, v) in kvs {
            qh.insert(k, v);
        }

        assert_eq!(idx.inverted_list, qh);
    }

    #[test]
    fn test_sed_index_traversals() {
        let tree_str = "{a{b}{c}{a{c}{b}}}".to_owned();
        /*
        Parsed labels will be:
        a -> 1
        b -> 2
        c -> 3
         */
        let mut label_dict = LabelDict::new();
        let tree = parse_single(tree_str, &mut label_dict);
        let idx = SEDIndexWithStructure::index_tree(&tree, &label_dict);
        assert_eq!(
            idx.preorder,
            vec![
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 5
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 4,
                    preorder_descendant_postorder_ancestor: 0
                },
                TraversalCharacter {
                    char: 3,
                    preorder_following_postorder_preceding: 3,
                    preorder_descendant_postorder_ancestor: 0
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 2
                },
                TraversalCharacter {
                    char: 3,
                    preorder_following_postorder_preceding: 1,
                    preorder_descendant_postorder_ancestor: 0
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0
                }
            ]
        );
        assert_eq!(
            idx.postorder,
            vec![
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 3,
                    preorder_descendant_postorder_ancestor: 2
                },
                TraversalCharacter {
                    char: 3,
                    preorder_following_postorder_preceding: 2,
                    preorder_descendant_postorder_ancestor: 2
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 2,
                    preorder_descendant_postorder_ancestor: 1
                },
                TraversalCharacter {
                    char: 3,
                    preorder_following_postorder_preceding: 1,
                    preorder_descendant_postorder_ancestor: 1
                },
                TraversalCharacter {
                    char: 2,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 1
                },
                TraversalCharacter {
                    char: 1,
                    preorder_following_postorder_preceding: 0,
                    preorder_descendant_postorder_ancestor: 0
                },
            ]
        );
    }
}
