use crate::indexing::{Indexer, InvertedListLabelPostorderIndex, SEDIndex};
use crate::lb::indexes::histograms::create_collection_histograms;
use crate::parsing::{tree_to_string, LabelDict, LabelId, TreeOutput};
use crate::statistics::TreeStatistics;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use itertools::Itertools;
use lb::binary_branch::BinaryBranchConverter;
use lb::label_intersection::label_intersection_k;
use lb::sed::sed_k;
use lb::structural_filter::{self, ted as struct_ted_k, LabelSetConverter};
use rayon::prelude::*;
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{self, exit};

mod indexing;
mod lb;
mod parsing;
mod statistics;
mod validation;

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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum LowerBoundMethods {
    /// Histogram lower bound
    Hist,
    /// Label intersection lower bound
    Lblint,
    /// String edit distance lower bound
    Sed,
    /// Structural filter lower bound
    Structural,
    /// Structural variant filter lower bound
    StructuralVariant,
    /// Binary branch lower bound
    Bib,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// outputs data for degree, leaf paths and labels histograms
    Statistics {
        /// outputs data for degree, leaf paths and labels histograms
        #[arg(long)]
        hists: Option<PathBuf>,
    },
    /// Gets pre- and post- order traversals of each tree
    Traversals {
        /// output path for traversals
        #[arg(long)]
        output: PathBuf,
    },
    /// Calculates lower bound candidates
    LowerBound {
        /// Query file input, on each file <Threshold>,<Query tree>
        #[arg(long, short = 'q')]
        query_file: PathBuf,
        /// output path for lower bound candidates
        #[arg(long, short = 'o')]
        output: PathBuf,
        /// Run only given lower bound method
        #[arg(value_enum)]
        method: Option<LowerBoundMethods>,
        /// Optional real results path - will output precision and filter_times
        #[arg(long)]
        results_path: Option<PathBuf>,
    },
    /// Validates candidate results against real results
    Validate {
        /// Candidates path
        #[arg(long)]
        candidates_path: PathBuf,
        /// Real results path
        #[arg(long)]
        results_path: PathBuf,
        /// Threshold for validation
        #[arg()]
        threshold: usize,
    },
    /// Compares 2 candidate files TED execution time
    TedTime {
        /// First candidates path
        #[arg(long = "cf")]
        candidates_first: PathBuf,
        /// Second candidates path
        #[arg(long = "cs")]
        candidates_second: PathBuf,
        /// Threshold for validation
        #[arg()]
        threshold: usize,
    },
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let mut cmd = Cli::command();

    if !cli.dataset_path.exists() || !cli.dataset_path.is_file() {
        cmd.error(
            ErrorKind::InvalidValue,
            "Path does not exists or is not a valid file!",
        )
        .exit();
    }
    let mut label_dict = LabelDict::new();
    let trees = match parsing::parse_dataset(&cli.dataset_path, &mut label_dict) {
        Ok(trees) => trees,
        Err(e) => {
            eprintln!("Got unexpected error: {}", e);
            exit(1);
        }
    };
    if !cli.quiet {
        println!("Parsed {} trees", trees.len());
    }

    match cli.command {
        Commands::Statistics { hists } => {
            let stats: Vec<_> = trees.par_iter().map(statistics::gather).collect();
            let summary = statistics::summarize(&stats);
            println!("Collection statistics\nmin_tree,max_tree,avg_tree,tree_count,distinct_labels\n{summary},{}", label_dict.keys().len());
            if hists.is_some() {
                let mut output_path = hists.unwrap();
                if output_path.exists() && !output_path.is_dir() {
                    cmd.error(
                        ErrorKind::InvalidValue,
                        "Output path must be a directory! Defaulting to current...",
                    )
                    .print()?;
                    output_path = PathBuf::from("./");
                }

                if !output_path.exists() {
                    create_dir_all(&output_path)?;
                }

                write_files(&stats, &output_path)?;
            }
        }
        Commands::Traversals { output } => {
            let traversal_strings = trees
                .par_iter()
                .map(|tree| SEDIndex::index_tree(tree, &label_dict))
                .map(|index| {
                    format!(
                        "{pre}\n{post}",
                        pre = index
                            .preorder
                            .iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(";"),
                        post = index
                            .postorder
                            .iter()
                            .map(|x| x.to_string())
                            .collect::<Vec<_>>()
                            .join(";")
                    )
                })
                .collect::<Vec<_>>();

            write_file(output, &traversal_strings)?;
        }
        Commands::LowerBound {
            query_file,
            output,
            method,
            results_path,
        } => {
            use LowerBoundMethods as LBM;
            if !output.is_dir() {
                eprintln!("Output arg must be a directory, is: {output:#?}");
                process::exit(1);
            }

            // let mut times = vec![];
            // let mut candidate_times = vec![];
            // let mut selectivities = vec![];
            println!("Preparing dataset and running preprocessing for all methods");
            let collection_histograms = create_collection_histograms(&trees);
            let lblint_indexes = trees
                .par_iter()
                .map(|t| InvertedListLabelPostorderIndex::index_tree(t, &label_dict))
                .collect::<Vec<_>>();
            let sed_indexes = trees
                .par_iter()
                .map(|t| SEDIndex::index_tree(t, &label_dict))
                .collect::<Vec<_>>();
            let mut bib_converter = BinaryBranchConverter::default();
            let tree_bib_vectors = bib_converter.create(&trees);
            let mut lc = LabelSetConverter::default();
            let structural_sets = lc.create(&trees);
            let split_distribution_map = structural_filter::best_split_distribution(&label_dict);
            let split_distribution =
                move |lbl: &LabelId| -> usize { *split_distribution_map.get(lbl).unwrap() };
            let structural_split_sets = lc.create_split(&trees, split_distribution);
            let queries = parsing::parse_queries(&query_file, &mut label_dict).unwrap();

            let lbms: [LBM; 3] = [LBM::Lblint, LBM::Sed, LBM::Structural];

            for current_method in lbms.iter() {
                // TODO: preprocess the queries
                let (mut candidates, duration) = match *current_method {
                    LBM::Lblint => {
                        let lblint_queries = queries
                            .iter()
                            .map(|(t, q)| {
                                (
                                    *t,
                                    InvertedListLabelPostorderIndex::index_tree(q, &label_dict),
                                )
                            })
                            .collect_vec();

                        lb::iterate_queries!(lblint_queries, lblint_indexes, label_intersection_k)
                    }
                    LBM::Sed => {
                        let sed_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, SEDIndex::index_tree(q, &label_dict)))
                            .collect_vec();

                        lb::iterate_queries!(sed_queries, sed_indexes, sed_k)
                    }
                    LBM::Structural => {
                        let structural_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, lc.create_single(q)))
                            .collect_vec();

                        lb::iterate_queries!(structural_queries, structural_sets, struct_ted_k)
                    }
                    _ => todo!(),
                };

                println!(
                    "Execution time for {current_method:?} was: {duration_ms}ms",
                    duration_ms = duration.as_millis()
                );
                let mut output_file = output.clone();
                output_file.push(format!("{current_method:#?}_candidates.csv"));

                candidates.par_sort();
                write_file(
                    output_file,
                    &candidates
                        .iter()
                        .map(|(c1, c2)| format!("{c1},{c2}"))
                        .collect_vec(),
                )?;
            }

            // candidates.par_sort();
            // write_file(
            //     output.clone(),
            //     &candidates
            //         .iter()
            //         .map(|(c1, c2)| format!("{c1},{c2}"))
            //         .collect_vec(),
            // )?;
            // let mean_selectivity = statistics::mean(&selectivities);
            // println!("Mean selectivity is: {mean_selectivity:.4}%");
            // println!(
            //     "Total LB execution time: {}ms",
            //     lb_start.elapsed().as_millis()
            // );
            // let ds_name: Vec<&str> = cli
            //     .dataset_path
            //     .file_name()
            //     .unwrap()
            //     .to_str()
            //     .unwrap()
            //     .split('_')
            //     .collect();
            // let ds_name = ds_name[0];
            // // let Some((ds_name, _)) = ds_name.split_once('.') else { todo!(); };
            // write_file(
            //     output
            //         .parent()
            //         .unwrap()
            //         .join(format!("{ds_name}-{method:?}-times-us.txt")),
            //     &times.iter().map(|t| format!("{t}")).collect_vec(),
            // )?;
            // write_file(
            //     output
            //         .parent()
            //         .unwrap()
            //         .join(format!("{ds_name}-{method:?}-candidate-times-ns.txt")),
            //     &candidate_times.iter().map(|t| format!("{t}")).collect_vec(),
            // )?;
        }
        Commands::Validate {
            results_path,
            threshold,
            candidates_path,
        } => {
            let false_positives = validation::validate(&candidates_path, &results_path, threshold)?;
            let candidates = validation::read_candidates(&candidates_path)?;
            let (correct, extra, precision, mean_selectivity) =
                validation::get_precision(&candidates, &results_path, threshold, trees.len())?;

            println!("Correct trees;Extra trees;Precision;Mean Selectivity");
            println!("{correct};{extra};{precision};{mean_selectivity:.7}%");
            println!("Printing false positives in bracket");
            write_file(
                PathBuf::from("./resources/results/false-positives.bracket"),
                &false_positives
                    .iter()
                    .map(|(c1, c2)| {
                        format!(
                            "\"{}\",\"{}\"",
                            tree_to_string(&trees[*c1], TreeOutput::BracketNotation),
                            tree_to_string(&trees[*c2], TreeOutput::BracketNotation)
                        )
                    })
                    .collect_vec(),
            )?;
            println!("Printing not found in graphviz");
            write_file(
                PathBuf::from("./resources/results/false-positives.graphviz"),
                &false_positives
                    .iter()
                    .map(|(c1, c2)| {
                        format!(
                            "{}{}\n-------------------------\n",
                            tree_to_string(&trees[*c1], TreeOutput::Graphviz),
                            tree_to_string(&trees[*c2], TreeOutput::Graphviz)
                        )
                    })
                    .collect_vec(),
            )?;
        }
        Commands::TedTime {
            candidates_first: _,
            candidates_second: _,
            threshold: _,
        } => {
            todo!();
        }
    }

    Ok(())
}

