use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

/// Tree statistics utility
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Dataset file of trees in bracket notation
    #[arg(short, long, value_name = "FILE")]
    pub dataset_path: PathBuf,
    /// outputs only collected statistics
    #[arg(long, default_value_t = false)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum LowerBoundMethods {
    /// Histogram lower bound
    Hist,
    /// Label intersection lower bound
    Lblint,
    /// String edit distance lower bound
    Sed,
    /// String edit distance with structure lower bound
    SEDStruct,
    /// Structural filter lower bound
    Structural,
    /// Structural variant filter lower bound
    StructuralVariant,
    /// Binary branch lower bound
    Bib,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum SedTraversal {
    Preorder,
    Postorder,
    ReversedPreorder,
    ReversedPostorder,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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
    /// Just output processed and parsed trees with labels as numbers instead of original strings
    Output {
        /// input path for queries in bracket notation
        #[arg(long)]
        queries: PathBuf,
        /// output path for trees in bracket notation
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
        /// Total number of runs for each method
        /// Then the lowest duration of all runs is taken as result
        #[arg(long = "runs", short = 'r', default_value_t = 1)]
        runs: usize,
        /// First pass query traversal for SED / SEDStruct
        #[arg(long = "sed-first-traversal", value_enum, default_value_t = SedTraversal::ReversedPreorder)]
        sed_first_traversal: SedTraversal,
        /// Second pass query traversal for SED / SEDStruct
        #[arg(long = "sed-second-traversal", value_enum, default_value_t = SedTraversal::Preorder)]
        sed_second_traversal: SedTraversal,
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
