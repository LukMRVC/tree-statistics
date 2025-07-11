use crate::indexing::{Indexer, InvertedListLabelPostorderIndex, SEDIndex};
use crate::parsing::{tree_to_string, LabelDict, TreeOutput};
use crate::statistics::TreeStatistics;
use clap::error::ErrorKind;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use cli::{Cli, Commands, LowerBoundMethods};
use indexing::SEDIndexWithStructure;
use itertools::Itertools;
use lb::indexes;
use lb::label_intersection::{self, label_intersection_k};
use lb::sed::{sed_k, sed_struct_k};
use lb::structural_filter::{self, ted as struct_ted_k, LabelSetConverter};
use parsing::get_frequency_ordering;
use rand::seq::index;
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{self, exit};
use std::time::{Duration, Instant};
use std::u128;

mod cli;
mod indexing;
mod lb;
mod parsing;
mod statistics;
mod validation;

fn main() -> Result<(), anyhow::Error> {
    let cli = cli::Cli::parse();
    let mut cmd = cli::Cli::command();

    if !cli.dataset_path.exists() || !cli.dataset_path.is_file() {
        cmd.error(
            ErrorKind::InvalidValue,
            "Path does not exists or is not a valid file!",
        )
        .exit();
    }
    let mut label_dict = LabelDict::default();
    let trees = match parsing::parse_dataset(&cli.dataset_path, &mut label_dict) {
        Ok(trees) => trees,
        Err(e) => {
            eprintln!("Got unexpected error: {}", e);
            exit(1);
        }
    };

    for ((idx, tree), (idxnext, treenext)) in trees.iter().enumerate().tuple_windows() {
        if tree.count() > treenext.count() {
            // eprintln!("Tree {idx} has more nodes than tree {idxnext}");
            // exit(1);
        }
    }

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
            println!("Collection statistics\nmin_tree,max_tree,avg_tree,tree_count,avg_unique_labels_per_tree,avg_tree_distinct_labels,avg_sacking_index,avg_degree_stddev,distinct_labels\n{summary},{}", label_dict.keys().len());
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
            runs,
        } => {
            use LowerBoundMethods as LBM;
            if !output.is_dir() {
                eprintln!("Output arg must be a directory, is: {output:#?}");
                process::exit(1);
            }
            let q = q.unwrap_or(2);

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

            let ordering = get_frequency_ordering(&label_dict);

            let queries = parsing::parse_queries(&query_file, &mut label_dict).unwrap();
            let lbms: [LBM; 4] = [LBM::Lblint, LBM::Sed, LBM::Structural, LBM::SEDStruct];
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

                        let lblint_queries = queries
                            .iter()
                            .map(|(t, q)| {
                                (
                                    *t,
                                    InvertedListLabelPostorderIndex::index_tree(q, &label_dict),
                                )
                            })
                            .collect_vec();

                        let mut candidates = vec![];
                        let mut elapsed: Duration = Duration::MAX;
                        for _ in 0..runs {
                            let elapsed_run: Duration;
                            (candidates, elapsed_run) = lb::iterate_queries!(
                                lblint_queries,
                                lblint_indexes,
                                label_intersection_k,
                                size_map
                            );
                            elapsed = std::cmp::min(elapsed, elapsed_run)
                        }
                        (candidates, elapsed)
                    }
                    LBM::Sed => {
                        let sed_indexes = trees
                            .par_iter()
                            .map(|t| SEDIndex::index_tree(t, &label_dict))
                            .collect::<Vec<_>>();

                        let sed_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, SEDIndex::index_tree(q, &label_dict)))
                            .collect_vec();

                        let mut candidates = vec![];
                        let mut elapsed: Duration = Duration::MAX;
                        for _ in 0..runs {
                            let elapsed_run: Duration;
                            (candidates, elapsed_run) =
                                lb::iterate_queries!(sed_queries, sed_indexes, sed_k, size_map);
                            elapsed = std::cmp::min(elapsed, elapsed_run)
                        }
                        (candidates, elapsed)
                    }
                    LBM::SEDStruct => {
                        let sed_indexes = trees
                            .par_iter()
                            .map(|t| SEDIndexWithStructure::index_tree(t, &label_dict))
                            .collect::<Vec<_>>();

                        let sed_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, SEDIndexWithStructure::index_tree(q, &label_dict)))
                            .collect_vec();

                        let mut candidates = vec![];
                        let mut elapsed: Duration = Duration::MAX;
                        for _ in 0..runs {
                            let elapsed_run: Duration;
                            (candidates, elapsed_run) = lb::iterate_queries!(
                                sed_queries,
                                sed_indexes,
                                sed_struct_k,
                                size_map
                            );
                            elapsed = std::cmp::min(elapsed, elapsed_run)
                        }
                        (candidates, elapsed)
                    }
                    LBM::Structural => {
                        let mut lc = LabelSetConverter::default();
                        let structural_sets = lc.create(&trees);
                        let structural_queries = queries
                            .iter()
                            .map(|(t, q)| (*t, lc.create_single(q)))
                            .collect_vec();

                        let mut candidates = vec![];
                        let mut elapsed: Duration = Duration::MAX;
                        for _ in 0..runs {
                            let elapsed_run: Duration;
                            (candidates, elapsed_run) = lb::iterate_queries!(
                                structural_queries,
                                structural_sets,
                                struct_ted_k
                            );
                            elapsed = std::cmp::min(elapsed, elapsed_run)
                        }
                        (candidates, elapsed)
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
        Commands::Output {
            queries: queries_file,
            output,
        } => {
            if !output.is_dir() {
                eprintln!("Output arg must be a directory, is: {output:#?}");
                process::exit(1);
            }
            let queries = parsing::parse_dataset(&queries_file, &mut label_dict).unwrap();
            let mut output_path = output.clone();
            let mut output_q_path = output.clone();

            output_q_path.push(queries_file.file_name().expect("No queries file given!"));
            let query_strings = queries
                .par_iter()
                .map(|tree| tree_to_string(tree, TreeOutput::BracketNotation))
                .collect::<Vec<_>>();
            write_file(output_q_path, &query_strings)?;
            drop(query_strings);

            output_path.push(
                cli.dataset_path
                    .file_name()
                    .expect("No dataset path given!"),
            );
            let tree_strings = trees
                .par_iter()
                .map(|tree| tree_to_string(tree, TreeOutput::BracketNotation))
                .collect::<Vec<_>>();
            write_file(output_path, &tree_strings)?;
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

    write_file(
        [&out, &PathBuf::from("tree_sizes.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats.iter().map(|s| s.size).collect::<Vec<_>>(),
    )?;

    write_file(
        [&out, &PathBuf::from("sackins.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats.iter().map(|s| s.sacking_index).collect::<Vec<_>>(),
    )?;

    write_file(
        [&out, &PathBuf::from("degree_stddev.csv")]
            .iter()
            .collect::<PathBuf>(),
        &stats.iter().map(|s| s.degree_stddev).collect::<Vec<_>>(),
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
