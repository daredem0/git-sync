use crate::app::AppConfig;
use crate::git::{
    self, BundleVersion, CommitAuditEntry, CommitAuditIdentity, ReceiveBundleOptions,
};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};

const DIFF_SCROLL_VERTICAL_STEP: usize = 1;
const DIFF_SCROLL_HORIZONTAL_STEP: usize = 2;
const DIFF_SCROLL_PAGE_STEP: usize = 20;

pub fn run(config: &AppConfig) -> Result<()> {
    let model = build_audit_model(config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut app_state = AppState::new(&model);

    let loop_result = run_loop(&mut terminal, &model, &mut app_state);

    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_result
}

#[derive(Debug)]
struct AuditModel {
    overview: OverviewModel,
    commit_pages: CommitPagesModel,
    repo_path: PathBuf,
    bundle_path: PathBuf,
    syntax_highlighter: SyntaxHighlighter,
}

#[derive(Debug)]
enum CommitPagesModel {
    Ok(Vec<CommitAuditEntry>),
    Failed(String),
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

#[derive(Debug)]
struct AppState {
    page_index: usize,
    selected_file_indices: Vec<usize>,
    show_help: bool,
    action_message: Option<String>,
    diff_view: Option<DiffViewState>,
}

#[derive(Debug, Clone)]
struct DiffViewState {
    commit_index: usize,
    commit_total: usize,
    file_index: usize,
    commit_id: git2::Oid,
    commit_subject: String,
    file_path: String,
    syntax_name: String,
    lines: Vec<Line<'static>>,
    max_line_width: usize,
    scroll_y: usize,
    scroll_x: usize,
}

#[derive(Debug)]
struct RenderedDiff {
    syntax_name: String,
    lines: Vec<Line<'static>>,
    max_line_width: usize,
}

#[derive(Debug)]
struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchLineKind {
    Header,
    Hunk,
    Added,
    Deleted,
    Context,
    Other,
}

impl SyntaxHighlighter {
    fn load() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let themes = ThemeSet::load_defaults();
        let theme = themes
            .themes
            .get("base16-ocean.dark")
            .or_else(|| themes.themes.get("InspiredGitHub"))
            .or_else(|| themes.themes.values().next())
            .cloned()
            .expect("syntect should provide at least one built-in theme");

        Self { syntax_set, theme }
    }

    fn resolve_syntax_for_path<'a>(&'a self, path: &str) -> (&'a SyntaxReference, String) {
        let extension = Path::new(path).extension().and_then(|ext| ext.to_str());
        let syntax = extension
            .and_then(|ext| self.syntax_set.find_syntax_by_extension(ext))
            .unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());
        (syntax, syntax.name.to_string())
    }
}

impl AppState {
    fn new(model: &AuditModel) -> Self {
        let selected_file_indices = match &model.commit_pages {
            CommitPagesModel::Ok(entries) => vec![0; entries.len()],
            CommitPagesModel::Failed(_) => Vec::new(),
        };
        Self {
            page_index: 0,
            selected_file_indices,
            show_help: false,
            action_message: None,
            diff_view: None,
        }
    }

