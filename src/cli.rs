use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "git-sync-audit",
    version,
    about = "Air-gap Git sync audit tool (scaffold)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Audit {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value = "sync/last")]
        base: String,
        #[arg(long)]
        tip: Option<String>,
    },
    Ui {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value = "sync/last")]
        base: String,
        #[arg(long)]
        tip: Option<String>,
    },
}
