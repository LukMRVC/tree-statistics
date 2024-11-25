use crossbeam_channel::Sender;
// use gxhash::{HashMap, HashMapExt};
use indextree::{Arena, NodeEdge, NodeId};
use itertools::Itertools;
use memchr::memchr2_iter;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::num::NonZeroUsize;
use std::path::Path;
use std::string::String;
use std::sync::{Arc, Mutex};
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

// the index is the labelId, and the value on that index is the frequency of it
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LabelFreqOrdering<T = usize>(Vec<T>);

impl<T> LabelFreqOrdering<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self(data)
    }

    pub fn get(&self, index: NonZeroUsize) -> Option<&T> {
        self.0.get(index.get() - 1)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

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

pub fn get_frequency_ordering(ld: &LabelDict) -> LabelFreqOrdering {
    LabelFreqOrdering(ld.values().sorted_by_key(|(label, _)| label).fold(
        Vec::with_capacity(ld.values().len()),
        |mut ordering, (_, label_count)| {
            ordering.push(*label_count);
            ordering
        },
    ))
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
    let (sender, receiver) = crossbeam_channel::unbounded::<String>();
    let ld = Arc::new(Mutex::new(label_dict));
    let copy_ld = Arc::clone(&ld);
    let collection_tree_tokens = std::thread::scope(|s| {
        s.spawn(move || {
            let mut ld = copy_ld.lock().unwrap();
            let mut max_node_id = ld.values().len() as LabelId;
            while let Ok(label) = receiver.recv() {
                ld.entry(label)
                    .and_modify(|(_, lblcnt)| *lblcnt += 1)
                    .or_insert_with(|| {
                        max_node_id += 1;
                        (max_node_id, 1)
                    });
            }
        });

        let reader = BufReader::new(File::open(dataset_file).unwrap());
        let tree_lines = reader
            .lines()
            .collect::<Result<Vec<String>, _>>()
            .expect("Unable to read input file");
        // println!("Consumed {} lines of trees", tree_lines.len());

        tree_lines
            .into_par_iter()
            .map_with(sender, |s, tree_line| {
                if !tree_line.is_ascii() {
                    return Err(TreeParseError::IsNotAscii);
                }
                parse_tree_tokens(tree_line, Some(s))
            })
            .filter(Result::is_ok)
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    });

    // println!(
    //     "Parsed {} lines of tree tokens",
    //     collection_tree_tokens.len()
    // );
    // println!("Parsing tokens into trees");
    let label_dict = Arc::try_unwrap(ld)
        .expect("Arc has references")
        .into_inner()
        .unwrap();
    let trees = collection_tree_tokens
        .par_iter()
        .map(|tokens| parse_tree(tokens, label_dict))
        .filter(Result::is_ok)
        .collect::<Result<Vec<_>, _>>()?;
    // println!("Final number of trees: {}", trees.len());

    Ok(trees)
}

pub fn parse_queries(
    query_file: &impl AsRef<Path>,
    ld: &mut LabelDict,
) -> Result<Vec<(usize, ParsedTree)>, DatasetParseError> {
    let reader = buf_open_file!(query_file);
    let trees: Vec<(usize, Vec<String>)> = reader
        .lines()
        .filter_map(|l| {
            let l = l.expect("line reading failed!");
            let (threshold_str, tree) = l.split_once(";")?;
            Some((threshold_str.parse::<usize>().unwrap(), tree.to_string()))
        })
        .filter_map(|(t, tree)| {
            let tokens = parse_tree_tokens(tree, None);
            if tokens.is_err() {
                return None;
            }
            let tks: Vec<String> = tokens
                .unwrap()
                .iter()
                .map(|tkn| tkn.to_string())
                .collect_vec();

            Some((t, tks))
        })
        .collect::<Vec<_>>();

    let only_tokens = trees
        .iter()
        .map(|(_, tkns)| tkns.iter().map(|t| t.as_str()).collect_vec())
        .collect_vec();

    update_label_dict(&only_tokens, ld);
    let trees = trees
        .iter()
        .filter_map(|(t, tokens)| {
            let parsed_tree = parse_tree(&tokens, ld);
            if parsed_tree.is_err() {
                return None;
            }

            Some((*t, parsed_tree.unwrap()))
        })
        .collect();

    Ok(trees)
}

pub fn parse_single(tree_str: String, label_dict: &mut LabelDict) -> ParsedTree {
    if !tree_str.is_ascii() {
        panic!("Passed tree string is not ASCII");
    }

    let tokens = parse_tree_tokens(tree_str, None).expect("Failed to parse single tree");
    let str_tokens = tokens.iter().map(|t| t.as_str()).collect_vec();
    let token_col = vec![str_tokens];
    update_label_dict(&token_col, label_dict);
    parse_tree(&tokens, label_dict).unwrap()
}

pub fn update_label_dict(tokens_collection: &[Vec<&str>], ld: &mut LabelDict) {
    let labels_only = tokens_collection
        .par_iter()
        .flat_map(|tree_tokens| {
            tree_tokens
                .iter()
                .filter(|token| **token != "{" && **token != "}")
                .map(|label_token| label_token.to_string())
                .collect_vec()
        })
        .collect::<Vec<_>>();

    let mut max_node_id = ld.values().len() as LabelId;
    for lbl in labels_only {
        ld.entry(lbl)
            .and_modify(|(_, lblcnt)| *lblcnt += 1)
            .or_insert_with(|| {
                max_node_id += 1;
                (max_node_id, 1)
            });
    }
}

pub fn parse_tree(tokens: &[String], ld: &LabelDict) -> Result<ParsedTree, TreeParseError> {
    let mut tree_arena = ParsedTree::with_capacity(tokens.len() / 2);
    let mut node_stack: Vec<NodeId> = vec![];

    for t in tokens.iter().skip(1) {
        match t.as_str() {
            "{" => continue,
            "}" => {
                let Some(_) = node_stack.pop() else {
                    return Err(TreeParseError::IncorrectFormat(
                        "Wrong bracket pairing".to_owned(),
                    ));
                };
            }
            label_str => {
                let Some((label, _)) = ld.get(label_str) else {
                    return Err(TreeParseError::TokenizerError);
                };
                let n = tree_arena.new_node(*label);
                if let Some(last_node) = node_stack.last() {
                    last_node.append(n, &mut tree_arena);
                } else if tree_arena.count() > 1 {
                    return Err(TreeParseError::IncorrectFormat(
                        "Reached unexpected end of token".to_owned(),
                    ));
                };
                node_stack.push(n);
            }
        }
    }

    Ok(tree_arena)
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

fn braces_parity_check(parity: &mut i32, addorsub: i32) -> Result<(), TreeParseError> {
    *parity += addorsub;
    if *parity < 0 {
        return Err(TreeParseError::IncorrectFormat(
            "Parity of brces does not match".to_owned(),
        ));
    }
    Ok(())
}

fn parse_tree_tokens<'a>(
    tree_bytes: String,
    sender_channel: Option<&mut Sender<String>>,
) -> Result<Vec<String>, TreeParseError> {
    use TreeParseError as TPE;

    let tree_bytes = tree_bytes.as_bytes();
    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(TPE::IncorrectFormat(
            "Minimal of 2 brackets not found!".to_owned(),
        ));
    }

    let mut str_tokens = vec![];
    let mut parity_check = 0;

    let mut token_iterator = token_positions.iter().peekable();

    while let Some(token_pos) = token_iterator.next() {
        match tree_bytes[*token_pos] {
            TOKEN_START => {
                braces_parity_check(&mut parity_check, 1)?;
                unsafe {
                    str_tokens.push(String::from_utf8_unchecked(
                        tree_bytes[*token_pos..(token_pos + 1)].to_vec(),
                    ));
                }
                let Some(token_end) = token_iterator.peek() else {
                    let err_msg = format!("Label has no ending token near col {token_pos}");
                    return Err(TPE::IncorrectFormat(err_msg));
                };
                let label = unsafe {
                    String::from_utf8_unchecked(tree_bytes[(token_pos + 1)..**token_end].to_vec())
                };
                str_tokens.push(label.clone());
                if let Some(ref s) = sender_channel {
                    s.send(label).expect("Failed sending label");
                }
            }
            TOKEN_END => {
                braces_parity_check(&mut parity_check, -1)?;
                let label = unsafe {
                    String::from_utf8_unchecked(tree_bytes[*token_pos..(token_pos + 1)].to_vec())
                };
                str_tokens.push(label.clone());
                if let Some(ref s) = sender_channel {
                    s.send(label).expect("Failed sending label");
                }
            }
            _ => return Err(TPE::TokenizerError),
        }
    }
    Ok(str_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_into_tokens() {
        let input = "{einsteinstrasse{1}{3}}".to_owned();
        let tokens = parse_tree_tokens(input, None);
        assert!(tokens.is_ok());
        let tokens = tokens.unwrap();
        assert_eq!(
            tokens,
            vec!["{", "einsteinstrasse", "{", "1", "}", "{", "3", "}", "}"]
        );
    }

    #[test]
    fn test_parses_escaped() {
        use std::string::String;
        let input = String::from(r#"{article{key{An optimization of \log data}}}"#);
        let tokens = parse_tree_tokens(input, None);
        assert!(tokens.is_ok());
        let tokens = tokens.unwrap();
        assert_eq!(
            tokens,
            vec![
                "{",
                "article",
                "{",
                "key",
                "{",
                r"An optimization of \log data",
                "}",
                "}",
                "}"
            ]
        );
    }

    #[test]
    fn test_parses_into_tree_arena() {
        let input = "{einsteinstrasse{1}{3}}".to_owned();
        let tokens = parse_tree_tokens(input, None);
        let tokens = tokens.unwrap();
        let ld = LabelDict::from([
            ("einsteinstrasse".to_owned(), (1, 1)),
            ("1".to_owned(), (2, 1)),
            ("3".to_owned(), (3, 1)),
        ]);
        let tree_arena = parse_tree(&tokens, &ld).unwrap();
        let mut arena = ParsedTree::new();

        let n1 = arena.new_node(1);
        let n2 = arena.new_node(2);
        let n3 = arena.new_node(3);
        n1.append(n2, &mut arena);
        n1.append(n3, &mut arena);

        assert_eq!(tree_arena, arena);
    }

    #[test]
    fn test_updated_label_dict() {
        let input = "{einsteinstrasse{1}{3}}".to_owned();
        let tokens = parse_tree_tokens(input, None);
        let tokens = tokens.unwrap();
        let input2 = "{weinsteinstrasse{3}{2}}".to_owned();
        let tokens2 = parse_tree_tokens(input2, None);
        let tokens2 = tokens2.unwrap();
        let mut ld = LabelDict::default();
        let token_col = vec![tokens, tokens2];
        // update_label_dict(&token_col, &mut ld);

        let tld = LabelDict::from([
            ("einsteinstrasse".to_owned(), (1, 1)),
            ("1".to_owned(), (2, 1)),
            ("3".to_owned(), (3, 2)),
            ("weinsteinstrasse".to_owned(), (4, 1)),
            ("2".to_owned(), (5, 1)),
        ]);
        assert_eq!(ld, tld, "Label dicts are equal");
    }

    #[test]
    fn test_frequency_ordering_build() {
        let ld: LabelDict = LabelDict::from([
            ("A".to_owned(), (0, 5)),
            ("B".to_owned(), (1, 2)),
            ("C".to_owned(), (2, 3)),
            ("D".to_owned(), (3, 1)),
            ("F".to_owned(), (4, 5)),
        ]);

        let freq_ordering = get_frequency_ordering(&ld);
        assert_eq!(freq_ordering, LabelFreqOrdering::new(vec![5, 2, 3, 1, 5]));

        let mut values = vec![0, 2, 3, 0, 4];
        values.sort_by_key(|lbl| {
            freq_ordering
                .get(NonZeroUsize::new(*lbl as usize).unwrap())
                .unwrap()
        });

        assert_eq!(values, vec![3, 2, 0, 0, 4]);
    }

    /*

    #[test]
    fn test_label_dict_preserved_label_ids() {
        // test label ids are not overwritten when parsing another tree
        let mut ld = LabelDict::new();
        let _t1 = parse_tree(Ok("{b{e}{d{a}}}".to_owned())).unwrap();
        let _t2 = parse_tree(Ok("{d{c}{f{g}{d{a}}}}".to_owned())).unwrap();

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
        let arena = parse_tree(Ok(input));
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
        let arena = parse_tree(Ok(input));
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
        let tree = parse_tree(Ok(input));
        assert!(tree.is_err());
    }

     */
}
