use indextree::Arena;
use memchr::memchr2_iter;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetParseError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    ParseError(#[from] TreeParseError),
}

pub type LabelId = i32;

pub type LabelDict = HashMap<String, LabelId>;
pub(crate) type ParsedTree = Arena<LabelId>;

pub fn parse_dataset(
    dataset_file: PathBuf,
    label_dict: &mut LabelDict,
) -> Result<Vec<ParsedTree>, DatasetParseError> {
    let f = File::open(dataset_file)?;
    let reader = BufReader::new(f);
    let trees: Vec<ParsedTree> = reader
        .lines()
        .map(|l| parse_tree(l, label_dict))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(trees)
}

const TOKEN_START: u8 = b'{';
const TOKEN_END: u8 = b'}';
const ESCAPE_CHAR: u8 = b'\\';

#[inline(always)]
fn is_escaped(byte_string: &[u8], offset: usize) -> bool {
    offset > 0 && byte_string[offset - 1] == ESCAPE_CHAR
}

#[derive(Error, Debug)]
pub enum TreeParseError {
    #[error("tree string contains non ascii characters")]
    IsNotAscii,
    #[error(transparent)]
    LineReadError(#[from] io::Error),
    #[error("tree string has incorrect bracket notation format: {}", .0)]
    IncorrectFormat(String),
    #[error("Bad tokenizing")]
    TokenizerError,
}

pub(crate) fn parse_tree(
    tree_str: Result<String, io::Error>,
    label_map: &mut LabelDict,
) -> Result<ParsedTree, TreeParseError> {
    use TreeParseError as TPE;

    let tree_str = tree_str?;
    if !tree_str.is_ascii() {
        return Err(TPE::IsNotAscii);
    }
    let mut tree = ParsedTree::new();
    let tree_bytes = tree_str.as_bytes();

    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(TPE::IncorrectFormat(
            "Minimal of 2 brackets not found!".to_owned(),
        ));
    }
    let mut max_node_id = label_map.values().max().cloned().unwrap_or(0);

    let mut tokens = token_positions.iter().peekable();
    let root_start = *tokens.next().unwrap();
    let root_end = **tokens.peek().unwrap();

    let root_label = String::from_utf8(tree_bytes[(root_start + 1)..root_end].to_vec()).unwrap();
    let is_first_label_in_map = label_map.is_empty();
    let root_label = label_map.entry(root_label).or_insert_with(|| {
        if !is_first_label_in_map {
            max_node_id += 1;
        }
        max_node_id
    });
    let root = tree.new_node(*root_label);

    let mut node_stack = vec![root];

    while let Some(token) = tokens.next() {
        match tree_bytes[*token] {
            TOKEN_START => {
                let Some(token_end) = tokens.peek() else {
                    let err_msg =
                        format!("Label has no ending token near col {token} , line \"{tree_str}\"");
                    return Err(TPE::IncorrectFormat(err_msg));
                };
                let label =
                    String::from_utf8(tree_bytes[(*token + 1)..**token_end].to_vec()).unwrap();

                let node_label = label_map.entry(label).or_insert_with(|| {
                    max_node_id += 1;
                    max_node_id
                });

                let n = tree.new_node(*node_label);
                let Some(last_node) = node_stack.last() else {
                    let err_msg = format!("Reached unexpected end of token on line \"{tree_str}\"");
                    return Err(TPE::IncorrectFormat(err_msg));
                };
                last_node.append(n, &mut tree);
                node_stack.push(n);
            }
            TOKEN_END => {
                let Some(_) = node_stack.pop() else {
                    return Err(TPE::IncorrectFormat("Wrong bracket pairing".to_owned()));
                };
            }
            _ => return Err(TPE::TokenizerError),
        }
    }

