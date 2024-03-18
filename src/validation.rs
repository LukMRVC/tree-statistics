use crate::lb::indexes::histograms::Candidates;
use itertools::Itertools;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use rayon::prelude::*;

pub fn validate(candidates_file: PathBuf, results: PathBuf, k: usize) -> Result<(), anyhow::Error> {
    let cfile = File::open(candidates_file)?;
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
    let mut candidates = vec![];

    let creader = BufReader::new(cfile);
    let mut creader = csv::Reader::from_reader(creader);
    for result in creader.records() {
        let record = result?;
        let (t1, t2): (usize, usize) = (record[0].parse()?, record[1].parse()?);
        candidates.push((t1, t2));
    }
    candidates.sort();

    let mut not_found = vec![];

    for p1 in real_result.iter() {
        match candidates.binary_search(p1) {
            Ok(_) => continue,
            Err(_) => not_found.push(p1),
        };
    }

    // println!("{not_found:?}");
    println!(
        "Candidates and real result size diff is: {}, should have found: {} and found: {}",
        not_found.len(),
        real_result.len(),
        candidates.len()
    );

    if not_found.len() > 0 {
        let max = std::cmp::min(5, not_found.len());
        println!("Some not found candidates");
        for (c1, c2) in &not_found[..max] {
            println!("{c1} --- {c2}")
        }
    }

    Ok(())
}

pub fn get_precision(
    candidates: &Candidates,
    results_path: &PathBuf,
    k: usize,
) -> Result<(usize, usize, f32), anyhow::Error> {
    let rfile = File::open(results_path)?;
    let rreader = BufReader::new(rfile);
    let mut real_result = vec![];
    let mut rreader = csv::Reader::from_reader(rreader);
    for result in rreader.records() {
        let record = result?;
        let (t1, t2, dist): (usize, usize, usize) =
            (record[0].parse()?, record[1].parse()?, record[2].parse()?);
        if dist <= k {
            real_result.push((t1, t2));
        }
    }
    real_result.sort();
    let candidates = candidates.iter().sorted().cloned().collect_vec();
    let extra = candidates.iter().fold(0usize, |acc, candidate| {
        match real_result.binary_search(candidate) {
            Ok(_) => acc,
            Err(_) => acc + 1,
        }
    });

    let correct = candidates.len() - extra;
    let precision = correct as f32 / candidates.len() as f32;

    Ok((correct, extra, precision))
}