fn write_precision_and_filter_times(
    base: &Path,
    times: &[u128],
    precision: (usize, usize, f32, u128),
    hist_method: &str,
    k: usize,
    candidates: Option<&[(usize, usize)]>,
) -> Result<(), anyhow::Error> {
    let mut times_output = PathBuf::from(base);
    let mut precision_output = PathBuf::from(base);
    let mut candidates_output = PathBuf::from(base);
    times_output.push(format!("hist_{hist_method}_us.txt"));
    precision_output.push(format!("precision-hist-{hist_method}-{k}.txt"));
    let times_file = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(times_output)?;
    let mut precision_file = File::options()
        .create(true)
        .truncate(true)
        .write(true)
        .open(precision_output)?;
    let mut times_writer = BufWriter::new(times_file);

    for t in times.iter() {
        writeln!(times_writer, "{t}")?;
    }

    writeln!(
        precision_file,
        "Correct trees;Incorrect trees;Precision;Total Time"
    )?;
    writeln!(
        precision_file,
        "{};{};{};{}",
        precision.0, precision.1, precision.2, precision.3
    )?;

    if let Some(candidates) = candidates {
        candidates_output.push(format!("candidates-hist-{hist_method}-{k}.csv"));
        write_file(
            candidates_output,
            &candidates
                .iter()
                .map(|(c1, c2)| format!("{c1},{c2}"))
                .collect_vec(),
        )?;
    }

    Ok(())
}

