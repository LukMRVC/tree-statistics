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
    use TreeParseError::*;

    let tree_str = tree_str?;
    if !tree_str.is_ascii() {
        return Err(IsNotAscii);
    }
    let mut tree = Arena::<String>::new();
    let tree_bytes = tree_str.as_bytes();

    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(IncorrectFormat("Minimal of 2 brackets not found!".to_owned()));
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
                    return Err(IncorrectFormat(err_msg));
                };
                let label = String::from_utf8(
                    tree_bytes[(*token + 1)..**token_end].to_vec()
                );
                let n = tree.new_node(label.unwrap());
                node_stack.last()
                    .unwrap()
                    .append(n, &mut tree);
                node_stack.push(n);
            },
            TOKEN_END => {
                let Some(_) = node_stack.pop() else {
                    return Err(IncorrectFormat("Wrong bracket pairing".to_owned()));
                };
            },
            _ => return Err(TokenizerError),
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
        assert_eq!(arena.is_ok(), true);
        let arena = arena.unwrap();
        assert_eq!(arena.count(), 3);
        let mut iter = arena.iter();
        assert_eq!(iter.next().map(|node| node.get().clone()), Some("einsteinstrasse".to_owned()));
        assert_eq!(iter.next().map(|node| node.get().clone()), Some("1".to_owned()));
        assert_eq!(iter.next().map(|node| node.get().clone()), Some("3".to_owned()));
    }
}
