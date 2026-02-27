mod app;
mod cli;
mod git;
mod ui;

use anyhow::Result;
use app::AppConfig;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Audit {
            repo,
            bundle,
            base,
            tip,
        }) => {
            let config = AppConfig {
                repo_path: repo,
                bundle_path: bundle,
                base_ref: base,
                tip_ref: tip,
            };
            git::open_context(&config)?;
            println!("Audit command scaffold is ready.");
            println!("Implementation is intentionally pending.");
        }
        Some(Command::Ui {
            repo,
            bundle,
            base,
            tip,
        }) => {
            let config = AppConfig {
                repo_path: repo,
                bundle_path: bundle,
                base_ref: base,
                tip_ref: tip,
            };
            git::open_context(&config)?;
            ui::run(&config)?;
            println!("UI command scaffold is ready.");
            println!("Implementation is intentionally pending.");
        }
        None => {
            println!("git-sync-audit scaffold is ready.");
            println!("Use --help to inspect planned commands.");
        }
    }

    Ok(())
}
