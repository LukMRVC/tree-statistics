use indextree::{Arena, NodeId};
use crate::parsing::ParsedTree;

#[derive(Debug)]
pub struct SEDIndex {
    pub preorder: Vec<i32>,
    pub postorder: Vec<i32>,
}

impl SEDIndex {
    pub fn index_tree(tree: &ParsedTree) -> Self {
        let Some(root) = tree.iter().next() else {
            panic!("Unable to get root but tree is not empty!");
        };
        let root_id = tree.get_node_id(root).unwrap();

        let mut pre = Vec::with_capacity(tree.count());
        let mut post = Vec::with_capacity(tree.count());

        traverse(root_id, tree, &mut pre, &mut post);

        Self {
            postorder: post,
            preorder: pre
        }
    }
}

fn traverse<'a>(nid: NodeId, tree: &'a ParsedTree, pre: &mut Vec<i32>, post: &mut Vec<i32>) {
    // i am here at the current root
    let label = tree.get(nid).unwrap().get();
    pre.push(*label);
    for cnid in nid.children(tree) {
        traverse(cnid, tree, pre, post);
    }
    post.push(*label);
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_and_preorder() {
        let mut a = Arena::new();
        let n1 = a.new_node(1i32);
        let n2 = a.new_node(2i32);
        let n3 = a.new_node(3i32);
        let n4 = a.new_node(4i32);
        let n5 = a.new_node(5i32);
        let n6 = a.new_node(6i32);
        let n7 = a.new_node(7i32);
        let n8 = a.new_node(8i32);
        let n9 = a.new_node(9i32);

        n1.append(n2, &mut a);
        n1.append(n3, &mut a);
        n1.append(n4, &mut a);

        n2.append(n5, &mut a);
        n2.append(n6, &mut a);

        n3.append(n7, &mut a);

        n4.append(n8, &mut a);
        n4.append(n9, &mut a);

        let sed_index = SEDIndex::index_tree(&a as &ParsedTree);
        assert_eq!(sed_index.preorder, vec![1, 2, 5, 6, 3, 7, 4, 8, 9]);
        assert_eq!(sed_index.postorder, vec![5, 6, 2, 7, 3, 8, 9, 4, 1]);
    }
}