    fn total_pages(&self, model: &AuditModel) -> usize {
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => {
                if entries.is_empty() {
                    1
                } else {
                    1 + entries.len()
                }
            }
            CommitPagesModel::Failed(_) => 2,
        }
    }

    fn next_page(&mut self, model: &AuditModel) {
        let last = self.total_pages(model).saturating_sub(1);
        self.page_index = std::cmp::min(self.page_index + 1, last);
        self.action_message = None;
    }

    fn previous_page(&mut self) {
        self.page_index = self.page_index.saturating_sub(1);
        self.action_message = None;
    }

    fn first_page(&mut self) {
        self.page_index = 0;
        self.action_message = None;
    }

    fn last_page(&mut self, model: &AuditModel) {
        self.page_index = self.total_pages(model).saturating_sub(1);
        self.action_message = None;
    }

    fn move_selection_down(&mut self, model: &AuditModel) {
        let Some((commit_index, file_count)) = self.current_commit_context(model) else {
            return;
        };
        if file_count == 0 {
            return;
        }
        if let Some(selected) = self.selected_file_indices.get_mut(commit_index) {
            *selected = std::cmp::min(*selected + 1, file_count - 1);
        }
    }

    fn move_selection_up(&mut self, model: &AuditModel) {
        let Some((commit_index, _)) = self.current_commit_context(model) else {
            return;
        };
        if let Some(selected) = self.selected_file_indices.get_mut(commit_index) {
            *selected = selected.saturating_sub(1);
        }
    }

    fn selected_file_index(&self, commit_index: usize) -> usize {
        self.selected_file_indices
            .get(commit_index)
            .copied()
            .unwrap_or(0)
    }

    fn current_commit_context(&self, model: &AuditModel) -> Option<(usize, usize)> {
        if self.page_index == 0 {
            return None;
        }
        match &model.commit_pages {
            CommitPagesModel::Ok(entries) => {
                let commit_index = self.page_index - 1;
                let file_count = entries.get(commit_index)?.files.len();
                Some((commit_index, file_count))
            }
            CommitPagesModel::Failed(_) => None,
        }
    }

    fn is_diff_open(&self) -> bool {
        self.diff_view.is_some()
    }

    fn close_diff(&mut self) {
        self.diff_view = None;
        self.action_message = None;
    }

    fn open_selected_diff(&mut self, model: &AuditModel) {
        let Some((commit_index, file_count)) = self.current_commit_context(model) else {
            return;
        };
        if file_count == 0 {
            self.action_message = Some("selected commit has no file content changes".to_string());
            return;
        }

        let CommitPagesModel::Ok(entries) = &model.commit_pages else {
            self.action_message = Some("commit pages are unavailable".to_string());
            return;
        };

        let Some(commit_entry) = entries.get(commit_index) else {
            self.action_message = Some("commit index is out of range".to_string());
            return;
        };

        let file_index = self
            .selected_file_index(commit_index)
            .min(commit_entry.files.len() - 1);
        let file_path = commit_entry.files[file_index].path.clone();

        let patch = git::collect_commit_file_patch_for_bundle_input(
            &model.bundle_path,
            &model.repo_path,
            commit_entry.commit_id,
            &file_path,
        );

        match patch {
            Ok(patch_text) => {
                let rendered =
                    render_patch_with_syntax(&file_path, &patch_text, &model.syntax_highlighter);
                self.diff_view = Some(DiffViewState {
                    commit_index,
                    commit_total: entries.len(),
                    file_index,
                    commit_id: commit_entry.commit_id,
                    commit_subject: commit_entry.subject.clone(),
                    file_path,
                    syntax_name: rendered.syntax_name,
                    lines: rendered.lines,
                    max_line_width: rendered.max_line_width,
                    scroll_y: 0,
                    scroll_x: 0,
                });
                self.action_message = None;
            }
            Err(err) => {
                self.action_message = Some(format!(
                    "failed to open patch view: {}",
                    single_line_error(&err)
                ));
            }
        }
    }

    fn scroll_diff_down(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            let last = view.lines.len().saturating_sub(1);
            view.scroll_y = std::cmp::min(view.scroll_y.saturating_add(step), last);
        }
    }

    fn scroll_diff_up(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            view.scroll_y = view.scroll_y.saturating_sub(step);
        }
    }

    fn scroll_diff_right(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            let max = view.max_line_width.saturating_sub(1);
            view.scroll_x = std::cmp::min(view.scroll_x.saturating_add(step), max);
        }
    }

    fn scroll_diff_left(&mut self, step: usize) {
        if let Some(view) = self.diff_view.as_mut() {
            view.scroll_x = view.scroll_x.saturating_sub(step);
        }
    }

    fn reset_diff_scroll(&mut self) {
        if let Some(view) = self.diff_view.as_mut() {
            view.scroll_x = 0;
            view.scroll_y = 0;
        }
    }
}

fn build_audit_model(config: &AppConfig) -> AuditModel {
    let overview = build_overview_model(config);
    let commit_pages = match git::collect_commit_audit_entries_for_bundle_input(
        &config.bundle_path,
        &config.repo_path,
    ) {
        Ok(entries) => CommitPagesModel::Ok(entries),
        Err(err) => CommitPagesModel::Failed(single_line_error(&err)),
    };

    AuditModel {
        overview,
        commit_pages,
        repo_path: config.repo_path.clone(),
        bundle_path: config.bundle_path.clone(),
        syntax_highlighter: SyntaxHighlighter::load(),
    }
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

fn run_loop(
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    model: &AuditModel,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render_page(frame, model, state))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if handle_key_press(state, model, key.code) {
                break;
            }
        }
    }

    Ok(())
}

fn handle_key_press(state: &mut AppState, model: &AuditModel, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') => true,
        KeyCode::Esc => {
            if state.is_diff_open() {
                state.close_diff();
                false
            } else {
                true
            }
        }
        KeyCode::Char('?') => {
            state.show_help = !state.show_help;
            false
        }
        _ => {
            if state.is_diff_open() {
                handle_diff_keys(state, code);
            } else {
                handle_page_keys(state, model, code);
            }
            false
        }
    }
}

fn handle_page_keys(state: &mut AppState, model: &AuditModel, code: KeyCode) {
    match code {
        KeyCode::Right | KeyCode::Char('l') => state.next_page(model),
        KeyCode::Left | KeyCode::Char('h') => state.previous_page(),
        KeyCode::Down | KeyCode::Char('j') => state.move_selection_down(model),
        KeyCode::Up | KeyCode::Char('k') => state.move_selection_up(model),
        KeyCode::Char('g') => state.first_page(),
        KeyCode::Char('G') => state.last_page(model),
        KeyCode::Enter => state.open_selected_diff(model),
        _ => {}
    }
}

fn handle_diff_keys(state: &mut AppState, code: KeyCode) {
    match code {
        KeyCode::Down | KeyCode::Char('j') => state.scroll_diff_down(DIFF_SCROLL_VERTICAL_STEP),
        KeyCode::Up | KeyCode::Char('k') => state.scroll_diff_up(DIFF_SCROLL_VERTICAL_STEP),
        KeyCode::Right | KeyCode::Char('l') => state.scroll_diff_right(DIFF_SCROLL_HORIZONTAL_STEP),
        KeyCode::Left | KeyCode::Char('h') => state.scroll_diff_left(DIFF_SCROLL_HORIZONTAL_STEP),
        KeyCode::PageDown => state.scroll_diff_down(DIFF_SCROLL_PAGE_STEP),
        KeyCode::PageUp => state.scroll_diff_up(DIFF_SCROLL_PAGE_STEP),
        KeyCode::Home => state.reset_diff_scroll(),
        _ => {}
    }
}

