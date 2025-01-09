use crate::indexing::{Indexer, InvertedListLabelPostorderIndex, SEDIndex};
use crate::parsing::{tree_to_string, LabelDict, TreeOutput};
use crate::statistics::TreeStatistics;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use itertools::Itertools;
use lb::indexes;
use lb::label_intersection::{self, label_intersection_k};
use lb::sed::sed_k;
use lb::structural_filter::{self, ted as struct_ted_k, LabelSetConverter};
use parsing::get_frequency_ordering;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{self, exit};
use std::time::{Duration, Instant};

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
    #[arg(long, default_value_t = false)]
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
        /// Q size for QGrams for SED indexing
        #[arg(long = "qgram-size")]
        q: Option<usize>,
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
    let mut label_dict = LabelDict::default();
    let mut trees = match parsing::parse_dataset(&cli.dataset_path, &mut label_dict) {
        Ok(trees) => trees,
        Err(e) => {
            eprintln!("Got unexpected error: {}", e);
            exit(1);
        }
    };
    trees.par_sort_by(|a, b| a.count().cmp(&b.count()));

    if !cli.quiet {
        println!("Parsed {} trees", trees.len());
    }

    match cli.command {
        Commands::Statistics { hists } => {
            let freq_ordering = get_frequency_ordering(&label_dict);
            let stats: Vec<_> = trees
                .par_iter()
                .map(|tree| statistics::gather(tree, &freq_ordering))
                .collect();
            let summary = statistics::summarize(&stats);
            println!("Collection statistics\nmin_tree,max_tree,avg_tree,tree_count,avg_unique_labels_per_tree,avg_tree_distinct_labels,distinct_labels,\n{summary},{}", label_dict.keys().len());
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
            method: filter_method,
            results_path: _results,
            q,
        } => {
            use LowerBoundMethods as LBM;
            if !output.is_dir() {
                eprintln!("Output arg must be a directory, is: {output:#?}");
                process::exit(1);
            }
            let q = q.unwrap_or(2);

            // let mut times = vec![];
            // let mut candidate_times = vec![];
            // let mut selectivities = vec![];
            if !cli.quiet {
                println!("Preparing dataset and running preprocessing for all methods");
            }
            let mut size_map = BTreeMap::new();
            let first = trees.first().unwrap();
            let mut size = first.count();
            size_map.insert(first.count(), 0);
            for (idx, t) in trees.iter().enumerate().skip(1) {
                if size != t.count() {
                    for i in (size + 1)..t.count() {
                        size_map.insert(i, idx);
                    }
                    size = t.count();
                    size_map.insert(size, idx);
                }
            }

            // let _collection_histograms = create_collection_histograms(&trees);

            // let split_distribution_map = structural_filter::best_split_distribution(&label_dict);
            // let split_distribution =
            // move |lbl: &LabelId| -> usize { *split_distribution_map.get(lbl).unwrap() };
            // let _structural_split_sets = lc.create_split(&trees, split_distribution);
            let ordering = get_frequency_ordering(&label_dict);

            let queries = parsing::parse_queries(&query_file, &mut label_dict).unwrap();
            let lbms: [LBM; 3] = [LBM::Lblint, LBM::Sed, LBM::Structural];
            // let label_dict = dbg!(label_dict);

            for current_method in lbms.iter().filter(|method| {
                if let Some(single_method) = filter_method {
                    return **method == single_method;
                }
                true
            }) {
                let (mut candidates, duration) = match *current_method {
                    LBM::Lblint => {
                        let lblint_indexes = trees
                            .par_iter()
                            .map(|t| InvertedListLabelPostorderIndex::index_tree(t, &label_dict))
                            .collect::<Vec<_>>();
                        let lblint_index =
                            label_intersection::LabelIntersectionIndex::new(&lblint_indexes);

                        let lblint_queries = queries
                            .iter()
                            .map(|(t, q)| {
                                (
                                    *t,
                                    InvertedListLabelPostorderIndex::index_tree(q, &label_dict),
                                )
                            })
                            .collect_vec();

                        let start = Instant::now();
                        let mut index_candidates = vec![];
                        for (qid, (t, query)) in lblint_queries.iter().enumerate() {
                            index_candidates.append(&mut lblint_index.query_index_prefix(
                                query,
                                *t,
                                &ordering,
                                &lblint_indexes,
                                Some(qid),
                            ));
                        }

                        println!(
                            "Lblint index\ntime:{dur}ms\ncandidates:{canlen}",
                            canlen = index_candidates.len(),
                            dur = start.elapsed().as_millis()
                        );
                        index_candidates.par_sort();
                        let mut output_file = output.clone();
                        output_file.push(format!("{current_method:#?}_index_candidates.csv"));
                        write_file(
                            output_file,
                            &index_candidates
                                .iter()
                                .map(|(c1, c2)| format!("{c1},{c2}"))
                                .collect_vec(),
                        )?;

                        lb::iterate_queries!(
                            lblint_queries,
                            lblint_indexes,
                            label_intersection_k,
                            size_map
                        )
                    }
                    LBM::Sed => {
                        let sed_indexes = trees
                            .par_iter()
                            .map(|t| SEDIndex::index_tree(t, &label_dict))
                            .collect::<Vec<_>>();
                        let pre_only = sed_indexes
                            .iter()
                            .map(|si| si.preorder.clone())
                            .collect::<Vec<Vec<i32>>>();
                        let start = Instant::now();
                        // TODO: Heuristic: Calculate the best Q for each dataset
                        // TODO: DBLP with Q = 2 is missing 4 results, find out why!
                        let mut pre_index = indexes::index_gram::IndexGram::new(&pre_only, q);
                        // let post_index = indexes::index_gram::IndexGram::new(&post_only, q);
                        if !cli.quiet {
                            println!("Building indexes took: {}ms", start.elapsed().as_millis());
                        }
                        let sed_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, SEDIndex::index_tree(q, &label_dict)))
                            .collect_vec();

                        // dbg!(&pre_only[])

                        let mut index_used_cnt = 0;
                        let mut index_candidates = Vec::with_capacity(15_000);
                        let start = Instant::now();
                        let sed_indexes_len = sed_indexes.len();
                        let mut total_lookup_duration = Duration::new(0, 0);
                        let mut total_filter_duration = Duration::new(0, 0);
                        let mut avg_precision = 0.0;

                        for (qid, (threshold, sed_query)) in sed_queries.iter().enumerate() {
                            let c1 = pre_index.query(sed_query.preorder.clone(), *threshold);
                            if let Ok((c1, lookup_duration, filter_duration)) = c1 {
                                index_used_cnt += 1;
                                total_lookup_duration += lookup_duration;
                                total_filter_duration += filter_duration;

                                let mut correct_results = 0;
                                for cid in c1.iter() {
                                    if sed_k(sed_query, &sed_indexes[*cid], *threshold)
                                        <= *threshold
                                    {
                                        correct_results += 1;
                                        index_candidates.push((qid, *cid));
                                    }
                                }
                                let precision =
                                    correct_results as f64 / std::cmp::max(c1.len(), 1) as f64;
                                avg_precision = avg_precision
                                    + (precision - avg_precision) / (index_used_cnt as f64);
                            } else {
                                let start_idx = size_map
                                    .get(&sed_query.c.tree_size.saturating_sub(*threshold))
                                    .unwrap_or(&0);
                                let end_idx = size_map
                                    .get(&(sed_query.c.tree_size + threshold + 1))
                                    .unwrap_or(&sed_indexes_len);
                                let idx_diff = end_idx - start_idx + 1;
                                // println!("Starting from {start_idx} and taking at most {idx_diff} trees!");

                                for (tid, tree) in sed_indexes
                                    .iter()
                                    .enumerate()
                                    .skip(*start_idx)
                                    .take(idx_diff)
                                {
                                    if sed_k(sed_query, tree, *threshold) <= *threshold {
                                        index_candidates.push((qid, tid));
                                    }
                                }
                            }
                        }

                        println!(
                            "Sed Index\ntime:{}ms\ncandidates:{}",
                            start.elapsed().as_millis(),
                            index_candidates.len(),
                        );

                        // println!(
                        //     "Total lookup duration was: {}ms",
                        //     total_lookup_duration.as_millis(),
                        // );
                        // println!(
                        //     "Total count filter duration was: {}ms --- CNT:{}ms/TM:{}ms",
                        //     total_filter_duration.as_millis(),
                        //     pre_index.cnt.as_millis(),
                        //     pre_index.true_matches.as_millis(),
                        // );

                        // index_candidates.par_sort();
                        // let mut output_file = output.clone();
                        // output_file.push(format!("{current_method:#?}_index_candidates.csv"));
                        // write_file(
                        //     output_file,
                        //     &index_candidates
                        //         .iter()
                        //         .map(|(c1, c2)| format!("{c1},{c2}"))
                        //         .collect_vec(),
                        // )?;

                        lb::iterate_queries!(sed_queries, sed_indexes, sed_k, size_map)
                    }
                    LBM::Structural => {
                        let mut lc = LabelSetConverter::default();
                        let structural_sets = lc.create(&trees);
                        let struct_index =
                            structural_filter::StructuralFilterIndex::new(&structural_sets);
                        let structural_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, lc.create_single(q)))
                            .collect_vec();

                        let start = Instant::now();
                        let index_candidates = structural_queries
                            .par_iter()
                            .enumerate()
                            .flat_map(|(qid, (t, query))| {
                                struct_index.query_index_prefix(
                                    query,
                                    &ordering,
                                    *t,
                                    &structural_sets,
                                    Some(qid),
                                )
                            })
                            .collect::<Vec<(usize, usize)>>();
                        println!(
                            "Structural Index\ntime:{dur}ms\ncandidates:{canlen}",
                            canlen = index_candidates.len(),
                            dur = start.elapsed().as_millis()
                        );
                        // index_candidates.par_sort();
                        // let mut output_file = output.clone();
                        // output_file.push(format!("{current_method:#?}_index_candidates.csv"));
                        // write_file(
                        //     output_file,
                        //     &index_candidates
                        //         .iter()
                        //         .map(|(c1, c2)| format!("{c1},{c2}"))
                        //         .collect_vec(),
                        // )?;

                        lb::iterate_queries!(structural_queries, structural_sets, struct_ted_k)
                    }
                    _ => todo!(),
                };

                println!(
                    "{current_method:?}\ntime:{duration_ms}ms\ncandidates:{canlen}",
                    duration_ms = duration.as_millis(),
                    canlen = candidates.len()
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
    write_file(
        [&out, &PathBuf::from("unique_labels.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats
            .iter()
            .map(|s| s.collection_unique_labels)
            .collect::<Vec<_>>(),
    )?;
    write_file(
        [&out, &PathBuf::from("distinct_labels.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats.iter().map(|s| s.distinct_labels).collect::<Vec<_>>(),
    )?;

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
