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
            println!(
                "Running for dataset: {} and method: {:?}",
                cli.dataset_path.to_str().unwrap(),
                method
            );
            // TODO: Fix this unwrap_or
            let k = threshold.unwrap_or(0);
            match method {
                LBM::Hist => {
                    let (leaf_hist, degree_hist, label_hist) = create_collection_histograms(&trees);
                    let start = Instant::now();
                    let (times, c) =
                        index_lookup(&leaf_hist, &degree_hist, &label_hist, &label_dict, k);
                    candidates = c;
                    let duration = start.elapsed();
                    println!("Histogram LB lookup took: {}ms", duration.as_millis());

                    if let Some(results_path) = results_path {
                        let (all_correct, all_extra, all_precision) =
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
                        //
                        // let degree_time = Instant::now();
                        // let (degree_filter_times, mut degree_candidates) =
                        //     degree_index_lookup(&degree_hist, &label_dict, k);
                        // degree_candidates.sort();
                        // let degree_time = degree_time.elapsed().as_millis();
                        // let (correct, extra, precision) =
                        //     validation::get_precision(&degree_candidates, &results_path, k)
                        //         .unwrap();
                        // write_precision_and_filter_times(
                        //     output_dir,
                        //     &degree_filter_times,
                        //     (correct, extra, precision, degree_time),
                        //     "degree",
                        //     k,
                        //     Some(&degree_candidates),
                        // )?;
                        //
                        // let leaf_time = Instant::now();
                        // let (leaf_filter_times, mut leaf_candidates) =
                        //     leaf_index_lookup(&leaf_hist, &label_dict, k);
                        // leaf_candidates.sort();
                        // let leaf_time = leaf_time.elapsed().as_millis();
                        // let (correct, extra, precision) =
                        //     validation::get_precision(&leaf_candidates, &results_path, k).unwrap();
                        // write_precision_and_filter_times(
                        //     output_dir,
                        //     &leaf_filter_times,
                        //     (correct, extra, precision, leaf_time),
                        //     "leaf",
                        //     k,
                        //     Some(&leaf_candidates),
                        // )?;
                        //
                        // let label_time = Instant::now();
                        // let (label_filter_times, mut label_candidates) =
                        //     label_index_lookup(&label_hist, &label_dict, k);
                        // label_candidates.sort();
                        // let label_time = label_time.elapsed().as_millis();
                        // let (correct, extra, precision) =
                        //     validation::get_precision(&label_candidates, &results_path, k).unwrap();
                        // write_precision_and_filter_times(
                        //     output_dir,
                        //     &label_filter_times,
                        //     (correct, extra, precision, label_time),
                        //     "label",
                        //     k,
                        //     Some(&label_candidates),
                        // )?;
                    }
                }
                LBM::Lblint => {
                    let indexed_trees = trees
                        .iter()
                        .enumerate()
                        .map(|(_idx, t)| {
                            InvertedListLabelPostorderIndex::index_tree(t, &label_dict)
                        })
                        .collect_vec();

                    candidates = indexed_trees
                        .iter()
                        .enumerate()
                        .flat_map(|(i, t1)| {
                            let mut lc = vec![];
                            for (j, t2) in indexed_trees.iter().enumerate().skip(i + 1) {
                                let lb = lb::label_intersection::label_intersection(t1, t2);
                                if lb <= k {
                                    lc.push((i, j));
                                }
                            }
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
                            for (j, t2) in indexed_trees.iter().enumerate().skip(i + 1) {
                                let lb = lb::sed::sed_k(t1, t2, k + 1);
                                if lb <= k {
                                    lc.push((i, j));
                                }
                            }
                            lc
                        })
                        .collect::<Vec<_>>();
                }
                LBM::Structural | LBM::StructuralVariant => {
                    let start = Instant::now();
                    let mut lc = LabelSetConverter::default();
                    // let half = dbg!(half);
                    let sorted_labels = label_dict
                        .values()
                        .sorted_unstable_by_key(|(_, c)| c)
                        .collect_vec();
                    let most_used_labels = sorted_labels
                        .iter()
                        .rev()
                        .map(|(lbl, _)| *lbl)
                        .take(8)
                        .collect_vec();
                    use rand::{Rng, SeedableRng};

                    let mut label_distribution = FxHashMap::default();
                    let mut rng1 = rand_xoshiro::Xoshiro256PlusPlus::from_entropy();
                    label_dict.values().for_each(|(lbl, _)| {
                        label_distribution
                            .insert(*lbl, rng1.gen_range(0..LabelSetConverter::MAX_SPLIT));
                    });

                    let split_labels_into_axes =
                        move |lbl: &LabelId| -> usize { *label_distribution.get(lbl).unwrap() };

                    // let mut i = 0;
                    // most_used_labels.iter().for_each(|lbl| {
                    //     label_distribution.insert(lbl, i % 4);
                    //     i += 1;
                    // });
                    //
                    // sorted_labels
                    //     .iter()
                    //     .rev()
                    //     .skip(most_used_labels.len())
                    //     .for_each(|(lbl, _)| {
                    //         label_distribution.insert(lbl, i % LabelSetConverter::MAX_SPLIT);
                    //         i += 1;
                    //     });

                    let mut selectivities = vec![];
                    if let LBM::StructuralVariant = method {
                        let structural_sets: Vec<
                            lb::structural_filter::SplitStructuralFilterTuple,
                        > = lc.create_split(&trees, split_labels_into_axes);
                        println!("Creating sets took {}ms", start.elapsed().as_millis());
                        let start = Instant::now();
                        candidates = structural_sets
                            .iter()
                            .enumerate()
                            .flat_map(|(i, t1)| {
                                let mut lower_bound_candidates = vec![];

                                for (j, t2) in structural_sets.iter().enumerate().skip(i + 1) {
                                    let lb = lb::structural_filter::ted_variant(t1, t2, k);
                                    if lb <= k {
                                        lower_bound_candidates.push((i, j));
                                    }
                                }
                                let sel = 100f64
                                    * (lower_bound_candidates.len() as f64
                                        / (trees.len() - i) as f64);
                                selectivities.push(sel);
                                lower_bound_candidates
                            })
                            .collect::<Vec<_>>();
                        println!(
                            "SF-Adjusted Filter elapsed time: {}ms",
                            start.elapsed().as_millis()
                        );
                    } else {
                        let structural_sets = lc.create(&trees);
                        println!("Creating sets took {}ms", start.elapsed().as_millis());
                        let start = Instant::now();
                        candidates = structural_sets
                            .iter()
                            .enumerate()
                            .flat_map(|(i, t1)| {
                                let mut lower_bound_candidates = vec![];
                                for (j, t2) in structural_sets.iter().enumerate().skip(i + 1) {
                                    let lb = lb::structural_filter::ted(t1, t2, k);
                                    if lb <= k {
                                        lower_bound_candidates.push((i, j));
                                    }
                                }
                                let sel = 100f64
                                    * (lower_bound_candidates.len() as f64
                                        / (trees.len() - i) as f64);
                                selectivities.push(sel);
                                lower_bound_candidates
                            })
                            .collect::<Vec<_>>();
                        println!("SF Filter elapsed time: {}ms", start.elapsed().as_millis());
                    }
                    let mean_selectivity = statistics::mean(&selectivities);
                    println!("Mean selectivity is: {mean_selectivity:.4}");
                }
            }
            candidates.par_sort();
            write_file(
                output,
                &candidates
                    .iter()
                    .map(|(c1, c2)| format!("{c1},{c2}"))
                    .collect_vec(),
            )?;
        }
        Commands::Validate {
            results_path,
            threshold,
            candidates_path,
        } => {
            let false_positives = validation::validate(&candidates_path, &results_path, threshold)?;
            let candidates = validation::read_candidates(&candidates_path)?;
            let (correct, extra, precision) =
                validation::get_precision(&candidates, &results_path, threshold)?;

            println!("Correct trees;Extra trees;Precision");
            println!("{correct};{extra};{precision}");
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
