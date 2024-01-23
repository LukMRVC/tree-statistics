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
const TOKEN_END: u8 = b'{';
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
    #[error("tree string has incorrect bracket notation format")]
    IncorrectFormat
}


fn parse_tree(tree_str: Result<String, io::Error>) -> Result<Arena<String>, TreeParseError> {
    use TreeParseError::*;

    let tree_str = tree_str?;
    if !tree_str.is_ascii() {
        return Err(IsNotAscii);
    }
    let tree = Arena::<String>::new();
    let tree_bytes = tree_str.as_bytes();

    let token_positions: Vec<usize> = memchr2_iter(TOKEN_START, TOKEN_END, tree_bytes)
        .filter(|char_pos| !is_escaped(tree_bytes, *char_pos))
        .collect();

    if token_positions.len() < 2 {
        return Err(IncorrectFormat);
    }

    let mut tokens = token_positions.iter().peekable();
    let root_start = *tokens.next().unwrap();
    let root_end = **tokens.peek().unwrap();

    let root_label = String::from(&tree_bytes[(root_start + 1)..root_end]);
    // TODO: create root node

    Ok(tree)
}

