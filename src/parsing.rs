// use crossbeam_channel::Sender; // Removed: no longer using crossbeam channel
// use gxhash::{HashMap, HashMapExt};
use indextree::{Arena, NodeEdge, NodeId};
use itertools::Itertools;
use memchr::memchr2_iter;
use rayon::prelude::*;
use rustc_hash::FxHashMap;
use serde::de;
use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::num::NonZeroUsize;
use std::path::Path;
use std::string::String;
use std::sync::atomic::{AtomicI32, Ordering};
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

pub type LabelDict = FxHashMap<String, (LabelId, usize)>;

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
    // Use scc::HashMap for lock-free concurrent label dictionary during parsing
    let scc_label_dict = scc::HashMap::new();

    // Initialize scc::HashMap with existing label_dict entries and find max ID
    let max_node_id = AtomicI32::new(label_dict.values().map(|(id, _)| *id).max().unwrap_or(0));

    for (label, (id, count)) in label_dict.iter() {
        let _ = scc_label_dict.insert_sync(label.clone(), (*id, *count));
    }

    let reader = BufReader::new(File::open(dataset_file).unwrap());

    // Parse in parallel while tracking original index for stable ordering
    // enumerate() is lazy, par_bridge() streams directly from the reader
    let mut trees: Vec<(usize, ParsedTree)> = reader
        .lines()
        .enumerate()
        .par_bridge()
        .filter_map(|(idx, tree_line)| {
            let tree_line = tree_line.ok()?;
            if !tree_line.is_ascii() {
                return None;
            }

            parse_tree_directly(&tree_line, &scc_label_dict, &max_node_id)
                .ok()
                .map(|tree| (idx, tree))
        })
        .collect();

    // Convert scc::HashMap to FxHashMap using scan_sync
    label_dict.clear();
    scc_label_dict.retain_sync(|label, (id, count)| {
        label_dict.insert(label.clone(), (*id, *count));
        false
    });

    // Stable sort: by tree count, then by original index as tiebreaker
    trees.sort_by(|(idx_a, a), (idx_b, b)| a.count().cmp(&b.count()).then(idx_a.cmp(idx_b)));

    // Extract just the trees, discarding indices
    let trees = trees.into_iter().map(|(_, tree)| tree).collect();

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
            let parsed_tree = parse_tree(tokens, ld);
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
    let mut tree_arena = ParsedTree::with_capacity(tokens.len() / 4);
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
    offset > 0 && byte_string[offset - 1] == ESCAPE_CHAR
    // && !(offset > 1 && byte_string[offset - 2] == ESCAPE_CHAR)
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
            "Parity of braces does not match".to_owned(),
        ));
    }
    Ok(())
}

fn parse_tree_tokens(
    tree_line: String,
    _sender_channel: Option<&mut ()>, // Deprecated parameter, kept for compatibility
) -> Result<Vec<String>, TreeParseError> {
    use TreeParseError as TPE;

    let tree_bytes = tree_line.as_bytes();
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
                str_tokens.push(label);
            }
            TOKEN_END => {
                braces_parity_check(&mut parity_check, -1)?;
                let label = unsafe {
                    String::from_utf8_unchecked(tree_bytes[*token_pos..(token_pos + 1)].to_vec())
                };
                str_tokens.push(label);
            }
            _ => return Err(TPE::TokenizerError),
        }
    }
    Ok(str_tokens)
}

