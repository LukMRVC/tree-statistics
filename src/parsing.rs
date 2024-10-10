use dashmap::DashMap;
use indextree::{Arena, NodeEdge};
use memchr::memchr2_iter;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::string::String;
use std::sync::RwLock;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetParseError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    ParseError(#[from] TreeParseError),
}

pub type LabelId = i32;

pub type LabelDict = HashMap<String, (LabelId, usize)>;
pub(crate) type ParsedTree = Arena<LabelId>;

pub enum TreeOutput {
    BracketNotation,
    Graphviz,
}

pub fn tree_to_string(tree: &ParsedTree, out_type: TreeOutput) -> String {
    match out_type {
        TreeOutput::BracketNotation => tree_to_bracket(tree),
        TreeOutput::Graphviz => tree_to_graphviz(tree),
    }
}

fn tree_to_graphviz(tree: &ParsedTree) -> String {
    let mut graphviz = String::with_capacity(tree.count() * 4);
    graphviz.push_str("strict digraph G {\n");
    let mut nodeid_stack = vec![];
    let Some(root) = tree.iter().next() else {
        panic!("Root not found!");
    };
    let root_id = tree.get_node_id(root).expect("Root ID not found!");
    nodeid_stack.push((root_id, format!("A{}", root.get())));
    while let Some((nid, lbl_str)) = nodeid_stack.pop() {
        for (idx, cnid) in nid.children(tree).enumerate() {
            let label = tree.get(cnid).unwrap().get();
            let ascii_char = char::from_u32(idx as u32 + 65).unwrap();
            graphviz.push_str(&format!("{lbl_str} -> {ascii_char}{label};\n"));
            nodeid_stack.push((cnid, format!("{ascii_char}{label}")));
        }
    }
    graphviz.push('}');
    graphviz.push('\n');
    graphviz
}

fn tree_to_bracket(tree: &ParsedTree) -> String {
    let mut bracket_notation = String::with_capacity(tree.count() * 4);
    let Some(root) = tree.iter().next() else {
        panic!("Root not found!");
    };
    let root_id = tree.get_node_id(root).expect("Root ID not found!");

    for edge in root_id.traverse(tree) {
        match edge {
            NodeEdge::Start(node_id) => {
                bracket_notation.push('{');
                bracket_notation.push_str(&tree.get(node_id).unwrap().get().to_string());
            }
            NodeEdge::End(_) => {
                bracket_notation.push('}');
            }
        }
    }

    bracket_notation
}

macro_rules! buf_open_file {
    ($file_path:ident) => {
        BufReader::new(File::open($file_path)?)
    };
}

pub fn parse_dataset(
    dataset_file: &impl AsRef<Path>,
    label_dict: &mut LabelDict,
) -> Result<Vec<ParsedTree>, DatasetParseError> {
    let mut line_counter = 0;
    let reader = buf_open_file!(dataset_file);
    let trees: Vec<ParsedTree> = reader
        .lines()
        .map(|l| parse_tree(l, label_dict))
        .inspect(|_| {
            line_counter += 1;
            if line_counter % 10_000 == 0 {
                println!("Parsed {line_counter} lines so far...");
            }
        })
        .filter(Result::is_ok)
        .collect::<Result<Vec<_>, _>>()?;

    println!("Consumed {line_counter} lines of trees");
    Ok(trees)
}

