use std::collections::HashSet;
use indextree::Arena;

#[derive(Default, Debug)]
pub struct TreeStatistics<'a> {
    /// Slice of degrees of tree - useful for histograms and average degree
    degrees: Vec<usize>,
    /// Tree depths - length of each path from root to leaf
    depths: Vec<usize>,
    /// distinct labels in a tree
    distinct_labels: HashSet<&'a str>,
}


pub fn gather(tree: &Arena<String>) -> TreeStatistics {
    if tree.is_empty() {
        return TreeStatistics::default();
    }

    let Some(root) = tree.iter().next() else {
        panic!("Unable to get root but tree is not empty!");
    };

    let mut node_stack = vec![];

    let root_id = tree.get_node_id(root).unwrap();
    let mut degrees = vec![];
    let mut depths = vec![];
    fn is_leaf(children: &usize) -> bool {
        *children == 0
    }
    let mut labels = HashSet::new();

    for nid in root_id.descendants(&tree) {
        let n = tree.get(nid).unwrap();
        labels.insert(n.get().as_str());

        let mut degree = nid.children(&tree).count();

        while node_stack.len() > 0 && *node_stack.last().unwrap() != tree.get(nid).unwrap().parent().unwrap() {
            node_stack.pop();
        }

        if is_leaf(&degree) {
            depths.push(node_stack.len());

        } else {
            node_stack.push(nid);
        }

        degree += if n.parent().is_some() { 1 } else { 0 };
        degrees.push(degree);
    }

    TreeStatistics {
        degrees,
        depths,
        distinct_labels: labels,
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_statistics() {
        let mut a = Arena::new();
        let n1 = a.new_node("first".to_owned());
        let n2 = a.new_node("second".to_owned());
        let n3 = a.new_node("third".to_owned());
        let n4 = a.new_node("fourth".to_owned());

        n1.append(n2, &mut a);
        n2.append(n3, &mut a);
        n3.append(n4, &mut a);
        let stats = gather(&a);

        assert_eq!(stats.depths, vec![3]);
        assert_eq!(stats.degrees, vec![1, 2, 2, 1]);
        assert_eq!(stats.distinct_labels.len(), 4);
    }

    #[test]
    fn test_branched_stats() {
        let mut a = Arena::new();
        let n1 = a.new_node("a".to_owned());
        let n2 = a.new_node("b".to_owned());
        let n3 = a.new_node("c".to_owned());
        let n4 = a.new_node("d".to_owned());
        let n5 = a.new_node("c".to_owned());
        let n6 = a.new_node("b".to_owned());
        let n7 = a.new_node("f".to_owned());


        n1.append(n2, &mut a);
        n2.append(n3, &mut a);
        n3.append(n4, &mut a);
        n3.append(n5, &mut a);

        n1.append(n6, &mut a);
        n6.append(n7, &mut a);


        let stats = gather(&a);


        assert_eq!(stats.depths, vec![3, 3, 2]);
        assert_eq!(stats.degrees, vec![2, 2, 3, 1, 1, 2, 1]);
        assert_eq!(stats.distinct_labels.len(), 5);
    }
}