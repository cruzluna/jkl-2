use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use thiserror::Error;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use log::{debug, info};

const DATA_NOT_RECEIVED: &str = "-";
const INFO_TEXT: &str = "(Esc/Ctrl+C) back/quit | (/) search | (Enter) switch | (↑/↓) move | (g/G) top/bottom | (0-9) jump | (l/h) expand/collapse | (r) refresh";

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("{0}")]
    BackendFailure(String),
    #[error("{0}")]
    ContextResolutionFailure(String),
    #[error("todo")]
    StateConstructionFailure,
    #[error("Terminal I/O error: {0}")]
    TerminalIo(#[from] io::Error),
}

pub fn run() -> Result<(), TuiError> {
    info!("tui starting");
    let sessions =
        crate::tmux::list_sessions().map_err(|e| TuiError::BackendFailure(e.to_string()))?;
    info!(
        "tui loaded {} tmux sessions: {:?}",
        sessions.len(),
        sessions
    );

    let contexts = crate::context::load_contexts()
        .map_err(|e| TuiError::ContextResolutionFailure(e.to_string()))?;
    info!("tui loaded {} contexts", contexts.len());

    let panes = crate::tmux::list_panes().map_err(|e| TuiError::BackendFailure(e.to_string()))?;
    info!("tui loaded {} panes", panes.len());

    let items = build_sessions(sessions, contexts, panes);
    info!("tui built {} session rows", items.len());

    let mut app = App::new(items)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    info!("tui exiting");
    result
}

pub fn run_pane_selector(pane_id: String, session_name: Option<String>) -> Result<(), TuiError> {
    info!(
        "tui pane selector starting pane_id={} session_name={:?}",
        pane_id, session_name
    );
    let mut selector = PaneSelector::new(pane_id, session_name)?;
    let mut terminal = ratatui::init();
    let result = selector.run(&mut terminal);
    ratatui::restore();
    info!("tui pane selector exiting");
    result
}

#[derive(Clone)]
struct SessionRow {
    /// The session ID
    id: String,
    /// The name of the session
    name: String,
    /// The status of the session
    status: Option<crate::context::AgentStatus>,
    /// The context of the session
    context: String,
    /// The panes belonging to the session
    panes: Vec<PaneRow>,
}

#[derive(Clone)]
struct PaneRow {
    /// The pane ID
    id: String,
    /// Temporary name of a pane, likely to fall out of sync with the pane ID
    alias: Option<String>,
    /// The status of the pane
    status: Option<crate::context::AgentStatus>,
    /// The context of the pane
    context: String,
    /// The session ID
    session_id: String,
}

#[derive(Clone)]
enum RowItem {
    Session(SessionRow),
    Pane(PaneRow),
}

#[derive(Clone, PartialEq, Eq)]
enum RowKey {
    Session(String),
    Pane { session_id: String, pane_id: String },
}

impl RowItem {
    fn key(&self) -> RowKey {
        match self {
            RowItem::Session(row) => RowKey::Session(row.id.clone()),
            RowItem::Pane(row) => RowKey::Pane {
                session_id: row.session_id.clone(),
                pane_id: row.id.clone(),
            },
        }
    }
}

enum ListViewModes {
    /// Default mode when launching
    NormalMode,
    /// Mode when searching for a session or pane
    SearchMode,
}

struct SearchQuery {
    query: String,
    // TODO: cursor position
}

/// Column widths for the session table (Session, Status, Context).
struct ColumnWidths {
    session: u16,
    status: u16,
    context: u16,
}

type FilterFn = Box<dyn Fn(&str, &[String]) -> Result<String, TuiError>>;

struct App {
    state: TableState,
    sessions: Vec<SessionRow>,
    filtered_sessions: Vec<SessionRow>,
    rows: Vec<RowItem>,
    session_index_by_id: HashMap<String, usize>,
    widths: ColumnWidths,
    search: SearchQuery,
    mode: ListViewModes,
    expanded_sessions: HashSet<String>,
    jump_buffer: String,
    filter: FilterFn,
}

impl App {
    /// Creates a list view of sessions and panes
    fn new(sessions: Vec<SessionRow>) -> Result<Self, TuiError> {
        Self::new_with_filter(sessions, Box::new(run_fzf_filter))
    }

