use crate::lb::indexes::histograms::Candidates;

use rayon::prelude::*;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub fn read_candidates(
    candidates_file: &impl AsRef<Path>,
) -> Result<Vec<(usize, usize)>, anyhow::Error> {
    let cfile = File::open(candidates_file)?;
    let mut candidates = vec![];

    let creader = BufReader::new(cfile);
    let mut creader = csv::Reader::from_reader(creader);
    for result in creader.records() {
        let record = result?;
        let (t1, t2): (usize, usize) = (record[0].parse()?, record[1].parse()?);
        candidates.push((t1, t2));
    }
    candidates.par_sort();
    Ok(candidates)
}

pub fn validate(
    candidates_file: &impl AsRef<Path>,
    results: &impl AsRef<Path>,
    k: usize,
) -> Result<Vec<(usize, usize)>, anyhow::Error> {
    let rfile = File::open(results)?;

    let mut real_result = vec![];
    let rreader = BufReader::new(rfile);
    let mut rreader = csv::Reader::from_reader(rreader);
    for result in rreader.records() {
        let record = result?;
        let (t1, t2, dist): (usize, usize, usize) =
            (record[0].parse()?, record[1].parse()?, record[2].parse()?);
        if dist <= k {
            real_result.push((t1, t2));
        }
    }
    real_result.par_sort();
    let candidates = read_candidates(candidates_file)?;

    let not_found = real_result
        .iter()
        .filter_map(|(p1, p2)| {
            candidates.binary_search(&(*p1, *p2)).map_or_else(
                |_| {
                    let flipped = &(*p2, *p1);
                    candidates
                        .binary_search(flipped)
                        .map_or(Some((*p1, *p2)), |_| None)
                },
                |_| None,
            )
        })
        .collect::<Vec<_>>();

    let _false_positives = candidates
        .par_iter()
        .filter_map(|(p1, p2)| {
            real_result
                .binary_search(&(*p1, *p2))
                .map_or(Some((*p1, *p2)), |_| {
                    let flipped = &(*p2, *p1);
                    real_result
                        .binary_search(flipped)
                        .map_or(Some((*p1, *p2)), |_| None)
                })
        })
        .collect::<Vec<_>>();

    println!(
        "Candidates and real result size diff is: {}, should have found: {} and found: {}",
        not_found.len(),
        real_result.len(),
        candidates.len()
    );

    if !not_found.is_empty() {
        let max = std::cmp::min(5, not_found.len());
        println!("Some not found candidates");
        for (c1, c2) in &not_found[..max] {
            println!("{c1} --- {c2}")
        }
    }

    Ok(not_found)
}

pub fn get_precision(
    candidates: &Candidates,
    results_path: &PathBuf,
    k: usize,
) -> Result<(usize, usize, f32, f64), anyhow::Error> {
    let rfile = File::open(results_path)?;
    let rreader = BufReader::new(rfile);
    let mut real_result = vec![];
    let mut rreader = csv::Reader::from_reader(rreader);
    let mut max_tree_id = 0;
    for result in rreader.records() {
        let record = result?;
        let (t1, t2, dist): (usize, usize, usize) =
            (record[0].parse()?, record[1].parse()?, record[2].parse()?);
        if dist <= k {
            real_result.push((t1, t2));
        }
        max_tree_id = std::cmp::max(max_tree_id, std::cmp::max(t1, t2));
    }
    real_result.par_sort();
    let max_tree_id = real_result.iter().map(|tup| tup.0).max().unwrap();
    let mut matches = vec![0; max_tree_id + 1];
    matches.par_iter_mut().enumerate().for_each(|(tree_id, tau_match)| {
        for (t1, t2) in real_result.iter() {
            if *t1 == tree_id || *t2 == tree_id {
                *tau_match += 1;
            }
        }
    });
    let selectivity: Vec<f64> = matches.par_iter().enumerate().map(|(tree_id, tau_match)| 100f64 *  (*tau_match as f64 / (max_tree_id + 1) as f64)).collect();
    let mean_sel = crate::statistics::mean(&selectivity);
    

    let extra = candidates.iter().fold(0usize, |acc, (c1, c2)| {
        match real_result.binary_search(&(*c1, *c2)) {
            Ok(_) => acc,
            Err(_) => match real_result.binary_search(&(*c2, *c1)) {
                Ok(_) => acc,
                Err(_) => acc + 1,
            },
        }
    });

    let correct = candidates.len() - extra;
    let precision = correct as f32 / candidates.len() as f32;

    Ok((correct, extra, precision, mean_sel))
}
