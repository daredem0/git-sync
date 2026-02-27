use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
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
    Create {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        with_patches: bool,
    },
    Audit {
        #[arg(long)]
        repo: Option<PathBuf>,
        #[arg(long)]
        bundle: Option<PathBuf>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long)]
        to: Option<String>,
        #[arg(long, default_value_t = false)]
        verify_metadata: bool,
        #[arg(long, value_enum, default_value_t = OutputFormat::Tsv)]
        format: OutputFormat,
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
    Receive {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long, default_value_t = false)]
        verify_metadata: bool,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Tsv,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditTarget {
    Bundle {
        bundle_path: PathBuf,
    },
    RepoRange {
        repo_path: PathBuf,
        from_rev: String,
        to_rev: String,
    },
}

pub fn resolve_audit_target(
    repo: Option<PathBuf>,
    bundle: Option<PathBuf>,
    from: Option<String>,
    to: Option<String>,
) -> Result<AuditTarget> {
    match (repo, bundle, from, to) {
        (None, Some(bundle_path), None, None) => Ok(AuditTarget::Bundle { bundle_path }),
        (Some(repo_path), None, Some(from_rev), Some(to_rev)) => Ok(AuditTarget::RepoRange {
            repo_path,
            from_rev,
            to_rev,
        }),
        (Some(_), Some(_), _, _) => bail!("provide either --repo or --bundle, not both"),
        (Some(_), None, _, None) => {
            bail!("repo audit mode requires both --from and --to")
        }
        (Some(_), None, None, Some(_)) => {
            bail!("repo audit mode requires both --from and --to")
        }
        (None, Some(_), Some(_), _) | (None, Some(_), _, Some(_)) => {
            bail!("bundle audit mode does not accept --from or --to")
        }
        (None, None, _, _) => {
            bail!("audit requires either --bundle or --repo with --from and --to")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verifies that resolve_audit_target selects bundle mode when only --bundle is provided.
    #[test]
    fn resolve_audit_target_accepts_bundle_mode_without_base_or_tip() {
        let bundle_path = PathBuf::from("sync.bundle");
        let result = resolve_audit_target(None, Some(bundle_path.clone()), None, None)
            .expect("bundle-only input should be accepted");
        assert_eq!(result, AuditTarget::Bundle { bundle_path });
    }

    // Verifies that resolve_audit_target selects repo mode when --repo, --from, and --to are provided.
    #[test]
    fn resolve_audit_target_accepts_repo_mode_with_from_and_to() {
        let repo_path = PathBuf::from(".");
        let result = resolve_audit_target(
            Some(repo_path.clone()),
            None,
            Some("HEAD~3".to_string()),
            Some("HEAD".to_string()),
        )
        .expect("repo range input should be accepted");
        assert_eq!(
            result,
            AuditTarget::RepoRange {
                repo_path,
                from_rev: "HEAD~3".to_string(),
                to_rev: "HEAD".to_string(),
            }
        );
    }

    // Verifies that resolve_audit_target rejects mixed bundle and repo arguments.
    #[test]
    fn resolve_audit_target_rejects_bundle_and_repo_combined() {
        let result = resolve_audit_target(
            Some(PathBuf::from(".")),
            Some(PathBuf::from("sync.bundle")),
            Some("HEAD~1".to_string()),
            Some("HEAD".to_string()),
        );
        assert!(
            result.is_err(),
            "audit mode selection must reject combined bundle and repo inputs"
        );
    }

    // Verifies that resolve_audit_target rejects repo mode when --from or --to is missing.
    #[test]
    fn resolve_audit_target_rejects_repo_mode_without_complete_range() {
        let missing_to = resolve_audit_target(
            Some(PathBuf::from(".")),
            None,
            Some("HEAD~1".to_string()),
            None,
        );
        assert!(missing_to.is_err(), "repo mode must reject missing --to");

        let missing_from = resolve_audit_target(
            Some(PathBuf::from(".")),
            None,
            None,
            Some("HEAD".to_string()),
        );
        assert!(
            missing_from.is_err(),
            "repo mode must reject missing --from"
        );
    }

    // Verifies that resolve_audit_target rejects from/to arguments when auditing a bundle directly.
    #[test]
    fn resolve_audit_target_rejects_bundle_mode_with_from_or_to() {
        let with_from = resolve_audit_target(
            None,
            Some(PathBuf::from("sync.bundle")),
            Some("HEAD~1".to_string()),
            None,
        );
        assert!(with_from.is_err(), "bundle mode must reject --from");

        let with_to = resolve_audit_target(
            None,
            Some(PathBuf::from("sync.bundle")),
            None,
            Some("HEAD".to_string()),
        );
        assert!(with_to.is_err(), "bundle mode must reject --to");
    }
}
