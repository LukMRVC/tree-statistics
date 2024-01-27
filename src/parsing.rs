use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use indextree::Arena;
use thiserror::Error;
use memchr::memchr2_iter;


#[derive(Error, Debug)]
pub enum DatasetParseError {
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error(transparent)]
    ParseError(#[from] TreeParseError),
}


pub fn parse_dataset(dataset_file: PathBuf) -> Result<Vec<indextree::Arena<String>>, DatasetParseError> {
    let f = File::open(dataset_file)?;
    let reader = BufReader::new(f);
    let trees: Vec<indextree::Arena<String>> = reader.lines()
        .map(parse_tree)
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
    TokenizerError
}


fn parse_tree(tree_str: Result<String, io::Error>) -> Result<Arena<String>, TreeParseError> {
    use TreeParseError as TPE;

    let tree_str = tree_str?;
    if !tree_str.is_ascii() {
        return Err(TPE::IsNotAscii);
    }
    let mut tree = Arena::<String>::new();
    let tree_bytes = tree_str.as_bytes();

    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(TPE::IncorrectFormat("Minimal of 2 brackets not found!".to_owned()));
    }

    let mut tokens = token_positions.iter().peekable();
    let root_start = *tokens.next().unwrap();
    let root_end = **tokens.peek().unwrap();

    let root_label = String::from_utf8(tree_bytes[(root_start + 1)..root_end].to_vec());
    let root = tree.new_node(root_label.unwrap());
    let mut node_stack = vec![root];

    while let Some(token) = tokens.next() {
        match tree_bytes[*token] {
            TOKEN_START => {
                let Some(token_end) = tokens.peek() else {
                    let err_msg = format!("Label has no ending token near col {token} , line \"{tree_str}\"");
                    return Err(TPE::IncorrectFormat(err_msg));
                };
                let label = String::from_utf8(
                    tree_bytes[(*token + 1)..**token_end].to_vec()
                );
                let n = tree.new_node(label.unwrap());
                let Some(last_node) = node_stack.last() else {
                    let err_msg = format!("Reached unexpected end of token on line \"{tree_str}\"");
                    return Err(TPE::IncorrectFormat(err_msg));
                };
                last_node.append(n, &mut tree);
                node_stack.push(n);
            },
            TOKEN_END => {
                let Some(_) = node_stack.pop() else {
                    return Err(TPE::IncorrectFormat("Wrong bracket pairing".to_owned()));
                };
            },
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
        let arena = parse_tree(Ok(input));
        assert!(arena.is_ok());
        let arena = arena.unwrap();
        assert_eq!(arena.count(), 3);
        let mut iter = arena.iter();
        assert_eq!(iter.next().map(|node| node.get().clone()), Some("einsteinstrasse".to_owned()));
        assert_eq!(iter.next().map(|node| node.get().clone()), Some("1".to_owned()));
        assert_eq!(iter.next().map(|node| node.get().clone()), Some("3".to_owned()));
    }


    #[test]
    fn test_parses_escaped() {
        let input = String::from(r#"{article{key{journals/corr/abs-0812-2567}}{mdate{2017-06-07}}{publtype{informal}}{author{Jian Li}}{title{An O(log n / log log n\\}\\}) Upper Bound on the Price of Stability for Undirected Shapley Network Design Games}}{ee{http://arxiv.org/abs/0812.2567}}{year{2008}}{journal{CoRR}}{volume{abs/0812.2567}}{url{db/journals/corr/corr0812.html#abs-0812-2567}}}"#);
        let arena = parse_tree(Ok(input));
        assert!(arena.is_ok());
        assert_eq!(arena.unwrap().count(), 21);
    }

    #[test]
    fn test_descendants_correct() {
        let input = "{first{second{third}{fourth{fifth{six}{seven}}}}".to_owned();
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
        assert_eq!(arena.get(rd.unwrap()).map(|node| node.get()), Some("first".to_string()).as_ref());
        assert_eq!(arena.get(iter.next().unwrap()).map(|node| node.get()), Some("second".to_string()).as_ref());
        assert_eq!(arena.get(iter.next().unwrap()).map(|node| node.get()), Some("third".to_string()).as_ref());
        assert_eq!(arena.get(iter.next().unwrap()).map(|node| node.get()), Some("fourth".to_string()).as_ref());
        assert_eq!(arena.get(iter.next().unwrap()).map(|node| node.get()), Some("fifth".to_string()).as_ref());
        assert_eq!(arena.get(iter.next().unwrap()).map(|node| node.get()), Some("six".to_string()).as_ref());
        assert_eq!(arena.get(iter.next().unwrap()).map(|node| node.get()), Some("seven".to_string()).as_ref());
    }
}
