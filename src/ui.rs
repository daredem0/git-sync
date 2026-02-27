use crate::app::AppConfig;
use crate::git::{self, BundleVersion, ReceiveBundleOptions};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap};
use std::io;
use std::time::Duration;

pub fn run(config: &AppConfig) -> Result<()> {
    let model = build_overview_model(config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let loop_result = run_overview_loop(&mut terminal, &model);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_result
}

#[derive(Debug)]
struct OverviewModel {
    repo_path: String,
    bundle_path: String,
    base_ref: String,
    tip_ref: String,
    metadata_verification: StatusLine,
    dry_run: DryRunLine,
}

#[derive(Debug)]
enum StatusLine {
    Ok,
    Failed(String),
}

#[derive(Debug)]
enum DryRunLine {
    Ok(git::ReceiveBundleResult),
    Failed(String),
}

fn build_overview_model(config: &AppConfig) -> OverviewModel {
    if !config
        .bundle_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        let _ = git::open_context(config);
    }

    let metadata_verification = match git::verify_bundle_metadata_against_repo_input(
        &config.bundle_path,
        &config.repo_path,
    ) {
        Ok(()) => StatusLine::Ok,
        Err(err) => StatusLine::Failed(single_line_error(&err)),
    };

    let dry_run = match git::receive_bundle_input_with_options(
        &config.bundle_path,
        &config.repo_path,
        ReceiveBundleOptions {
            verify_metadata: false,
            dry_run: true,
        },
    ) {
        Ok(result) => DryRunLine::Ok(result),
        Err(err) => DryRunLine::Failed(single_line_error(&err)),
    };

    OverviewModel {
        repo_path: config.repo_path.display().to_string(),
        bundle_path: config.bundle_path.display().to_string(),
        base_ref: config.base_ref.clone(),
        tip_ref: config.tip_ref.clone().unwrap_or_else(|| "-".to_string()),
        metadata_verification,
        dry_run,
    }
}

fn run_overview_loop(
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    model: &OverviewModel,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render_overview(frame, model))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && matches!(key.code, KeyCode::Esc | KeyCode::Char('q'))
        {
            break;
        }
    }

    Ok(())
}

fn render_overview(frame: &mut Frame<'_>, model: &OverviewModel) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title = Paragraph::new(
        "Audit Overview\n\
         First page: package validity, import heads, and would-change summary\n\
         Press q or Esc to quit",
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("git-sync-audit"),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(title, chunks[0]);

    let general_lines = vec![
        format!("repo: {}", model.repo_path),
        format!("bundle: {}", model.bundle_path),
        format!("base_ref: {} | tip_ref: {}", model.base_ref, model.tip_ref),
        format!(
            "metadata verification: {}",
            render_status_line(&model.metadata_verification)
        ),
        format!(
            "dry-run applicability: {}",
            render_dry_run_status(&model.dry_run)
        ),
    ];
    let general = Paragraph::new(general_lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("General"));
    frame.render_widget(general, chunks[1]);

    match &model.dry_run {
        DryRunLine::Ok(result) => {
            let detail_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(chunks[2]);
            render_heads_table(frame, result, detail_chunks[0]);
            render_changes_table(frame, result, detail_chunks[1]);
        }
        DryRunLine::Failed(err) => {
            let failure = Paragraph::new(format!(
                "Dry-run failed, so no per-file summary is available.\nerror: {err}"
            ))
            .block(Block::default().borders(Borders::ALL).title("Would Change"))
            .wrap(Wrap { trim: false });
            frame.render_widget(failure, chunks[2]);
        }
    }

    let footer =
        Paragraph::new("Next page (planned): commit-by-commit tree and per-file diff view.")
            .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[3]);
}

fn render_heads_table(
    frame: &mut Frame<'_>,
    result: &git::ReceiveBundleResult,
    area: ratatui::layout::Rect,
) {
    let version = match result.bundle_version {
        BundleVersion::V2 => "v2",
        BundleVersion::V3 => "v3",
    };

    let rows: Vec<Row<'_>> = result
        .imported_heads
        .iter()
        .map(|head| {
            Row::new(vec![
                Cell::from(head.oid.to_string()),
                Cell::from(head.reference.clone()),
            ])
        })
        .collect();
    let heads_table = Table::new(rows, [Constraint::Length(40), Constraint::Min(20)])
        .header(Row::new(vec!["OID", "REF"]).style(Style::default().add_modifier(Modifier::BOLD)))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Heads To Import (bundle {version})")),
        )
        .column_spacing(2);
    frame.render_widget(heads_table, area);
}

fn render_changes_table(
    frame: &mut Frame<'_>,
    result: &git::ReceiveBundleResult,
    area: ratatui::layout::Rect,
) {
    let rows: Vec<Row<'_>> = if result.line_stats.is_empty() {
        vec![Row::new(vec![
            Cell::from("(no file content changes)"),
            Cell::from("-"),
            Cell::from("-"),
        ])]
    } else {
        result
            .line_stats
            .iter()
            .map(|stat| {
                Row::new(vec![
                    Cell::from(stat.path.clone()),
                    Cell::from(stat.additions.to_string()),
                    Cell::from(stat.deletions.to_string()),
                ])
            })
            .collect()
    };

    let changes_table = Table::new(
        rows,
        [
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["PATH", "+LINES", "-LINES"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Would Change (per-file line diff summary)"),
    )
    .column_spacing(2);
    frame.render_widget(changes_table, area);
}

fn render_status_line(status: &StatusLine) -> String {
    match status {
        StatusLine::Ok => "OK".to_string(),
        StatusLine::Failed(err) => format!("FAILED ({err})"),
    }
}

fn render_dry_run_status(status: &DryRunLine) -> String {
    match status {
        DryRunLine::Ok(result) => {
            if result.can_apply_without_conflicts {
                "bundle can be applied without conflicts".to_string()
            } else {
                "bundle cannot be applied cleanly".to_string()
            }
        }
        DryRunLine::Failed(err) => format!("FAILED ({err})"),
    }
}

fn single_line_error(err: &anyhow::Error) -> String {
    err.to_string().replace('\n', " ")
}