// Fused parsing: tokenize and build tree directly without intermediate Vec<String>
// Eliminates brace tokens entirely - only stores label IDs in the Arena
fn parse_tree_directly(
    tree_line: &str,
    label_dict: &scc::HashMap<String, (LabelId, usize)>,
    max_node_id: &AtomicI32,
) -> Result<ParsedTree, TreeParseError> {
    use TreeParseError as TPE;

    let tree_bytes = tree_line.as_bytes();
    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(TPE::IncorrectFormat(
            "Minimal of 2 brackets not found!".to_owned(),
        ));
    }

    // Estimate tree size: roughly half of tokens are labels (the other half are braces)
    let estimated_nodes = token_positions.len() / 2;
    let mut tree_arena = ParsedTree::with_capacity(estimated_nodes);
    let mut node_stack: Vec<NodeId> = Vec::with_capacity(32); // typical tree depth
    let mut parity_check = 0;

    let mut token_iterator = token_positions.iter().peekable();

    while let Some(token_pos) = token_iterator.next() {
        match tree_bytes[*token_pos] {
            TOKEN_START => {
                braces_parity_check(&mut parity_check, 1)?;

                let Some(token_end) = token_iterator.peek() else {
                    let err_msg = format!("Label has no ending token near col {token_pos}");
                    return Err(TPE::IncorrectFormat(err_msg));
                };

                // Extract label bytes without allocating String for braces
                let label_bytes = &tree_bytes[(token_pos + 1)..**token_end];

                // Skip empty labels or escaped braces that result in brace-only labels
                if label_bytes.is_empty() || label_bytes == b"{" || label_bytes == b"}" {
                    continue;
                }

                // Convert to string only for the label lookup/insert
                let label = unsafe { String::from_utf8_unchecked(label_bytes.to_vec()) };

                // Get or insert label ID from concurrent hashmap
                let label_id = {
                    let entry = label_dict.entry_sync(label);
                    match entry {
                        scc::hash_map::Entry::Occupied(mut occ) => {
                            let (id, count) = occ.get_mut();
                            *count += 1;
                            *id
                        }
                        scc::hash_map::Entry::Vacant(vac) => {
                            let new_id = max_node_id.fetch_add(1, Ordering::Relaxed) + 1;
                            vac.insert_entry((new_id, 1));
                            new_id
                        }
                    }
                };

                // Create node and append to tree
                let node = tree_arena.new_node(label_id);
                if let Some(parent) = node_stack.last() {
                    parent.append(node, &mut tree_arena);
                } else if tree_arena.count() > 1 {
                    return Err(TPE::IncorrectFormat(
                        "Multiple root nodes detected".to_owned(),
                    ));
                }
                node_stack.push(node);
            }
            TOKEN_END => {
                braces_parity_check(&mut parity_check, -1)?;
                if node_stack.pop().is_none() {
                    return Err(TPE::IncorrectFormat("Wrong bracket pairing".to_owned()));
                }
            }
            _ => return Err(TPE::TokenizerError),
        }
    }

    if parity_check != 0 {
        return Err(TPE::IncorrectFormat("Unbalanced brackets".to_owned()));
    }

    Ok(tree_arena)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_into_tokens() {
        let input = "{NP{NP{NNS{Fees}}}{QP{CD{1}}{CD{3\\}/4}}}{Interpunction{.}}}".to_owned();
        let tokens = parse_tree_tokens(input, None);
        assert!(tokens.is_err());
        let tokens = tokens.unwrap();
    }

    #[test]
    fn parsing_should_fail_on_incorrect_brackets() {
        let input = r#"{entry{created{1997-11-01}}{dataset{Swiss-Prot}}{modified{2023-02-22}}{version{57}}{xmlns{http://uniprot.org/uniprot}}{accession{Q50834}}{name{MFNF_METVA}}{protein{recommendedName{fullName{evidence{1}}{(4-\\}{4-[2-(gamma-L-glutamylamino)ethyl]phenoxymethyl\\}}furan-2-yl)methanamine synthase}}{ecNumber{evidence{1}}{2.5.1.131}}}{alternativeName{fullName{evidence{1}}{4-[[4-(2-aminoethyl)phenoxy]-methyl]-2-furanmethanamine-glutamate synthase}}{shortName{evidence{1}}{APMF-Glu synthase}}}}{gene{name{evidence{1}}{type{primary}}{mfnF}}}{organism{name{type{scientific}}{Methanococcus vannielii}}{lineage{taxon{Archaea}}{taxon{Euryarchaeota}}{taxon{Methanomada group}}{taxon{Methanococci}}{taxon{Methanococcales}}{taxon{Methanococcaceae}}{taxon{Methanococcus}}}}{reference{key{1}}{citation{date{1988}}{first{3125}}{last{3130}}{name{J. Bacteriol.}}{type{journal article}}{volume{170}}{title{Conservation of structure in the human gene encoding argininosuccinate synthetase and the argG genes of the archaebacteria Methanosarcina barkeri MS and Methanococcus vannielii.}}{authorList}}{scope{NUCLEOTIDE SEQUENCE [GENOMIC DNA]}}}{comment{type{function}}{text{evidence{1}}{Catalyzes the condensation between 5-(aminomethyl)-3-furanmethanol diphosphate (F1-PP) and gamma-glutamyltyramine to produce APMF-Glu.}}}{comment{type{catalytic activity}}{reaction{evidence{1}}{text{[5-(aminomethyl)furan-3-yl]methyl diphosphate + gamma-L-glutamyltyramine = (4-\\}{4-[2-(gamma-L-glutamylamino)ethyl]phenoxymethyl\\}}furan-2-yl)methanamine + diphosphate}}}}{comment{type{pathway}}{text{evidence{1}}{Cofactor biosynthesis; methanofuran biosynthesis.}}}{comment{type{similarity}}{text{evidence{2}}{Belongs to the MfnF family.}}}{dbReference{id{M21315}}{type{EMBL}}}{dbReference{id{GO:0016787}}{type{GO}}}{dbReference{id{GO:0016740}}{type{GO}}}{dbReference{id{3.30.420.190}}{type{Gene3D}}}{dbReference{id{IPR002821}}{type{InterPro}}}{dbReference{id{PF01968}}{type{Pfam}}}{keyword{id{KW-0808}}{Transferase}}{feature{description{(4-\\}{4-[2-(gamma-L-glutamylamino)ethyl]phenoxymethyl\\}}furan-2-yl)methanamine synthase}}{id{PRO_0000107077}}{type{chain}}{location}}{feature{type{non-terminal residue}}{location}}{evidence{key{1}}{type{ECO:0000250}}{source}}{sequence{checksum{2AA3DC7D3A0105DE}}{fragment{single}}{length{222}}{mass{24655}}{modified{1996-11-01}}{version{1}}{AEFVSQNIDKNCILVDMGSTTTDIIPIVDGKAASNKTDLERLMNNELLYVGSLRTPLSFLSNKIMFKDTITNVSSEYFAITGDISLVLDKITEMDYSCDTPDGKPADKRNSLIRISKVLCSDLNQISADESINIAIEYYKILIDLILENVKKVSEKYGLKNIVITGLGEEILKDALSELTKSNEFNIISIKERYGKDVSLATPSFSVSILLKNELNAKLNRS}}}"#.to_owned(); // missing closing brace
        let tokens = parse_tree_tokens(input, None);
        assert!(tokens.is_err(), "Parsing should fail on incorrect brackets");
    }

    #[test]
    fn test_parses_into_tokens_2() {
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
        let ld = LabelDict::from_iter([
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

        let tld = LabelDict::from_iter([
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
        let ld: LabelDict = LabelDict::from_iter([
            ("A".to_string(), (0i32, 5usize)),
            ("B".to_string(), (1i32, 2usize)),
            ("C".to_string(), (2i32, 3usize)),
            ("D".to_string(), (3i32, 1usize)),
            ("F".to_string(), (4i32, 5usize)),
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

    #[test]
    fn test_parses_deep_tree() {
        let input = "{1{5{1}{5{5{5{5}}}}{3{3{5}}}}{3{3{3}}{3{3{3{3{3}}}{5{3}}}}{3{1{3{5{1}}}}{3{3}}}}{5{2{1}}}}".to_owned();
        let tokens = parse_tree_tokens(input, None);
        assert!(tokens.is_ok());
        let tokens = tokens.unwrap();
        let mut ld = LabelDict::default();
        update_label_dict(&[tokens.iter().map(|t| t.as_str()).collect()], &mut ld);
        assert!(ld.get(r"}").is_none());
    }

    /*

    #[test]
    fn test_label_dict_preserved_label_ids() {
        // test label ids are not overwritten when parsing another tree
        let mut ld = LabelDict::default();
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
        let mut hs = LabelDict::default();
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
        let mut hs = LabelDict::default();
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
        let mut ld = LabelDict::default();
        let tree = parse_tree(Ok(input));
        assert!(tree.is_err());
    }

     */
}
