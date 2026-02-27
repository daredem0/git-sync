mod app;
mod cli;
mod git;
mod ui;

use anyhow::Result;
use app::AppConfig;
use clap::Parser;
use cli::{Cli, Command, OutputFormat};
use git::{collect_changed_files, render_manifest, render_manifest_json};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Create {
            repo,
            from,
            to,
            output,
        }) => {
            let result = git::create_bundle(&repo, &from, &to, &output)?;
            println!(
                "bundle created: path={}, from={}, to={}, tip_ref={}",
                result.bundle_path.display(),
                result.from_commit_id,
                result.to_commit_id,
                result.tip_ref_name
            );
        }
        Some(Command::Audit {
            repo,
            bundle,
            base,
            tip,
            format,
        }) => {
            let config = AppConfig {
                repo_path: repo,
                bundle_path: bundle,
                base_ref: base,
                tip_ref: tip,
            };
            let context = git::open_context(&config)?;
            let tip_commit_id = context.tip_commit_id.unwrap_or(context.base_commit_id);
            let changes =
                collect_changed_files(&config.repo_path, context.base_commit_id, tip_commit_id)?;
            match format {
                OutputFormat::Tsv => {
                    let manifest = render_manifest(&changes);
                    print!("{manifest}");
                }
                OutputFormat::Json => {
                    let manifest = render_manifest_json(&changes)?;
                    println!("{manifest}");
                }
            }
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