fn render_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    if state.is_diff_open() {
        render_diff_view(frame, state);
    } else if state.page_index == 0 {
        render_overview_page(frame, model, state);
    } else {
        render_commit_page(frame, model, state);
    }

    if state.show_help {
        render_help_overlay(frame, state.is_diff_open());
    }
}

fn render_diff_view(frame: &mut Frame<'_>, state: &AppState) {
    let Some(diff_view) = &state.diff_view else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let header = Paragraph::new(format!(
        "Commit {}/{} | {}\n{}\nfile: {}\nsyntax: {} | selected file index: {}",
        diff_view.commit_index + 1,
        diff_view.commit_total,
        diff_view.commit_id,
        diff_view.commit_subject,
        diff_view.file_path,
        diff_view.syntax_name,
        diff_view.file_index + 1
    ))
    .block(Block::default().borders(Borders::ALL).title("Diff View"))
    .wrap(Wrap { trim: false });
    frame.render_widget(header, chunks[0]);

    let diff_text = Text::from(diff_view.lines.clone());
    let diff_paragraph = Paragraph::new(diff_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Patch (first-parent commit diff)"),
        )
        .scroll((
            u16::try_from(diff_view.scroll_y).unwrap_or(u16::MAX),
            u16::try_from(diff_view.scroll_x).unwrap_or(u16::MAX),
        ));
    frame.render_widget(diff_paragraph, chunks[1]);

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[2]);
}

fn render_overview_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    let overview = &model.overview;
    let page_label = format!("page {}/{}", state.page_index + 1, state.total_pages(model));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    let title = Paragraph::new(format!(
        "Audit Overview ({})\n\
             This page shows package validity, import heads, and would-change summary\n\
             Use h/l or left/right to move pages",
        page_label
    ))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("git-sync-audit"),
    )
    .wrap(Wrap { trim: false });
    frame.render_widget(title, chunks[0]);

    let general_lines = vec![
        format!("repo: {}", overview.repo_path),
        format!("bundle: {}", overview.bundle_path),
        format!(
            "base_ref: {} | tip_ref: {}",
            overview.base_ref, overview.tip_ref
        ),
        format!(
            "metadata verification: {}",
            render_status_line(&overview.metadata_verification)
        ),
        format!(
            "dry-run applicability: {}",
            render_dry_run_status(&overview.dry_run)
        ),
    ];
    let general = Paragraph::new(general_lines.join("\n"))
        .block(Block::default().borders(Borders::ALL).title("General"));
    frame.render_widget(general, chunks[1]);

    match &overview.dry_run {
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

    let footer = Paragraph::new(render_footer_text(state))
        .style(Style::default().add_modifier(Modifier::ITALIC));
    frame.render_widget(footer, chunks[3]);
}

fn render_commit_page(frame: &mut Frame<'_>, model: &AuditModel, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    match &model.commit_pages {
        CommitPagesModel::Failed(err) => {
            let page_label = format!("page {}/{}", state.page_index + 1, state.total_pages(model));
            let message = Paragraph::new(format!(
                "Commit page data is unavailable ({})\nerror: {}\n\
                 The overview page is still usable for package-level auditing.",
                page_label, err
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Commit Pages Unavailable"),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(message, chunks[0]);
            frame.render_widget(
                Paragraph::new("No commit list can be rendered for this package.").block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Changed Files"),
                ),
                chunks[1],
            );
            frame.render_widget(
                Paragraph::new(render_footer_text(state))
                    .style(Style::default().add_modifier(Modifier::ITALIC)),
                chunks[2],
            );
        }
        CommitPagesModel::Ok(entries) => {
            let commit_index = state.page_index.saturating_sub(1);
            let Some(entry) = entries.get(commit_index) else {
                frame.render_widget(
                    Paragraph::new("Page index is out of bounds for commit entries.")
                        .block(Block::default().borders(Borders::ALL).title("Commit")),
                    chunks[0],
                );
                frame.render_widget(
                    Paragraph::new(render_footer_text(state))
                        .style(Style::default().add_modifier(Modifier::ITALIC)),
                    chunks[2],
                );
                return;
            };

            let header = Paragraph::new(format!(
                "Commit {}/{} | {}\n{}\ncommitter date: {}\ncommitter: {}\nauthor date: {}\nauthor: {}\nChanged files: {}",
                commit_index + 1,
                entries.len(),
                entry.commit_id,
                entry.subject,
                format_git_timestamp(
                    entry.committer.time_seconds,
                    entry.committer.offset_minutes,
                ),
                format_identity(&entry.committer),
                format_git_timestamp(entry.author.time_seconds, entry.author.offset_minutes),
                format_identity(&entry.author),
                entry.files.len()
            ))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Commit Detail"),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(header, chunks[0]);

            render_commit_files_table(frame, entry, commit_index, state, chunks[1]);
            frame.render_widget(
                Paragraph::new(render_footer_text(state))
                    .style(Style::default().add_modifier(Modifier::ITALIC)),
                chunks[2],
            );
        }
    }
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

fn render_commit_files_table(
    frame: &mut Frame<'_>,
    entry: &CommitAuditEntry,
    commit_index: usize,
    state: &AppState,
    area: Rect,
) {
    if entry.files.is_empty() {
        let empty = Paragraph::new("(no file content changes in this commit)").block(
            Block::default()
                .borders(Borders::ALL)
                .title("Changed Files"),
        );
        frame.render_widget(empty, area);
        return;
    }

    let rows: Vec<Row<'_>> = entry
        .files
        .iter()
        .map(|stat| {
            Row::new(vec![
                Cell::from(stat.path.clone()),
                Cell::from(stat.additions.to_string()),
                Cell::from(stat.deletions.to_string()),
            ])
        })
        .collect();

    let files_table = Table::new(
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
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Changed Files (this commit)"),
    )
    .column_spacing(2);

    let mut table_state = TableState::default();
    table_state.select(Some(
        state
            .selected_file_index(commit_index)
            .min(entry.files.len() - 1),
    ));
    frame.render_stateful_widget(files_table, area, &mut table_state);
}

fn render_patch_with_syntax(
    path: &str,
    patch: &str,
    highlighter: &SyntaxHighlighter,
) -> RenderedDiff {
    let (syntax, syntax_name) = highlighter.resolve_syntax_for_path(path);
    let mut syntax_state = HighlightLines::new(syntax, &highlighter.theme);

    let mut old_line: Option<usize> = None;
    let mut new_line: Option<usize> = None;
    let mut lines = Vec::new();
    let mut max_line_width = 0usize;

    for raw_line in patch.lines() {
        if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
            old_line = Some(old_start);
            new_line = Some(new_start);
        }

        let kind = classify_patch_line(raw_line);
        let (old_display, new_display) = line_number_columns(kind, &mut old_line, &mut new_line);
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("{:>6} {:>6} │ ", old_display, new_display),
            Style::default().fg(Color::DarkGray),
        ));

        spans.extend(render_patch_content_line(
            raw_line,
            kind,
            &mut syntax_state,
            &highlighter.syntax_set,
        ));

        let visual_width = raw_line.chars().count() + 18;
        max_line_width = std::cmp::max(max_line_width, visual_width);
        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(patch contains no renderable text lines)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    RenderedDiff {
        syntax_name,
        lines,
        max_line_width,
    }
}

