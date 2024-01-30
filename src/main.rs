use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use clap::{CommandFactory, Parser};
use std::path::{Path, PathBuf};
use std::process::exit;
use clap::error::ErrorKind;
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
    #[arg(long)]
    hists: Option<PathBuf>,
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let mut cmd = Cli::command();

    if !cli.dataset_path.exists() || !cli.dataset_path.is_file() {
        cmd.error(
            ErrorKind::InvalidValue,
            "Path does not exists or is not a valid file!"
        ).exit();
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

    if cli.hists.is_some() {
        let mut output_path = cli.hists.unwrap();
        if output_path.exists() && !output_path.is_dir() {
            cmd.error(ErrorKind::InvalidValue, "Output path must be a directory! Defaulting to current...").print()?;
            output_path = PathBuf::from("./");
        }

        if !output_path.exists() {
            create_dir_all(&output_path)?;
        }

        write_files(&stats, &output_path)?;
    }
    Ok(())
}

fn write_files(stats: &[TreeStatistics], output_dir: &impl AsRef<Path>) -> Result<(), anyhow::Error> {
    let out = output_dir.as_ref().to_path_buf();
    write_file(
        [&out, &PathBuf::from("degrees.csv")].iter().collect::<PathBuf>(),
        &stats.iter().flat_map(|s| &s.degrees).collect::<Vec<&usize>>()
    )?;
    write_file(
        [&out, &PathBuf::from("depths.csv")].iter().collect::<PathBuf>(),
        &stats.iter().flat_map(|s| &s.depths).collect::<Vec<&usize>>()
    )?;
    write_file(
        [&out, &PathBuf::from("labels.csv")].iter().collect::<PathBuf>(),
        &stats.iter().flat_map(|s| {
            s.distinct_labels.iter().map(|(k, v)| format!("{k},{v}")).collect::<Vec<_>>()
        }).collect::<Vec<_>>()
    )?;

    Ok(())
}

fn write_file<T>(file_name: impl AsRef<Path>, data: &[T]) -> Result<(), std::io::Error>
    where T: Display{
    let f = File::create(file_name.as_ref().to_path_buf())?;
    let mut w = BufWriter::new(f);

    for d in data.iter() {
        writeln!(w, "{d}")?;
    }
    Ok(())
}