use clap::Parser;
use std::path::PathBuf;
use std::process::exit;

mod parsing;

/// Tree statistics utility
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Dataset file of trees in bracket notation
    #[arg(short, long, value_name="FILE")]
    dataset_path: PathBuf
}

fn main() -> Result<(), clap::Error> {
    let cli = Cli::parse();

    if !cli.dataset_path.exists() || !cli.dataset_path.is_file() {
        eprintln!("This file does not exists or is not a valid file!");
        exit(1);
    }

    let trees = match parsing::parse_dataset(cli.dataset_path) {
        Ok(trees) => trees,
        Err(e) => {
            eprintln!("Got unexpected error: {}", e);
            exit(1);
        },
    };
    println!("Parsed {} trees", trees.len());
    Ok(())
}