pub fn parse_dataset_concurrent(
    dataset_file: &impl AsRef<Path>,
    label_dict: &mut LabelDict,
) -> Result<Vec<ParsedTree>, DatasetParseError> {
    let mut line_counter = 0;
    let reader = buf_open_file!(dataset_file);
    let mut all_lines = Vec::with_capacity(5000);
    for line in reader.lines() {
        all_lines.push(line?);
    }

    let concurrent_map = std::sync::Arc::new(DashMap::with_capacity(all_lines.len() * 2));
    let mut max_node_id = std::sync::Arc::new(RwLock::new(1));
    println!("Read {lines} lines of trees", lines = all_lines.len());

    let trees = all_lines
        .par_iter()
        .map(|tree_str| {
            use TreeParseError as TPE;
            if !tree_str.is_ascii() {
                return Err(TPE::IsNotAscii);
            }
            let mut tree = ParsedTree::with_capacity(tree_str.len() / 2);
            let tree_bytes = tree_str.as_bytes();

            let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
                .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
                .collect();

            if token_positions.len() < 2 {
                return Err(TPE::IncorrectFormat(
                    "Minimal of 2 brackets not found!".to_owned(),
                ));
            }

            let mut tokens = token_positions.iter().peekable();
            let root_start = *tokens.next().unwrap();
            let root_end = **tokens.peek().unwrap();

            let root_label = unsafe {
                String::from_utf8_unchecked(tree_bytes[(root_start + 1)..root_end].to_vec())
            };
            let is_first_label_in_map = concurrent_map.is_empty();
            let root_label = concurrent_map
                .entry(root_label)
                .and_modify(|(_, counter)| {
                    *counter += 1;
                })
                .or_insert_with(|| (*max_node_id.read().unwrap(), 1));
            let root = tree.new_node(root_label.0);
            let mut node_stack = vec![root];
            while let Some(token) = tokens.next() {
                match tree_bytes[*token] {
                    TOKEN_START => {
                        let Some(token_end) = tokens.peek() else {
                            let err_msg = format!(
                                "Label has no ending token near col {token} , line \"{tree_str}\""
                            );
                            return Err(TPE::IncorrectFormat(err_msg));
                        };
                        let label = unsafe {
                            String::from_utf8_unchecked(
                                tree_bytes[(*token + 1)..**token_end].to_vec(),
                            )
                        };

                        let node_label = concurrent_map
                            .entry(label)
                            .and_modify(|(_, counter)| {
                                *counter += 1;
                            })
                            .or_insert_with(|| {
                                let mut max_id = max_node_id.write().unwrap();
                                *max_id += 1;
                                (*max_id, 1)
                            });

                        let n = tree.new_node(node_label.0);
                        let Some(last_node) = node_stack.last() else {
                            let err_msg =
                                format!("Reached unexpected end of token on line \"{tree_str}\"");
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
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(trees)
}

pub fn parse_queries(
    query_file: &impl AsRef<Path>,
    ld: &mut LabelDict,
) -> Result<Vec<(usize, ParsedTree)>, DatasetParseError> {
    let reader = buf_open_file!(query_file);
    let trees: Vec<(usize, ParsedTree)> = reader
        .lines()
        .map(|l| {
            let l = l.expect("line reading failed!");
            let Some((threshold_str, tree)) = l.split_once(";") else {
                return (
                    0,
                    Err(TreeParseError::IncorrectFormat(
                        "(Could not parse query line!)".to_owned(),
                    )),
                );
            };

            (
                threshold_str.parse::<usize>().unwrap(),
                parse_tree(Ok(tree.to_owned()), ld),
            )
        })
        .filter_map(|(t, tree_result)| {
            if tree_result.is_err() {
                return None;
            }
            Some((t, tree_result.unwrap()))
        })
        .collect::<Vec<_>>();

    Ok(trees)
}

const TOKEN_START: u8 = b'{';
const TOKEN_END: u8 = b'}';
const ESCAPE_CHAR: u8 = b'\\';

#[inline(always)]
fn is_escaped(byte_string: &[u8], offset: usize) -> bool {
    offset > 0
        && byte_string[offset - 1] == ESCAPE_CHAR
        && !(offset > 1 && byte_string[offset - 2] == ESCAPE_CHAR)
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

pub fn parse_tree(
    tree_str: Result<String, io::Error>,
    label_map: &mut LabelDict,
) -> Result<ParsedTree, TreeParseError> {
    use TreeParseError as TPE;

    let tree_str = tree_str?;
    if !tree_str.is_ascii() {
        return Err(TPE::IsNotAscii);
    }
    let mut tree = ParsedTree::with_capacity(tree_str.len() / 2);
    let tree_bytes = tree_str.as_bytes();

    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(TPE::IncorrectFormat(
            "Minimal of 2 brackets not found!".to_owned(),
        ));
    }

    let (mut max_node_id, _) = label_map.values().max().cloned().unwrap_or((0, 1));

    let mut tokens = token_positions.iter().peekable();
    let root_start = *tokens.next().unwrap();
    let root_end = **tokens.peek().unwrap();

    let root_label =
        unsafe { String::from_utf8_unchecked(tree_bytes[(root_start + 1)..root_end].to_vec()) };
    let is_first_label_in_map = label_map.is_empty();
    let root_label = label_map
        .entry(root_label)
        .and_modify(|(_, counter)| {
            *counter += 1;
        })
        .or_insert_with(|| {
            if !is_first_label_in_map {
                max_node_id += 1;
            }
            (max_node_id, 1)
        });
    let root = tree.new_node(root_label.0);
    let mut node_stack = vec![root];

    while let Some(token) = tokens.next() {
        match tree_bytes[*token] {
            TOKEN_START => {
                let Some(token_end) = tokens.peek() else {
                    let err_msg =
                        format!("Label has no ending token near col {token} , line \"{tree_str}\"");
                    return Err(TPE::IncorrectFormat(err_msg));
                };
                let label = unsafe {
                    String::from_utf8_unchecked(tree_bytes[(*token + 1)..**token_end].to_vec())
                };

                let node_label = label_map
                    .entry(label)
                    .and_modify(|(_, counter)| {
                        *counter += 1;
                    })
                    .or_insert_with(|| {
                        max_node_id += 1;
                        (max_node_id, 1)
                    });

                let n = tree.new_node(node_label.0);
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
        use std::string::String;
        let mut hs = LabelDict::new();
        let input = String::from(
            r#"{article{key{journals/corr/abs-0812-2567}}{mdate{2017-06-07}}{publtype{informal}}{author{Jian Li}}{title{An O(log n / log log n\}\}) Upper Bound on the Price of Stability for Undirected Shapley Network Design Games}}{ee{http://arxiv.org/abs/0812.2567}}{year{2008}}{journal{CoRR}}{volume{abs/0812.2567}}{url{db/journals/corr/corr0812.html#abs-0812-2567}}}"#,
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
                ("b".to_owned(), (0, 1)),
                ("e".to_owned(), (1, 1)),
                ("d".to_owned(), (2, 3)),
                ("a".to_owned(), (3, 2)),
                ("c".to_owned(), (4, 1)),
                ("f".to_owned(), (5, 1)),
                ("g".to_owned(), (6, 1)),
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

    #[test]
    fn test_invalid_escape() {
        let input = r"{article{key{journals/corr/FongT15b}}{mdate{2017-06-07}}{publtype{informal withdrawn}}{title{On the Empirical Output Distribution of $\\}varepsilon$-Good Codes for Gaussian Channels under a Long-Term Power Constraint.}}{year{2015}}{volume{abs/1510.08544}}{journal{CoRR}}{ee{http://arxiv.org/abs/1510.08544}}{url{db/journals/corr/corr1510.html#FongT15b}}}".to_owned();
        let mut ld = LabelDict::new();
        let tree = parse_tree(Ok(input), &mut ld);
        assert!(tree.is_err());
    }
}
