use crate::indexing::{Indexer, InvertedListLabelPostorderIndex, SEDIndex};
use crate::lb::indexes::histograms::{create_collection_histograms, index_lookup};
use crate::parsing::{tree_to_string, LabelDict, LabelId, TreeOutput};
use crate::statistics::TreeStatistics;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use itertools::Itertools;
use lb::structural_filter::LabelSetConverter;
use rayon::prelude::*;
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::exit;
use std::time::Instant;

use rand::prelude::*;
use rustc_hash::FxHashMap;

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
        /// output path for lower bound candidates
        #[arg(long)]
        output: PathBuf,
        /// Lower bound method
        #[arg(value_enum)]
        method: LowerBoundMethods,
        /// Optional threshold for bounded calculation - they are faster!
        #[arg()]
        threshold: Option<usize>,
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
            println!("Collection statistics\n{summary}");
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
            method,
            output,
            threshold,
            results_path,
        } => {
            use LowerBoundMethods as LBM;
            let mut candidates: Vec<(usize, usize)> = vec![];
            let lb_start = Instant::now();
            println!(
                "Running for dataset: {} and method: {:?}",
                cli.dataset_path.to_str().unwrap(),
                method
            );
            // TODO: Fix this unwrap_or
            let mut times = vec![];
            let mut candidate_times = vec![];
            let mut selectivities = vec![];
            let k = threshold.unwrap_or(0);
            match method {
                LBM::Hist => {
                    let (leaf_hist, degree_hist, label_hist) = create_collection_histograms(&trees);
                    let start = Instant::now();
                    let (times, c) =
                        index_lookup(&leaf_hist, &degree_hist, &label_hist, &label_dict, k);
                    candidates = c;
                    let duration = start.elapsed();

                    if let Some(results_path) = results_path {
                        let (all_correct, all_extra, all_precision, _) =
                            validation::get_precision(&candidates, &results_path, k).unwrap();
                        let output_dir = output.parent().expect("Output dir not found!");
                        write_precision_and_filter_times(
                            output_dir,
                            &times,
                            (all_correct, all_extra, all_precision, duration.as_millis()),
                            "all",
                            k,
                            None,
                        )?;
                    }
                }
                LBM::Lblint => {
                    let indexed_trees = trees
                        .iter()
                        .map(|t| {
                            InvertedListLabelPostorderIndex::index_tree(t, &label_dict)
                        })
                        .collect_vec();

                    candidates = indexed_trees
                        .iter()
                        .enumerate()
                        .flat_map(|(i, t1)| {
                            let mut lc = vec![];
                            let lb_start = Instant::now();
                            for (j, t2) in indexed_trees.iter().enumerate().skip(i + 1) {
                                let lb = lb::label_intersection::label_intersection_k(t1, t2, k);
                                if lb <= k {
                                    lc.push((i, j));
                                    candidate_times.push(lb_start.elapsed().as_nanos());
                                }
                            }
                            times.push(lb_start.elapsed().as_micros());
                            let sel = 100f64
                                * (lc.len() as f64
                                / (indexed_trees.len() - i) as f64);
                            selectivities.push(sel);
                            lc
                        })
                        .collect::<Vec<_>>();
                }
                LBM::Sed => {
                    let indexed_trees = trees
                        .iter()
                        .map(|t| SEDIndex::index_tree(t, &label_dict))
                        .collect_vec();

                    candidates = indexed_trees
                        .iter()
                        .enumerate()
                        .flat_map(|(i, t1)| {
                            let mut lc = vec![];
                            let lb_start = Instant::now();
                            for (j, t2) in indexed_trees.iter().enumerate().skip(i + 1) {
                                let lb = lb::sed::sed_k(t1, t2, k + 1);
                                if lb <= k {
                                    lc.push((i, j));
                                    candidate_times.push(lb_start.elapsed().as_nanos());
                                }
                            }
                            times.push(lb_start.elapsed().as_micros());
                            let sel = 100f64
                                * (lc.len() as f64
                                / (indexed_trees.len() - i) as f64);
                            selectivities.push(sel);
                            lc
                        })
                        .collect::<Vec<_>>();
                }
                LBM::Structural | LBM::StructuralVariant => {
                    let start = Instant::now();
                    let mut lc = LabelSetConverter::default();
                    let mut label_tree_size = FxHashMap::default();
                    trees.iter().for_each(|tree| {
                        tree.iter().for_each(|node| {
                            let label = node.get();
                            label_tree_size.entry(label).and_modify(|tc| *tc += tree.count())
                                .or_insert(tree.count());
                        })
                    });

                    label_dict.values().for_each(|(lbl, _)| {label_tree_size.entry(lbl).or_insert(0); });

                    let sorted_labels = label_dict
                        .values()
                        // .sorted_by_key(|(_, c)| c)
                        .sorted_by(|(lbl, c), (lbl2, c2)| (c2 * label_tree_size.get(lbl2).unwrap()).cmp(&(c * label_tree_size.get(lbl).unwrap())) )
                        .collect_vec();

                    // write_file("sorted_labels.txt", &sorted_labels.iter().map(|(lbl, lblcnt)| format!("{lbl},{lblcnt}")).collect_vec())?;

                    let mut label_distribution = FxHashMap::default();
                    let mut i = 0;
                    sorted_labels
                        .iter()
                        .rev()
                        .for_each(|(lbl, lblcnt)| {
                            label_distribution.insert(lbl, i % LabelSetConverter::MAX_SPLIT);
                            i += 1;
                        });


                    let split_labels_into_axes =
                        move |lbl: &LabelId| -> usize { *label_distribution.get(lbl).unwrap() };


                    if let LBM::StructuralVariant = method {
                        let structural_sets: Vec<
                            lb::structural_filter::SplitStructuralFilterTuple,
                        > = lc.create_split(&trees, split_labels_into_axes);
                        println!("Creating sets took {}ms", start.elapsed().as_millis());
                        candidates = structural_sets
                            .iter()
                            .enumerate()
                            .flat_map(|(i, t1)| {
                                // println!("{i}");
                                let lb_start = Instant::now();
                                let mut lower_bound_candidates = vec![];
                                for (j, t2) in structural_sets.iter().enumerate().skip(i + 1) {
                                    let lb = lb::structural_filter::ted_variant(t1, t2, k);
                                    if lb <= k {
                                        lower_bound_candidates.push((i, j));
                                        candidate_times.push(lb_start.elapsed().as_nanos());
                                    }
                                }
                                times.push(lb_start.elapsed().as_micros());
                                let sel = 100f64
                                    * (lower_bound_candidates.len() as f64
                                        / (trees.len() - i) as f64);
                                selectivities.push(sel);
                                lower_bound_candidates
                            })
                            .collect::<Vec<_>>();
                    } else {
                        let structural_sets = lc.create(&trees);
                        println!("Creating sets took {}ms", start.elapsed().as_millis());
                        candidates = structural_sets
                            .iter()
                            .enumerate()
                            .flat_map(|(i, t1)| {
                                // println!("{i}");
                                let lb_start = Instant::now();
                                let mut lower_bound_candidates = vec![];
                                for (j, t2) in structural_sets.iter().enumerate().skip(i + 1) {
                                    let lb = lb::structural_filter::ted(t1, t2, k);
                                    if lb <= k {
                                        lower_bound_candidates.push((i, j));
                                        candidate_times.push(lb_start.elapsed().as_nanos());
                                    }
                                }
                                times.push(lb_start.elapsed().as_micros());
                                let sel = 100f64
                                    * (lower_bound_candidates.len() as f64
                                        / (trees.len() - i) as f64);
                                selectivities.push(sel);
                                lower_bound_candidates
                            })
                            .collect::<Vec<_>>();
                    }
                }
            }
            candidates.par_sort();
            write_file(
                output.clone(),
                &candidates
                    .iter()
                    .map(|(c1, c2)| format!("{c1},{c2}"))
                    .collect_vec(),
            )?;
            let mean_selectivity = statistics::mean(&selectivities);
            println!("Mean selectivity is: {mean_selectivity:.4}%");
            println!("Total LB execution time: {}ms", lb_start.elapsed().as_millis());
            let ds_name: Vec<&str> = cli.dataset_path.file_name().unwrap().to_str().unwrap().split('_').collect();
            let ds_name = ds_name[0];
            // let Some((ds_name, _)) = ds_name.split_once('.') else { todo!(); };
            write_file(
                output.parent().unwrap().join(format!("{ds_name}-{method:?}-times-us.txt")),
                &times.iter().map(|t| format!("{t}") ).collect_vec()
            )?;
            write_file(
                output.parent().unwrap().join(format!("{ds_name}-{method:?}-candidate-times-ns.txt")),
                &candidate_times.iter().map(|t| format!("{t}") ).collect_vec()
            )?;
        }
        Commands::Validate {
            results_path,
            threshold,
            candidates_path,
        } => {
            let false_positives = validation::validate(&candidates_path, &results_path, threshold)?;
            let candidates = validation::read_candidates(&candidates_path)?;
            let (correct, extra, precision, mean_selectivity) =
                validation::get_precision(&candidates, &results_path, threshold)?;

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
