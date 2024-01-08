//! cco cli interface

use clap::{Parser, Subcommand, ValueEnum};
use std::fmt::Formatter;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Change the work directory
    ///
    /// Can be specified multiple times. Note that all
    /// paths on the way to the final path must exist.
    ///
    /// This is equivalent to running { cd <directory>; cco ... }
    #[clap(short = 'C', long = "directory", global(true))]
    pub directory: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Evaluate hcl expression
    ///
    /// Reads HCL from stdin unless any other source is provided (via --input-*)
    #[command(alias = "eval")]
    Evaluate(EvaluateCommand),

    /// Print debug information for development
    Dev(DevCommand),
}

#[derive(Parser, Debug)]
pub struct EvaluateCommand {
    #[clap(flatten)]
    pub input: InputArgs,

    #[clap(flatten)]
    pub output: OutputArgs,

    /// HCL expression to evaluate
    pub expression: String,
}

#[derive(Parser, Debug)]
pub struct InputArgs {
    /// Load files from work directory
    #[clap(short = 'w', long = "input-workdir")]
    pub workdir: bool,

    /// Load a file
    #[clap(short = 'f', long = "input-file")]
    pub files: Vec<PathBuf>,

    /// Load files from given directory
    #[clap(short = 'd', long = "input-dir")]
    pub directories: Vec<PathBuf>,

    /// Load files from work directory and up
    ///
    /// Load each directory walking up the tree.
    /// Stops when it no longer matches any files.
    /// Empty files are permitted.
    #[clap(short = 'c', long = "input-chain", conflicts_with("workdir"))]
    pub chain: bool,
}

#[derive(Parser, Debug)]
pub struct OutputArgs {
    #[arg(short = 'F', long = "output-format", default_value_t)]
    pub format: OutputFormat,
    // #[clap(short = 'O', long = "output-file")]
    // pub output_file: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Default, Debug)]
pub enum OutputFormat {
    Json,
    #[default]
    Yaml,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Json => f.write_str("json"),
            OutputFormat::Yaml => f.write_str("yaml"),
        }
    }
}

#[derive(Parser, Debug)]
pub struct DevCommand {
    #[command(subcommand)]
    pub command: DevSubCommand,
}

#[derive(Subcommand, Debug)]
pub enum DevSubCommand {
    Documents,
    Hcl,
}