fn line_number_columns(
    kind: PatchLineKind,
    old_line: &mut Option<usize>,
    new_line: &mut Option<usize>,
) -> (String, String) {
    match kind {
        PatchLineKind::Added => {
            let display = new_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            if let Some(value) = new_line.as_mut() {
                *value += 1;
            }
            ("".to_string(), display)
        }
        PatchLineKind::Deleted => {
            let display = old_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            if let Some(value) = old_line.as_mut() {
                *value += 1;
            }
            (display, "".to_string())
        }
        PatchLineKind::Context => {
            let old_display = old_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            let new_display = new_line
                .map(|value| value.to_string())
                .unwrap_or_else(|| "".to_string());
            if let Some(value) = old_line.as_mut() {
                *value += 1;
            }
            if let Some(value) = new_line.as_mut() {
                *value += 1;
            }
            (old_display, new_display)
        }
        _ => ("".to_string(), "".to_string()),
    }
}

fn render_patch_content_line(
    line: &str,
    kind: PatchLineKind,
    syntax_state: &mut HighlightLines<'_>,
    syntax_set: &SyntaxSet,
) -> Vec<Span<'static>> {
    let semantic_style = semantic_content_style(kind);

    match kind {
        PatchLineKind::Header => vec![Span::styled(line.to_string(), semantic_style)],
        PatchLineKind::Hunk => vec![Span::styled(line.to_string(), semantic_style)],
        PatchLineKind::Other => vec![Span::styled(line.to_string(), semantic_style)],
        PatchLineKind::Added | PatchLineKind::Deleted | PatchLineKind::Context => {
            let prefix_len = line.chars().next().map(char::len_utf8).unwrap_or(0);
            let (prefix, content) = line.split_at(prefix_len);
            let mut spans = vec![Span::styled(
                prefix.to_string(),
                semantic_prefix_style(kind),
            )];

            let mut highlight_input = String::with_capacity(content.len() + 1);
            highlight_input.push_str(content);
            highlight_input.push('\n');

            let regions = syntax_state.highlight_line(&highlight_input, syntax_set);
            match regions {
                Ok(regions) if !regions.is_empty() => {
                    let last = regions.len() - 1;
                    for (index, (style, segment)) in regions.into_iter().enumerate() {
                        let text = if index == last {
                            segment.strip_suffix('\n').unwrap_or(segment)
                        } else {
                            segment
                        };
                        if text.is_empty() {
                            continue;
                        }
                        let span_style = syntect_style_to_ratatui(style).patch(semantic_style);
                        spans.push(Span::styled(text.to_string(), span_style));
                    }
                }
                _ => {
                    spans.push(Span::styled(content.to_string(), semantic_style));
                }
            }

            if spans.len() == 1 {
                spans.push(Span::styled(String::new(), semantic_style));
            }
            spans
        }
    }
}