fn write_files(
    stats: &[TreeStatistics],
    output_dir: &impl AsRef<Path>,
) -> Result<(), anyhow::Error> {
    let out = output_dir.as_ref().to_path_buf();
    write_file(
        [&out, &PathBuf::from("degrees.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats
            .iter()
            .flat_map(|s| &s.degrees)
            .collect::<Vec<&usize>>(),
    )?;
    write_file(
        [&out, &PathBuf::from("depths.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats
            .iter()
            .flat_map(|s| &s.depths)
            .collect::<Vec<&usize>>(),
    )?;
    // write_file(
    //     [&out, &PathBuf::from("labels.csv")].iter().collect::<PathBuf>(),
    //     &stats.iter().flat_map(|s| {
    //         s.distinct_labels.iter().map(|(k, v)| format!("{k},{v}")).collect::<Vec<_>>()
    //     }).collect::<Vec<_>>()
    // )?;

    Ok(())
}

fn write_file<T>(file_name: impl AsRef<Path>, data: &[T]) -> Result<(), std::io::Error>
where
    T: Display,
{
    let f = File::options()
        .create(true)
        .write(true)
        .truncate(true)
        .open(file_name.as_ref())?;
    let mut w = BufWriter::new(f);

    for d in data.iter() {
        writeln!(w, "{d}")?;
    }
    Ok(())
}