    fn new_with_filter(sessions: Vec<SessionRow>, filter: FilterFn) -> Result<Self, TuiError> {
        let mut app = Self {
            state: TableState::default(),
            filtered_sessions: sessions.clone(),
            sessions,
            rows: Vec::new(),
            session_index_by_id: HashMap::new(),
            widths: ColumnWidths {
                session: 0,
                status: 0,
                context: 0,
            },
            search: SearchQuery {
                query: String::new(),
            },
            mode: ListViewModes::NormalMode,
            expanded_sessions: HashSet::new(),
            jump_buffer: String::new(),
            filter,
        };
        app.rebuild_rows();
        app.ensure_selection();
        Ok(app)
    }
    /// Runs the main event loop for the list view
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), TuiError> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if matches!(self.mode, ListViewModes::SearchMode) {
                    match key.code {
                        KeyCode::Esc => {
                            self.mode = ListViewModes::NormalMode;
                            self.jump_buffer.clear();
                        }
                        KeyCode::Enter => {
                            self.jump_buffer.clear();
                            self.switch_session()?;
                            return Ok(());
                        }
                        KeyCode::Backspace => {
                            self.search.query.pop();
                            self.apply_search()?;
                        }
                        KeyCode::Down => self.next_row(),
                        KeyCode::Up => self.previous_row(),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.mode = ListViewModes::NormalMode;
                            self.jump_buffer.clear();
                        }
                        // Append a character to the search query
                        KeyCode::Char(c) => {
                            self.search.query.push(c);
                            self.apply_search()?;
                        }
                        _ => {}
                    }
                } else {
                    // Normal mode keybindings
                    let is_digit = matches!(key.code, KeyCode::Char(c) if c.is_ascii_digit());
                    if !is_digit {
                        self.jump_buffer.clear();
                    }
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        KeyCode::Char('/') => {
                            self.mode = ListViewModes::SearchMode;
                            self.apply_search()?;
                        }
                        KeyCode::Enter => {
                            self.switch_session()?;
                            return Ok(());
                        }
                        KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                        KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
                        KeyCode::Char('g') => {
                            self.select_first_session();
                        }
                        KeyCode::Char('G') => {
                            self.select_last_session();
                        }
                        KeyCode::Char('l') => self.expand_selected(),
                        KeyCode::Char('h') => self.collapse_selected(),
                        KeyCode::Char('r') => {
                            info!("tui refresh requested");
                            self.refresh_panes()?;
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            self.jump_buffer.push(c);
                            if let Ok(index) = self.jump_buffer.parse::<usize>() {
                                self.jump_to_session_index(index);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn next_row(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let next = match self.state.selected() {
            Some(index) if index + 1 < self.rows.len() => index + 1,
            _ => 0,
        };
        self.state.select(Some(next));
    }

    fn previous_row(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let prev = match self.state.selected() {
            Some(0) | None => self.rows.len() - 1,
            Some(index) => index - 1,
        };
        self.state.select(Some(prev));
    }

    fn ensure_selection(&mut self) {
        if self.rows.is_empty() {
            self.state.select(None);
        } else if self.state.selected().is_none() {
            self.state.select(Some(0));
        }
    }

    fn selected_row(&self) -> Option<&RowItem> {
        let index = self.state.selected()?;
        self.rows.get(index)
    }

    fn selected_key(&self) -> Option<RowKey> {
        self.selected_row().map(RowItem::key)
    }

    fn apply_search(&mut self) -> Result<(), TuiError> {
        let previous = self.selected_key();
        self.apply_search_with(previous)
    }

    fn apply_search_with(&mut self, previous: Option<RowKey>) -> Result<(), TuiError> {
        if self.search.query.trim().is_empty() {
            self.filtered_sessions = self.sessions.clone();
            self.rebuild_rows();
            self.restore_selection(previous);
            return Ok(());
        }

        let candidates = self
            .sessions
            .iter()
            .map(|row| {
                format!(
                    "{}\t{}\t{}\t{}",
                    row.id,
                    row.name,
                    status_text(row.status.as_ref()),
                    row.context
                )
            })
            .collect::<Vec<_>>();

        let output = (self.filter.as_ref())(&self.search.query, &candidates)?;
        let mut lines = output.lines();
        let _ = lines.next();

        let lookup: HashMap<&str, &SessionRow> = self
            .sessions
            .iter()
            .map(|row| (row.id.as_str(), row))
            .collect();
        let mut filtered = Vec::new();
        for line in lines {
            if let Some(id) = line.split('\t').next() {
                if let Some(row) = lookup.get(id) {
                    filtered.push((*row).clone());
                }
            }
        }
        self.filtered_sessions = filtered;
        self.rebuild_rows();
        self.restore_selection(previous);
        Ok(())
    }

    fn rebuild_rows(&mut self) {
        self.session_index_by_id = self
            .filtered_sessions
            .iter()
            .enumerate()
            .map(|(index, session)| (session.id.clone(), index))
            .collect();
        let mut rows = Vec::new();
        for session in &self.filtered_sessions {
            rows.push(RowItem::Session(session.clone()));
            if self.expanded_sessions.contains(&session.id) {
                for pane in &session.panes {
                    rows.push(RowItem::Pane(pane.clone()));
                }
            }
        }
        self.rows = rows;
        self.widths = self.measure_widths();
    }

    fn restore_selection(&mut self, previous: Option<RowKey>) {
        if self.rows.is_empty() {
            self.state.select(None);
            return;
        }
        if let Some(key) = previous {
            if let Some(index) = self.rows.iter().position(|row| row.key() == key) {
                self.state.select(Some(index));
                return;
            }
        }
        self.state.select(Some(0));
    }

    /// Switches to the selected session or pane
    fn switch_session(&self) -> Result<(), TuiError> {
        if let Some(row) = self.selected_row() {
            let target_session_id = match row {
                RowItem::Session(session) => session.id.as_str(),
                RowItem::Pane(pane) => pane.session_id.as_str(),
            };
            info!("tui switching to session_id={}", target_session_id);
            crate::tmux::switch_client(target_session_id)
                .map_err(|e| TuiError::BackendFailure(e.to_string()))?;
        }
        Ok(())
    }

    fn expand_selected(&mut self) {
        let previous = self.selected_key();
        let session_id = self.selected_row().map(|row| match row {
            RowItem::Session(session) => session.id.clone(),
            RowItem::Pane(pane) => pane.session_id.clone(),
        });
        if let Some(session_id) = session_id {
            self.expanded_sessions.insert(session_id);
            self.rebuild_rows();
            self.restore_selection(previous);
        }
    }

    fn collapse_selected(&mut self) {
        let previous = self.selected_key();
        let session_id = self.selected_row().map(|row| match row {
            RowItem::Session(session) => session.id.clone(),
            RowItem::Pane(pane) => pane.session_id.clone(),
        });
        if let Some(session_id) = session_id {
            self.expanded_sessions.remove(&session_id);
            self.rebuild_rows();
            self.restore_selection(previous);
        }
    }

    fn refresh_panes(&mut self) -> Result<(), TuiError> {
        let live_panes =
            crate::tmux::list_panes().map_err(|e| TuiError::BackendFailure(e.to_string()))?;
        let live_map = collect_live_panes(&live_panes);
        crate::context::prune_panes(&live_map)
            .map_err(|e| TuiError::ContextResolutionFailure(e.to_string()))?;
        self.reload_data()?;
        Ok(())
    }

    fn reload_data(&mut self) -> Result<(), TuiError> {
        info!("tui reload_data starting");
        let previous = self.selected_key();

        let sessions =
            crate::tmux::list_sessions().map_err(|e| TuiError::BackendFailure(e.to_string()))?;
        info!("reload: loaded {} tmux sessions", sessions.len());

        let contexts = crate::context::load_contexts()
            .map_err(|e| TuiError::ContextResolutionFailure(e.to_string()))?;
        info!("reload: loaded {} contexts", contexts.len());

        let panes =
            crate::tmux::list_panes().map_err(|e| TuiError::BackendFailure(e.to_string()))?;
        info!("reload: loaded {} panes", panes.len());

        self.sessions = build_sessions(sessions, contexts, panes);
        self.filtered_sessions = self.sessions.clone();
        self.rebuild_rows();
        self.apply_search_with(previous)?;
        info!(
            "tui reloaded {} sessions, {} rows",
            self.sessions.len(),
            self.rows.len()
        );
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let layout = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ]);
        let sections = layout.split(frame.area());
        self.render_search(frame, sections[0]);
        self.render_table(frame, sections[1]);
        self.render_footer(frame, sections[2]);
    }

    fn render_search(&self, frame: &mut Frame, area: Rect) {
        let (text, style) = if self.search.query.is_empty() {
            (
                "Search: ".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )
        } else {
            (format!("Search: {}", self.search.query), Style::default())
        };
        let search = Paragraph::new(Text::from(text)).style(style);
        frame.render_widget(search, area);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(["Session", "Status", "Context"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        // Calculate available width for context column
        // area.width - 2 (borders) - session_width - status_width - 2 (column spacing)
        let available_context_width = area
            .width
            .saturating_sub(2) // borders
            .saturating_sub(self.widths.session + 1)
            .saturating_sub(self.widths.status + 1)
            .saturating_sub(2); // column spacing

        let rows = self.rows.iter().enumerate().map(|(index, item)| {
            let mut base_style = if index % 2 == 0 {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
            if matches!(item, RowItem::Pane(_)) {
                base_style = base_style.add_modifier(Modifier::DIM);
            }
            Row::new(vec![
                Cell::from(self.row_label(item)),
                Cell::from(status_text(row_status(item))).style(status_style(row_status(item))),
                Cell::from(truncate_with_ellipsis(
                    &row_context(item),
                    available_context_width as usize,
                )),
            ])
            .style(base_style)
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(self.widths.session + 1),
                Constraint::Length(self.widths.status + 1),
                Constraint::Min(0),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let sections = Layout::horizontal([Constraint::Min(1), Constraint::Length(9)]).split(area);
        let footer = Paragraph::new(Text::from(INFO_TEXT));
        let mode = if matches!(self.mode, ListViewModes::SearchMode) {
            "[SEARCH]"
        } else {
            "[NORMAL]"
        };
        let mode_widget = Paragraph::new(Text::from(mode)).alignment(Alignment::Right);

        frame.render_widget(footer, sections[0]);
        frame.render_widget(mode_widget, sections[1]);
    }

    fn row_label(&self, item: &RowItem) -> String {
        match item {
            RowItem::Session(row) => self
                .session_index_by_id
                .get(&row.id)
                .map(|index| format!("{index}: {}", row.name))
                .unwrap_or_else(|| row.name.clone()),
            RowItem::Pane(row) => format!("  └─ {}", row.id),
        }
    }

    fn measure_widths(&self) -> ColumnWidths {
        let session = self
            .rows
            .iter()
            .map(|item| UnicodeWidthStr::width(self.row_label(item).as_str()))
            .max()
            .unwrap_or(0)
            .max(UnicodeWidthStr::width("Session"));
        let status = self
            .rows
            .iter()
            .map(|item| UnicodeWidthStr::width(status_text(row_status(item)).as_str()))
            .max()
            .unwrap_or(0)
            .max(UnicodeWidthStr::width("Status"));
        let context = self
            .rows
            .iter()
            .map(|item| UnicodeWidthStr::width(row_context(item).as_str()))
            .max()
            .unwrap_or(0)
            .max(UnicodeWidthStr::width("Context"));

        #[allow(clippy::cast_possible_truncation)]
        ColumnWidths {
            session: session as u16,
            status: status as u16,
            context: context as u16,
        }
    }

    fn select_first_session(&mut self) {
        if let Some(index) = self.rows.iter().position(|row| matches!(row, RowItem::Session(_)))
        {
            self.state.select(Some(index));
        }
    }

    fn select_last_session(&mut self) {
        if let Some(index) = self.rows.iter().rposition(|row| matches!(row, RowItem::Session(_)))
        {
            self.state.select(Some(index));
        }
    }

    fn jump_to_session_index(&mut self, index: usize) {
        let Some(session) = self.filtered_sessions.get(index) else {
            return;
        };
        if let Some(row_index) = self.rows.iter().position(|row| match row {
            RowItem::Session(row) => row.id == session.id,
            RowItem::Pane(_) => false,
        }) {
            self.state.select(Some(row_index));
        }
    }
}

struct PaneSelector {
    session_name: Option<String>,
    pane_id: String,
    options: Vec<(String, Option<crate::context::AgentStatus>)>,
    selected: usize,
}

impl PaneSelector {
    /// Create a new pane selector. Uses defaults when pane_id is empty or context cannot be loaded so the UI can always be shown.
    fn new(pane_id: String, session_name: Option<String>) -> Result<Self, TuiError> {
        let options = pane_status_options();
        let current = current_pane_status(&pane_id, session_name.as_deref()).unwrap_or(None);
        let selected = options
            .iter()
            .position(|(_, status)| *status == current)
            .unwrap_or(0);
        let pane_id = if pane_id.trim().is_empty() {
            "(unknown)".to_string()
        } else {
            pane_id
        };
        Ok(Self {
            session_name,
            pane_id,
            options,
            selected,
        })
    }

    /// Run the pane selector event loop
    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), TuiError> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if self.selected == 0 {
                            self.selected = self.options.len() - 1;
                        } else {
                            self.selected -= 1;
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        self.selected = (self.selected + 1) % self.options.len();
                    }
                    KeyCode::Enter => {
                        let status = self.options[self.selected].1.clone();
                        // BUG: Handle the case where the session name is not provided
                        // and there we haven't saved anything for the session yet
                        if let Some(ref session_name) = self.session_name {
                            crate::context::upsert_pane(
                                session_name,
                                &self.pane_id,
                                None,
                                status,
                                None,
                            )
                            .map_err(|e| TuiError::ContextResolutionFailure(e.to_string()))?;
                        }
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }

    /// Draw the pane selector UI
    fn draw(&self, frame: &mut Frame) {
        let area = centered_rect(60, 20, frame.area());
        let spans = self
            .options
            .iter()
            .enumerate()
            .map(|(index, (label, _))| {
                let style = if index == self.selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Span::styled(format!(" {label} "), style)
            })
            .collect::<Vec<_>>();
        let line = Line::from(spans);
        let pane_title = format!("Pane {}", self.pane_id);
        let paragraph = Paragraph::new(line)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(pane_title));

        frame.render_widget(Clear, area);
        frame.render_widget(paragraph, area);
    }
}

fn pane_status_options() -> Vec<(String, Option<crate::context::AgentStatus>)> {
    vec![
        (
            "working".to_string(),
            Some(crate::context::AgentStatus::Working),
        ),
        (
            "waiting".to_string(),
            Some(crate::context::AgentStatus::Waiting),
        ),
        ("done".to_string(), Some(crate::context::AgentStatus::Done)),
        ("none".to_string(), Some(crate::context::AgentStatus::None)),
    ]
}

fn current_pane_status(
    pane_id: &str,
    session_name: Option<&str>,
) -> Result<Option<crate::context::AgentStatus>, TuiError> {
    let contexts = crate::context::load_contexts()
        .map_err(|e| TuiError::ContextResolutionFailure(e.to_string()))?;

    if let Some(session_name) = session_name {
        return Ok(contexts
            .get(&crate::context::session_key(session_name))
            .and_then(|session| session.panes.get(pane_id))
            .and_then(|pane| pane.pane_status.clone()));
    }

    Ok(contexts.iter().find_map(|session_context| {
        session_context
            .1
            .panes
            .get(pane_id)
            .and_then(|pane_context| pane_context.pane_status.clone())
    }))
}

fn centered_rect(percent_x: u16, percent_y: u16, rect: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(rect);
    let horizontal = Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1]);
    horizontal[1]
}

fn run_fzf_filter(query: &str, candidates: &[String]) -> Result<String, TuiError> {
    let mut child = Command::new("fzf")
        .args(["--filter", query, "--print-query", "--reverse"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        for line in candidates {
            writeln!(stdin, "{line}")?;
        }
    }

    let output = child.wait_with_output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(TuiError::BackendFailure(message));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn build_sessions(
    sessions: Vec<crate::tmux::TmuxSession>,
    contexts: HashMap<String, crate::context::SessionContext>,
    panes: Vec<crate::tmux::TmuxPane>,
) -> Vec<SessionRow> {
    info!(
        "build_sessions: {} tmux sessions, {} contexts, {} panes",
        sessions.len(),
        contexts.len(),
        panes.len()
    );
    debug!(
        "available context keys: {:?}",
        contexts.keys().collect::<Vec<_>>()
    );

    let mut panes_by_session: HashMap<String, Vec<String>> = HashMap::new();
    for pane in panes {
        panes_by_session
            .entry(pane.session_name)
            .or_default()
            .push(pane.pane_id);
    }

    let built: Vec<SessionRow> = sessions
        .into_iter()
        .map(|session| {
            let key = crate::context::session_key(&session.name);
            debug!("building session: {:?} computed_key={}", session, key);

            let context = contexts.get(&key);
            if let Some(ctx) = context {
                debug!("  ✓ context found: {:?}", ctx);
            } else {
                debug!("  ✗ no context found for session={}", session.name);
            }

            let status = context.and_then(|ctx| ctx.session_status.clone());
            let context_value =
                normalize_field(context.and_then(|ctx| ctx.session_context.as_ref()));

            let mut pane_rows = panes_by_session
                .get(&session.name)
                .cloned()
                .unwrap_or_default();
            pane_rows.sort();
            let panes = pane_rows
                .into_iter()
                .map(|pane_id| {
                    let pane_ctx = context.and_then(|ctx| ctx.panes.get(&pane_id));
                    let pane_status = pane_ctx.and_then(|pane| pane.pane_status.clone());
                    let pane_context_value =
                        normalize_field(pane_ctx.and_then(|pane| pane.pane_context.as_ref()));
                    PaneRow {
                        id: pane_id,
                        alias: None,
                        status: pane_status,
                        context: pane_context_value,
                        session_id: session.id.clone(),
                    }
                })
                .collect();
            SessionRow {
                id: session.id,
                name: session.name,
                status,
                context: context_value,
                panes,
            }
        })
        .collect();
    info!(
        "built {} session rows from {} contexts and {} pane groups",
        built.len(),
        contexts.len(),
        panes_by_session.len()
    );
    built
}

fn collect_live_panes(panes: &[crate::tmux::TmuxPane]) -> HashMap<String, HashSet<String>> {
    let mut live = HashMap::new();
    for pane in panes {
        live.entry(pane.session_name.clone())
            .or_insert_with(HashSet::new)
            .insert(pane.pane_id.clone());
    }
    live
}

fn row_status(item: &RowItem) -> Option<&crate::context::AgentStatus> {
    match item {
        RowItem::Session(row) => row.status.as_ref(),
        RowItem::Pane(row) => row.status.as_ref(),
    }
}

fn row_context(item: &RowItem) -> String {
    match item {
        RowItem::Session(row) => row.context.clone(),
        RowItem::Pane(row) => row.context.clone(),
    }
}

fn normalize_field(value: Option<&String>) -> String {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| DATA_NOT_RECEIVED.to_string())
}

fn truncate_with_ellipsis(s: &str, max_width: usize) -> String {
    const ELLIPSIS: &str = "...";

    if UnicodeWidthStr::width(s) <= max_width {
        return s.to_string();
    }
    if max_width <= 3 {
        return ELLIPSIS.to_string();
    }
    let mut w = 0;
    for (i, c) in s.char_indices() {
        w += c.width().unwrap_or(1);
        if w > max_width - 3 {
            return format!("{}{}", &s[..i], ELLIPSIS);
        }
    }
    s.to_string()
}

fn status_text(status: Option<&crate::context::AgentStatus>) -> String {
    status
        .map(|status| status.to_string())
        .unwrap_or_else(|| DATA_NOT_RECEIVED.to_string())
}

fn status_style(status: Option<&crate::context::AgentStatus>) -> Style {
    match status {
        Some(crate::context::AgentStatus::Done) => Style::default().fg(Color::Green),
        Some(crate::context::AgentStatus::None) => Style::default().fg(Color::Gray),
        Some(crate::context::AgentStatus::Working) => Style::default().fg(Color::Blue),
        Some(crate::context::AgentStatus::Waiting) => Style::default().fg(Color::Yellow),
        None => Style::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{session_key, AgentStatus, PaneContext, SessionContext};
    use crate::tmux::{TmuxPane, TmuxSession};
    use std::collections::HashMap;

    fn session_row(id: &str, name: &str) -> SessionRow {
        SessionRow {
            id: id.to_string(),
            name: name.to_string(),
            status: None,
            context: "ctx".to_string(),
            panes: Vec::new(),
        }
    }

    #[test]
    fn normalize_field_trims_and_defaults() {
        let value = "  hello  ".to_string();
        assert_eq!(normalize_field(Some(&value)), "hello");

        let empty = "   ".to_string();
        assert_eq!(normalize_field(Some(&empty)), DATA_NOT_RECEIVED);
        assert_eq!(normalize_field(None), DATA_NOT_RECEIVED);
    }

    #[test]
    fn truncate_with_ellipsis_handles_bounds() {
        assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
        assert_eq!(truncate_with_ellipsis("hello", 4), "h...");
        assert_eq!(truncate_with_ellipsis("hello", 3), "...");
        assert_eq!(truncate_with_ellipsis("hello", 2), "...");
    }

    #[test]
    fn status_text_uses_default_for_missing() {
        assert_eq!(status_text(Some(&AgentStatus::Done)), "done");
        assert_eq!(status_text(None), DATA_NOT_RECEIVED);
    }

    #[test]
    fn collect_live_panes_groups_by_session() {
        let panes = vec![
            TmuxPane {
                session_name: "alpha".to_string(),
                pane_id: "%1".to_string(),
            },
            TmuxPane {
                session_name: "alpha".to_string(),
                pane_id: "%2".to_string(),
            },
            TmuxPane {
                session_name: "beta".to_string(),
                pane_id: "%3".to_string(),
            },
        ];
        let map = collect_live_panes(&panes);
        assert_eq!(map.get("alpha").map(|set| set.len()), Some(2));
        assert!(map
            .get("alpha")
            .expect("alpha set")
            .contains("%1"));
        assert!(map.get("beta").expect("beta set").contains("%3"));
    }

    #[test]
    fn build_sessions_merges_context_and_sorts_panes() {
        let sessions = vec![
            TmuxSession {
                id: "@1".to_string(),
                name: "alpha".to_string(),
            },
            TmuxSession {
                id: "@2".to_string(),
                name: "beta".to_string(),
            },
        ];

        let mut pane_map = HashMap::new();
        pane_map.insert(
            "%2".to_string(),
            PaneContext {
                pane_id: None,
                pane_name: None,
                pane_status: Some(AgentStatus::Done),
                pane_context: Some("pctx".to_string()),
            },
        );

        let mut contexts = HashMap::new();
        contexts.insert(
            session_key("alpha"),
            SessionContext {
                session_name: Some("alpha".to_string()),
                session_id: None,
                session_status: Some(AgentStatus::Working),
                session_context: Some("ctx".to_string()),
                panes: pane_map,
            },
        );

        let panes = vec![
            TmuxPane {
                session_name: "alpha".to_string(),
                pane_id: "%2".to_string(),
            },
            TmuxPane {
                session_name: "alpha".to_string(),
                pane_id: "%1".to_string(),
            },
        ];

        let built = build_sessions(sessions, contexts, panes);
        let alpha = built
            .iter()
            .find(|row| row.name == "alpha")
            .expect("alpha row");
        assert_eq!(alpha.status, Some(AgentStatus::Working));
        assert_eq!(alpha.context, "ctx");
        assert_eq!(alpha.panes.len(), 2);
        assert_eq!(alpha.panes[0].id, "%1");
        assert_eq!(alpha.panes[0].context, DATA_NOT_RECEIVED);
        assert_eq!(alpha.panes[1].id, "%2");
        assert_eq!(alpha.panes[1].status, Some(AgentStatus::Done));
        assert_eq!(alpha.panes[1].context, "pctx");
    }

    #[test]
    fn apply_search_with_uses_injected_filter() {
        let sessions = vec![session_row("@1", "one"), session_row("@2", "two")];
        let filter = Box::new(|query: &str, candidates: &[String]| {
            assert_eq!(query, "one");
            Ok(format!("{query}\n{}\n", candidates[0]))
        });
        let mut app = App::new_with_filter(sessions, filter).expect("app");
        app.search.query = "one".to_string();

        app.apply_search_with(None).expect("search");

        assert_eq!(app.filtered_sessions.len(), 1);
        assert_eq!(app.filtered_sessions[0].id, "@1");
    }
}
