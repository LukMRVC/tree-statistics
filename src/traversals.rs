use indextree::{Arena, NodeId};

pub fn get_pre_post_strings(tree: &Arena<String>) -> (Vec<&str>, Vec<&str>) {
    let Some(root) = tree.iter().next() else {
        panic!("Unable to get root but tree is not empty!");
    };
    let root_id = tree.get_node_id(root).unwrap();

    let mut pre = Vec::with_capacity(tree.count());
    let mut post = Vec::with_capacity(tree.count());

    traverse(root_id, tree, &mut pre, &mut post);
    (pre, post)
}

fn traverse<'a>(nid: NodeId, tree: &'a Arena<String>, pre: &mut Vec<&'a str>, post: &mut Vec<&'a str>) {
    // i am here at the current root
    let label = tree.get(nid).unwrap().get().as_str();
    pre.push(label);
    for cnid in nid.children(tree) {
        traverse(cnid, tree, pre, post);
    }
    post.push(label);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_and_preorder() {
        let mut a = Arena::new();
        let n1 = a.new_node("1".to_owned());
        let n2 = a.new_node("2".to_owned());
        let n3 = a.new_node("3".to_owned());
        let n4 = a.new_node("4".to_owned());
        let n5 = a.new_node("5".to_owned());
        let n6 = a.new_node("6".to_owned());
        let n7 = a.new_node("7".to_owned());
        let n8 = a.new_node("8".to_owned());
        let n9 = a.new_node("9".to_owned());

        n1.append(n2, &mut a);
        n1.append(n3, &mut a);
        n1.append(n4, &mut a);

        n2.append(n5, &mut a);
        n2.append(n6, &mut a);

        n3.append(n7, &mut a);

        n4.append(n8, &mut a);
        n4.append(n9, &mut a);

        let (pre, post) = get_pre_post_strings(&a);
        assert_eq!(pre, vec!["1", "2", "5", "6", "3", "7", "4", "8", "9"]);
        assert_eq!(post, vec!["5", "6", "2", "7", "3", "8", "9", "4", "1"]);
    }
}