fn classify_patch_line(line: &str) -> PatchLineKind {
    if line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("similarity index ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
    {
        return PatchLineKind::Header;
    }

    if line.starts_with("@@") {
        return PatchLineKind::Hunk;
    }

    if line.starts_with('+') && !line.starts_with("+++") {
        return PatchLineKind::Added;
    }

    if line.starts_with('-') && !line.starts_with("---") {
        return PatchLineKind::Deleted;
    }

    if line.starts_with(' ') {
        return PatchLineKind::Context;
    }

    PatchLineKind::Other
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_part, rest) = rest.split_once(" +")?;
    let (new_part, _) = rest.split_once(" @@")?;

    let old_start = old_part.split(',').next()?.parse::<usize>().ok()?;
    let new_start = new_part.split(',').next()?.parse::<usize>().ok()?;

    Some((old_start, new_start))
}

fn semantic_content_style(kind: PatchLineKind) -> Style {
    match kind {
        PatchLineKind::Added => Style::default().bg(Color::Rgb(18, 46, 20)),
        PatchLineKind::Deleted => Style::default().bg(Color::Rgb(52, 20, 20)),
        PatchLineKind::Hunk => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        PatchLineKind::Header => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
        _ => Style::default(),
    }
}

fn semantic_prefix_style(kind: PatchLineKind) -> Style {
    match kind {
        PatchLineKind::Added => Style::default()
            .fg(Color::Green)
            .bg(Color::Rgb(18, 46, 20))
            .add_modifier(Modifier::BOLD),
        PatchLineKind::Deleted => Style::default()
            .fg(Color::Red)
            .bg(Color::Rgb(52, 20, 20))
            .add_modifier(Modifier::BOLD),
        PatchLineKind::Context => Style::default().fg(Color::DarkGray),
        _ => Style::default(),
    }
}

fn syntect_style_to_ratatui(style: syntect::highlighting::Style) -> Style {
    let mut result = Style::default().fg(Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    ));

    if style.font_style.contains(FontStyle::BOLD) {
        result = result.add_modifier(Modifier::BOLD);
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        result = result.add_modifier(Modifier::ITALIC);
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        result = result.add_modifier(Modifier::UNDERLINED);
    }

    result
}

fn render_footer_text(state: &AppState) -> String {
    let base = if state.is_diff_open() {
        "j/k or Up/Down scroll | h/l or Left/Right horizontal | PgUp/PgDn fast scroll | Home reset | Esc back | ? help | q quit"
    } else {
        "h/Left prev page | l/Right next page | j/k or Up/Down move | Enter open diff | ? help | q quit"
    };
    match &state.action_message {
        Some(message) => format!("{base} | {message}"),
        None => base.to_string(),
    }
}

fn render_help_overlay(frame: &mut Frame<'_>, in_diff_view: bool) {
    let area = centered_rect(75, 45, frame.area());
    frame.render_widget(Clear, area);
    let help_text = help_text_for_mode(in_diff_view);

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Keymap"))
        .wrap(Wrap { trim: false });
    frame.render_widget(help, area);
}