    Ok(tree)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses() {
        let input = "{einsteinstrasse{1}{3}}".to_owned();
        let mut hs = LabelDict::new();
        let arena = parse_tree(Ok(input), &mut hs);
        assert!(arena.is_ok());
        let arena = arena.unwrap();
        assert_eq!(arena.count(), 3);
        let mut iter = arena.iter();
        assert_eq!(iter.next().map(|node| *node.get()), Some(0));
        assert_eq!(iter.next().map(|node| *node.get()), Some(1));
        assert_eq!(iter.next().map(|node| *node.get()), Some(2));
    }

    #[test]
    fn test_parses_escaped() {
        let mut hs = LabelDict::new();
        let input = String::from(
            r#"{article{key{journals/corr/abs-0812-2567}}{mdate{2017-06-07}}{publtype{informal}}{author{Jian Li}}{title{An O(log n / log log n\\}\\}) Upper Bound on the Price of Stability for Undirected Shapley Network Design Games}}{ee{http://arxiv.org/abs/0812.2567}}{year{2008}}{journal{CoRR}}{volume{abs/0812.2567}}{url{db/journals/corr/corr0812.html#abs-0812-2567}}}"#,
        );
        let arena = parse_tree(Ok(input), &mut hs);
        assert!(arena.is_ok());
        assert_eq!(arena.unwrap().count(), 21);
    }

    #[test]
    fn test_label_dict_preserved_label_ids() {
        // test label ids are not overwritten when parsing another tree
        let mut ld = LabelDict::new();
        let _t1 = parse_tree(Ok("{b{e}{d{a}}}".to_owned()), &mut ld).unwrap();
        let _t2 = parse_tree(Ok("{d{c}{f{g}{d{a}}}}".to_owned()), &mut ld).unwrap();

        assert_eq!(
            ld,
            LabelDict::from([
                ("b".to_owned(), 0),
                ("e".to_owned(), 1),
                ("d".to_owned(), 2),
                ("a".to_owned(), 3),
                ("c".to_owned(), 4),
                ("f".to_owned(), 5),
                ("g".to_owned(), 6),
            ]),
            "Label dict label ids were not preserved!"
        );
    }

    #[test]
    fn test_descendants_correct() {
        let input = "{first{second{third}{fourth{fifth{six}{seven}}}}".to_owned();
        let mut hs = LabelDict::new();
        let arena = parse_tree(Ok(input), &mut hs);
        assert!(arena.is_ok());
        let arena = arena.unwrap();
        let Some(root) = arena.iter().next() else {
            panic!("Unable to get root but tree is not empty!");
        };
        let root_id = arena.get_node_id(root).unwrap();
        let mut iter = root_id.descendants(&arena);

        let rd = iter.next();
        assert!(rd.is_some());
        assert_eq!(
            arena.get(rd.unwrap()).map(|node| node.get()),
            Some(0).as_ref()
        );
        assert_eq!(
            arena.get(iter.next().unwrap()).map(|node| node.get()),
            Some(1).as_ref()
        );
        assert_eq!(
            arena.get(iter.next().unwrap()).map(|node| node.get()),
            Some(2).as_ref()
        );
        assert_eq!(
            arena.get(iter.next().unwrap()).map(|node| node.get()),
            Some(3).as_ref()
        );
        assert_eq!(
            arena.get(iter.next().unwrap()).map(|node| node.get()),
            Some(4).as_ref()
        );
        assert_eq!(
            arena.get(iter.next().unwrap()).map(|node| node.get()),
            Some(5).as_ref()
        );
        assert_eq!(
            arena.get(iter.next().unwrap()).map(|node| node.get()),
            Some(6).as_ref()
        );
    }

    #[test]
    fn test_parses_empty_label() {
        let input = "{wendelsteinstrasse{1{{1}{2}{3}{4}{5}{6}{7}{14}}}}".to_owned();
        let mut hs = LabelDict::new();
        let arena = parse_tree(Ok(input), &mut hs);
        assert!(arena.is_ok());
        let arena = arena.unwrap();
        assert_eq!(
            arena.count(),
            11,
            "Parser did not deal with empty label accordingly"
        );
    }
}
