use std::fmt::Display;
use std::fs::File;
use std::io::{BufWriter, Write};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::exit;
use rayon::prelude::*;
use crate::statistics::TreeStatistics;

mod parsing;
mod statistics;

/// Tree statistics utility
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Cli {
    /// Dataset file of trees in bracket notation
    #[arg(short, long, value_name = "FILE")]
    dataset_path: PathBuf,
    /// outputs only collected statistics
    #[arg(short, default_value_t = false)]
    quiet: bool,
    /// outputs data for degree, leaf paths and labels histograms
    #[arg(long, default_value_t = false)]
    hists: bool,
}

fn main() -> Result<(), anyhow::Error> {
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
        }
    };
    if !cli.quiet {
        println!("Parsed {} trees", trees.len());
        println!("Gathering statistics");
    }

    let stats: Vec<_> = trees.par_iter().map(statistics::gather).collect();
    let summary = statistics::summarize(&stats);

    println!("Collection statistics\n{summary}");

    if cli.hists {
        write_files(&stats)?;
    }
    Ok(())
}

fn write_files(stats: &[TreeStatistics]) -> Result<(), anyhow::Error> {
    write_file(
        PathBuf::from("degrees.csv"),
        &stats.iter().flat_map(|s| &s.degrees).collect::<Vec<&usize>>()
    )?;
    write_file(
        PathBuf::from("depths.csv"),
        &stats.iter().flat_map(|s| &s.depths).collect::<Vec<&usize>>()
    )?;
    write_file(
        PathBuf::from("labels.csv"),
        &stats.iter().flat_map(|s| {
            s.distinct_labels.iter().map(|(k, v)| format!("{k},{v}")).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    )?;

    Ok(())
}

fn write_file<T, P>(file_name: P, data: &[T]) -> Result<(), std::io::Error>
    where T: Display,
    P: AsRef<Path> {
    let f = File::create(file_name.as_ref().to_path_buf())?;
    let mut w = BufWriter::new(f);

    for d in data.iter() {
        writeln!(w, "{d}")?;
    }
    Ok(())
}