fn help_text_for_mode(in_diff_view: bool) -> &'static str {
    if in_diff_view {
        "Navigation (Diff View)\n\
         - j / Down: scroll down\n\
         - k / Up: scroll up\n\
         - h / Left: horizontal scroll left\n\
         - l / Right: horizontal scroll right\n\
         - PgUp / PgDn: fast vertical scroll\n\
         - Home: reset scroll\n\
         - Esc: close diff and return to commit page\n\
         - ?: toggle this help\n\
         - q: quit"
    } else {
        "Navigation (Page View)\n\
         - h / Left: previous page\n\
         - l / Right: next page\n\
         - j / Down: move file selection down on commit pages\n\
         - k / Up: move file selection up on commit pages\n\
         - g: first page\n\
         - G: last page\n\
         - Enter: open selected file diff view\n\
         - ?: toggle this help\n\
         - q / Esc: quit"
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
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

fn format_identity(identity: &CommitAuditIdentity) -> String {
    format!("{} <{}>", identity.name, identity.email)
}

fn format_git_timestamp(seconds: i64, offset_minutes: i32) -> String {
    let sign = if offset_minutes < 0 { '-' } else { '+' };
    let absolute = offset_minutes.abs();
    let hours = absolute / 60;
    let minutes = absolute % 60;
    format!("{seconds} (UTC{sign}{hours:02}:{minutes:02})")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug)]
    struct DiffFixture {
        source_dir: PathBuf,
        receiver_dir: PathBuf,
        bundle_archive_path: PathBuf,
        entries: Vec<CommitAuditEntry>,
    }

    impl Drop for DiffFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.source_dir);
            let _ = fs::remove_dir_all(&self.receiver_dir);
        }
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "git-sync-audit-ui-{}-{}-{}",
            prefix,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock should be after unix epoch")
                .as_nanos()
        ))
    }

    fn commit_from_files(
        repo: &git2::Repository,
        message: &str,
        files: &[(&str, &str)],
        parent_oids: &[git2::Oid],
    ) -> git2::Oid {
        let mut builder = repo.treebuilder(None).expect("must create tree builder");
        for (path, content) in files {
            let blob_id = repo
                .blob(content.as_bytes())
                .expect("must create blob object");
            builder
                .insert(*path, blob_id, 0o100644)
                .expect("must insert file entry");
        }
        let tree_id = builder.write().expect("must write tree");
        let tree = repo.find_tree(tree_id).expect("must find written tree");
        let parent_commits: Vec<git2::Commit<'_>> = parent_oids
            .iter()
            .map(|oid| repo.find_commit(*oid).expect("must resolve parent"))
            .collect();
        let parent_refs: Vec<&git2::Commit<'_>> = parent_commits.iter().collect();
        let sig = git2::Signature::now("UI Test", "ui-test@example.com")
            .expect("must create commit signature");
        repo.commit(None, &sig, &sig, message, &tree, &parent_refs)
            .expect("must create commit")
    }

    fn create_diff_fixture() -> DiffFixture {
        let source_dir = unique_temp_dir("source");
        fs::create_dir_all(&source_dir).expect("must create source dir");
        let source_repo = git2::Repository::init(&source_dir).expect("must init source repo");

        let base_commit = commit_from_files(
            &source_repo,
            "base",
            &[("f.rs", "fn value() -> i32 { 1 }\n")],
            &[],
        );
        let tip_commit = commit_from_files(
            &source_repo,
            "tip",
            &[
                ("f.rs", "fn value() -> i32 { 2 }\n"),
                ("g.txt", "new file\n"),
            ],
            &[base_commit],
        );
        source_repo
            .reference("refs/heads/base", base_commit, true, "create base ref")
            .expect("must create base ref");
        source_repo
            .reference("refs/heads/tip", tip_commit, true, "create tip ref")
            .expect("must create tip ref");

        let bundle_path = source_dir.join("sync.bundle");
        let bundle_result = git::create_bundle(
            &source_dir,
            "refs/heads/base",
            "refs/heads/tip",
            &bundle_path,
        )
        .expect("must create bundle package");
        git::remove_unarchived_bundle_artifacts(&bundle_result)
            .expect("must remove unarchived artifacts");

        let receiver_dir = unique_temp_dir("receiver");
        fs::create_dir_all(&receiver_dir).expect("must create receiver dir");
        let receiver_repo = git2::Repository::init_bare(&receiver_dir).expect("must init receiver");
        let mut source_remote = receiver_repo
            .remote_anonymous(source_dir.to_str().expect("source path should be utf-8"))
            .expect("must create source remote");
        source_remote
            .fetch(&["refs/heads/base:refs/heads/base"], None, None)
            .expect("must fetch prerequisite base ref");

        let entries = git::collect_commit_audit_entries_for_bundle_input(
            &bundle_result.archive_path,
            &receiver_dir,
        )
        .expect("must collect commit entries for fixture bundle");
        assert_eq!(
            entries.len(),
            1,
            "fixture should contain one commit in base..tip"
        );

        DiffFixture {
            source_dir,
            receiver_dir,
            bundle_archive_path: bundle_result.archive_path,
            entries,
        }
    }

    fn sample_model(commit_count: usize, files_per_commit: usize) -> AuditModel {
        let commit_pages = CommitPagesModel::Ok(
            (0..commit_count)
                .map(|i| CommitAuditEntry {
                    commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                        .expect("valid oid"),
                    subject: format!("commit-{i}"),
                    committer: CommitAuditIdentity {
                        name: "Committer".to_string(),
                        email: "committer@example.com".to_string(),
                        time_seconds: 1_700_000_000,
                        offset_minutes: 60,
                    },
                    author: CommitAuditIdentity {
                        name: "Author".to_string(),
                        email: "author@example.com".to_string(),
                        time_seconds: 1_700_000_001,
                        offset_minutes: 60,
                    },
                    files: (0..files_per_commit)
                        .map(|n| git::FileLineStat {
                            path: format!("file-{n}.txt"),
                            additions: n + 1,
                            deletions: n,
                        })
                        .collect(),
                })
                .collect(),
        );

        AuditModel {
            overview: OverviewModel {
                repo_path: ".".to_string(),
                bundle_path: "sync.bundle.zip".to_string(),
                base_ref: "sync/last".to_string(),
                tip_ref: "main".to_string(),
                metadata_verification: StatusLine::Ok,
                dry_run: DryRunLine::Failed("not needed for state tests".to_string()),
            },
            commit_pages,
            repo_path: PathBuf::from("."),
            bundle_path: PathBuf::from("sync.bundle.zip"),
            syntax_highlighter: SyntaxHighlighter::load(),
        }
    }

    fn build_model_from_fixture(fixture: &DiffFixture) -> AuditModel {
        AuditModel {
            overview: OverviewModel {
                repo_path: fixture.receiver_dir.display().to_string(),
                bundle_path: fixture.bundle_archive_path.display().to_string(),
                base_ref: "sync/last".to_string(),
                tip_ref: "-".to_string(),
                metadata_verification: StatusLine::Ok,
                dry_run: DryRunLine::Failed("not needed for ui unit tests".to_string()),
            },
            commit_pages: CommitPagesModel::Ok(fixture.entries.clone()),
            repo_path: fixture.receiver_dir.clone(),
            bundle_path: fixture.bundle_archive_path.clone(),
            syntax_highlighter: SyntaxHighlighter::load(),
        }
    }

    // Verifies that total_pages returns one overview page plus one page per commit.
    #[test]
    fn app_state_total_pages_counts_overview_and_commits() {
        let model = sample_model(3, 2);
        let state = AppState::new(&model);
        assert_eq!(state.total_pages(&model), 4);
    }

    // Verifies that page navigation clamps at the first and last available page.
    #[test]
    fn app_state_page_navigation_is_bounded() {
        let model = sample_model(2, 1);
        let mut state = AppState::new(&model);

        state.previous_page();
        assert_eq!(state.page_index, 0);

        state.next_page(&model);
        state.next_page(&model);
        state.next_page(&model);
        assert_eq!(state.page_index, 2);

        state.first_page();
        assert_eq!(state.page_index, 0);

        state.last_page(&model);
        assert_eq!(state.page_index, 2);
    }

    // Verifies that file selection movement on commit pages stays within valid row bounds.
    #[test]
    fn app_state_selection_movement_is_bounded() {
        let model = sample_model(1, 2);
        let mut state = AppState::new(&model);
        state.next_page(&model);
        assert_eq!(state.page_index, 1);

        state.move_selection_down(&model);
        assert_eq!(state.selected_file_index(0), 1);

        state.move_selection_down(&model);
        assert_eq!(state.selected_file_index(0), 1);

        state.move_selection_up(&model);
        assert_eq!(state.selected_file_index(0), 0);

        state.move_selection_up(&model);
        assert_eq!(state.selected_file_index(0), 0);
    }

    // Verifies that identity formatting is rendered as "Name <email>" for commit detail display.
    #[test]
    fn format_identity_renders_name_and_email() {
        let identity = CommitAuditIdentity {
            name: "Florian".to_string(),
            email: "florian@example.com".to_string(),
            time_seconds: 0,
            offset_minutes: 0,
        };
        assert_eq!(
            format_identity(&identity),
            "Florian <florian@example.com>".to_string()
        );
    }

    // Verifies that timestamp formatting keeps unix seconds and renders timezone offset in UTC form.
    #[test]
    fn format_git_timestamp_renders_seconds_and_offset() {
        assert_eq!(
            format_git_timestamp(1_700_000_000, -90),
            "1700000000 (UTC-01:30)".to_string()
        );
    }

    // Verifies that patch line classification detects headers, hunks, additions, deletions, and context lines.
    #[test]
    fn classify_patch_line_detects_core_kinds() {
        assert_eq!(
            classify_patch_line("diff --git a/a.txt b/a.txt"),
            PatchLineKind::Header
        );
        assert_eq!(classify_patch_line("@@ -1,2 +1,2 @@"), PatchLineKind::Hunk);
        assert_eq!(classify_patch_line("+added"), PatchLineKind::Added);
        assert_eq!(classify_patch_line("-removed"), PatchLineKind::Deleted);
        assert_eq!(classify_patch_line(" context"), PatchLineKind::Context);
    }

    // Verifies that hunk header parsing extracts old and new line starts.
    #[test]
    fn parse_hunk_header_extracts_line_starts() {
        assert_eq!(parse_hunk_header("@@ -12,3 +48,7 @@ fn x"), Some((12, 48)));
        assert_eq!(parse_hunk_header("not a hunk"), None);
    }

    // Verifies that rendered patch output includes line-number prefix and styled content rows.
    #[test]
    fn render_patch_with_syntax_includes_line_number_column() {
        let highlighter = SyntaxHighlighter::load();
        let patch =
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1 +1 @@\n-old\n+new\n";
        let rendered = render_patch_with_syntax("a.txt", patch, &highlighter);
        assert!(
            !rendered.lines.is_empty(),
            "rendered diff should contain lines for a valid patch"
        );
        let first = rendered.lines[0]
            .spans
            .first()
            .map(|span| span.content.to_string())
            .unwrap_or_default();
        assert!(
            first.contains('│'),
            "rendered rows should include a line-number column separator"
        );
    }

    // Verifies that opening a diff from non-commit context does not create an active diff view.
    #[test]
    fn open_selected_diff_noop_outside_commit_context() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        state.open_selected_diff(&model);
        assert!(
            state.diff_view.is_none(),
            "diff view should remain closed when not on a commit page"
        );
    }

    // Verifies that opening a selected diff on a commit page creates a populated diff view model.
    #[test]
    fn open_selected_diff_creates_diff_view_for_selected_file() {
        let fixture = create_diff_fixture();
        let model = build_model_from_fixture(&fixture);
        let mut state = AppState::new(&model);
        state.next_page(&model);
        let commit_index = 0usize;
        let target_index = fixture.entries[commit_index]
            .files
            .iter()
            .position(|file| file.path == "f.rs")
            .expect("fixture commit should contain f.rs");
        state.selected_file_indices[commit_index] = target_index;

        state.open_selected_diff(&model);

        let diff_view = state
            .diff_view
            .as_ref()
            .expect("diff view should be opened for selected file");
        assert_eq!(
            diff_view.commit_total,
            fixture.entries.len(),
            "diff view should carry total commit count for header rendering"
        );
        assert_eq!(
            diff_view.file_path, "f.rs",
            "diff view should open the selected commit file path"
        );
        assert!(
            !diff_view.lines.is_empty(),
            "diff view should include rendered patch lines"
        );
        assert!(
            diff_view.syntax_name.contains("Rust"),
            "syntax detection should identify Rust for .rs files"
        );
        assert!(
            state.action_message.is_none(),
            "opening valid diff should not set an error/action message"
        );
    }

    // Verifies that diff scrolling clamps to valid bounds and never underflows/overflows.
    #[test]
    fn diff_scroll_is_bounded() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        state.diff_view = Some(DiffViewState {
            commit_index: 0,
            commit_total: 1,
            file_index: 0,
            commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                .expect("valid oid"),
            commit_subject: "subject".to_string(),
            file_path: "f.rs".to_string(),
            syntax_name: "Rust".to_string(),
            lines: vec![
                Line::from("line 1"),
                Line::from("line 2"),
                Line::from("line 3"),
            ],
            max_line_width: 12,
            scroll_y: 0,
            scroll_x: 0,
        });

        state.scroll_diff_up(100);
        state.scroll_diff_left(100);
        assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_y, 0);
        assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_x, 0);

        state.scroll_diff_down(100);
        state.scroll_diff_right(100);
        assert_eq!(
            state.diff_view.as_ref().expect("diff view").scroll_y,
            2,
            "vertical diff scroll should clamp to last line index"
        );
        assert_eq!(
            state.diff_view.as_ref().expect("diff view").scroll_x,
            11,
            "horizontal diff scroll should clamp to max_line_width - 1"
        );

        state.reset_diff_scroll();
        assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_y, 0);
        assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_x, 0);
    }

    // Verifies that Esc closes diff view without requesting app exit, and Esc exits when no diff is open.
    #[test]
    fn handle_key_press_esc_closes_diff_then_exits() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);
        state.diff_view = Some(DiffViewState {
            commit_index: 0,
            commit_total: 1,
            file_index: 0,
            commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                .expect("valid oid"),
            commit_subject: "subject".to_string(),
            file_path: "f.rs".to_string(),
            syntax_name: "Rust".to_string(),
            lines: vec![Line::from("line 1")],
            max_line_width: 10,
            scroll_y: 0,
            scroll_x: 0,
        });

        let should_exit_with_diff = handle_key_press(&mut state, &model, KeyCode::Esc);
        assert!(
            !should_exit_with_diff,
            "Esc should close active diff view instead of exiting application"
        );
        assert!(
            state.diff_view.is_none(),
            "Esc should clear diff view state when diff is open"
        );

        let should_exit_without_diff = handle_key_press(&mut state, &model, KeyCode::Esc);
        assert!(
            should_exit_without_diff,
            "Esc should request exit when no diff view is active"
        );
    }

    // Verifies that line-number column tracking stays consistent across context/delete/add sequences.
    #[test]
    fn line_number_columns_tracks_old_and_new_counters() {
        let mut old = Some(10usize);
        let mut new = Some(20usize);

        let context = line_number_columns(PatchLineKind::Context, &mut old, &mut new);
        assert_eq!(context, ("10".to_string(), "20".to_string()));
        assert_eq!(old, Some(11));
        assert_eq!(new, Some(21));

        let deleted = line_number_columns(PatchLineKind::Deleted, &mut old, &mut new);
        assert_eq!(deleted, ("11".to_string(), "".to_string()));
        assert_eq!(old, Some(12));
        assert_eq!(new, Some(21));

        let added = line_number_columns(PatchLineKind::Added, &mut old, &mut new);
        assert_eq!(added, ("".to_string(), "21".to_string()));
        assert_eq!(old, Some(12));
        assert_eq!(new, Some(22));
    }

    // Verifies that file-header lines with +++/--- are classified as headers, not add/delete content.
    #[test]
    fn classify_patch_line_treats_file_headers_as_headers() {
        assert_eq!(
            classify_patch_line("+++ b/src/main.rs"),
            PatchLineKind::Header
        );
        assert_eq!(
            classify_patch_line("--- a/src/main.rs"),
            PatchLineKind::Header
        );
    }

    // Verifies that syntax resolution falls back to plain text when file extension is unknown.
    #[test]
    fn resolve_syntax_for_unknown_extension_falls_back_to_plain_text() {
        let highlighter = SyntaxHighlighter::load();
        let (_, syntax_name) = highlighter.resolve_syntax_for_path("file.unknownext");
        assert_eq!(
            syntax_name,
            highlighter.syntax_set.find_syntax_plain_text().name,
            "unknown extensions should fall back to plain text syntax"
        );
    }

    // Verifies that footer text switches to diff controls only when a diff view is active.
    #[test]
    fn render_footer_text_switches_between_page_and_diff_modes() {
        let model = sample_model(1, 1);
        let mut state = AppState::new(&model);

        let page_footer = render_footer_text(&state);
        assert!(
            page_footer.contains("Enter open diff"),
            "page mode footer should include commit-page action hints"
        );

        state.diff_view = Some(DiffViewState {
            commit_index: 0,
            commit_total: 1,
            file_index: 0,
            commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
                .expect("valid oid"),
            commit_subject: "subject".to_string(),
            file_path: "f.rs".to_string(),
            syntax_name: "Rust".to_string(),
            lines: vec![Line::from("line 1")],
            max_line_width: 10,
            scroll_y: 0,
            scroll_x: 0,
        });
        let diff_footer = render_footer_text(&state);
        assert!(
            diff_footer.contains("PgUp/PgDn"),
            "diff mode footer should include scrolling key hints"
        );
    }

    // Verifies that help text content changes between page mode and diff mode.
    #[test]
    fn help_text_for_mode_switches_content_by_view() {
        let page_help = help_text_for_mode(false);
        assert!(
            page_help.contains("Enter: open selected file diff view"),
            "page help should mention opening a file diff"
        );

        let diff_help = help_text_for_mode(true);
        assert!(
            diff_help.contains("Esc: close diff and return to commit page"),
            "diff help should mention returning from diff view"
        );
    }
}
