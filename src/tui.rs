use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap};
use ratatui::{DefaultTerminal, Frame};
use std::collections::{HashMap, HashSet};
use std::io;
use thiserror::Error;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use log::{debug, error, info};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config as NucleoConfig, Matcher as NucleoMatcher};

const DATA_NOT_RECEIVED: &str = "-";
// TODO: Will make this configurable in the future.
const PANE_LABEL_MAX_WIDTH: usize = 10;
const PREVIEW_CAPTURE_LINES: usize = 120;
const PREVIEW_DEFAULT_TEXT: &str =
    "Preview is off. Press 'p' to toggle live pane preview for the selected row.";
const INFO_TEXT: &str = "(?) help | (Esc/Ctrl+C) back/quit | (/) search | (Enter) switch | (↑/↓) move | (g/G) top/bottom | (0-9/Opt-a..z) jump | (l/h) expand/collapse | (L) toggle all | (p) preview | (x) delete selected | (r) refresh";
const HELP_FEEDBACK_TEXT: &str =
    "Feedback: create an issue at https://github.com/cruzluna/jkl-2/issues";
const HELP_BINDINGS: [(&str, &str, &str); 20] = [
    ("Normal", "q / Esc / Ctrl-C", "Quit list view"),
    ("Normal", "?", "Open this help"),
    ("Normal", "/", "Search sessions/windows/panes"),
    ("Normal", "Enter", "Switch to selected row"),
    ("Normal", "↑/↓ or j/k", "Move selection"),
    ("Normal", "g / G", "Jump to top/bottom session"),
    ("Normal", "0-9", "Jump to session index 0-9"),
    ("Normal", "Opt-a..z", "Jump to session index 10-35"),
    ("Normal", "l / h", "Expand/collapse selected session"),
    ("Normal", "L", "Toggle all sessions expanded/collapsed"),
    ("Normal", "p", "Toggle live pane preview"),
    ("Normal", "x", "Start delete confirmation"),
    ("Normal", "r", "Refresh pane list"),
    ("Search", "type / Backspace", "Filter sessions"),
    ("Search", "↑/↓", "Move results"),
    ("Search", "Esc / Ctrl-C", "Return to list"),
    ("Search", "Enter", "Switch to selected result"),
    ("Command", "x", "Confirm delete"),
    ("Command", "any other key", "Cancel delete"),
    ("Help", "q / Q / Esc / Ctrl-C", "Return to list"),
];

#[derive(Error, Debug)]
pub enum TuiError {
    #[error("{0}")]
    BackendFailure(String),
    #[error("{0}")]
    ContextResolutionFailure(String),
    #[error("Search failure: {0}")]
    SearchFailure(String),
    #[error("Terminal I/O error: {0}")]
    TerminalIo(#[from] io::Error),
}

fn backend_failure(source: impl std::fmt::Display) -> TuiError {
    let message = source.to_string();
    error!("tui backend failure: {message}");
    TuiError::BackendFailure(message)
}

fn context_resolution_failure(source: impl std::fmt::Display) -> TuiError {
    let message = source.to_string();
    error!("tui context resolution failure: {message}");
    TuiError::ContextResolutionFailure(message)
}

fn search_failure(message: String) -> TuiError {
    error!("tui search failure: {message}");
    TuiError::SearchFailure(message)
}

pub fn run() -> Result<(), TuiError> {
    info!("tui starting");
    let sessions = crate::tmux::list_sessions().map_err(backend_failure)?;
    info!(
        "tui loaded {} tmux sessions: {:?}",
        sessions.len(),
        sessions
    );

    let contexts = crate::context::load_contexts().map_err(context_resolution_failure)?;
    info!("tui loaded {} contexts", contexts.len());

    let panes = crate::tmux::list_panes().map_err(backend_failure)?;
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
    /// The windows belonging to the session
    windows: Vec<WindowRow>,
}

#[derive(Clone)]
struct WindowRow {
    /// The tmux window ID
    id: String,
    /// The tmux window name
    name: String,
    /// The status of the window
    status: Option<crate::context::AgentStatus>,
    /// The context of the window
    context: String,
    /// The panes belonging to the window
    panes: Vec<PaneRow>,
    /// The session ID
    session_id: String,
}

#[derive(Clone)]
struct PaneRow {
    /// The pane ID
    id: String,
    /// The tmux window ID this pane belongs to
    window_id: String,
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
    Window(WindowRow),
    Pane(PaneRow),
}

#[derive(Clone, PartialEq, Eq)]
enum RowKey {
    Session(String),
    Window {
        session_id: String,
        window_id: String,
    },
    Pane {
        session_id: String,
        pane_id: String,
    },
}

enum CommandMode {
    DeleteConfirm(RowKey),
}

impl RowItem {
    fn key(&self) -> RowKey {
        match self {
            RowItem::Session(row) => RowKey::Session(row.id.clone()),
            RowItem::Window(row) => RowKey::Window {
                session_id: row.session_id.clone(),
                window_id: row.id.clone(),
            },
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
    /// Mode when showing keybinding help
    HelpMode,
    /// Mode for command confirmations (e.g. delete).
    CommandMode(CommandMode),
}

struct SearchQuery {
    query: String,
    // TODO: cursor position
}

/// Column widths for the session table (Session, Status, Context).
struct ColumnWidths {
    session: u16,
    status: u16,
}

/// Concrete column widths used while rendering the table for a specific frame area.
#[derive(Debug, PartialEq, Eq)]
struct TableColumnWidths {
    session: u16,
    status: u16,
    context: u16,
}

#[derive(Clone, Debug)]
struct SearchCandidate {
    id: String,
    search_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PreviewTarget {
    pane_id: String,
}

impl AsRef<str> for SearchCandidate {
    fn as_ref(&self) -> &str {
        &self.search_text
    }
}

trait SessionSearch {
    fn filter_ids(
        &mut self,
        query: &str,
        candidates: &[SearchCandidate],
    ) -> Result<Vec<String>, TuiError>;
}

#[derive(Debug)]
struct NucleoSessionSearch {
    matcher: NucleoMatcher,
}

impl NucleoSessionSearch {
    fn new() -> Self {
        Self {
            matcher: NucleoMatcher::new(NucleoConfig::DEFAULT),
        }
    }
}

impl SessionSearch for NucleoSessionSearch {
    fn filter_ids(
        &mut self,
        query: &str,
        candidates: &[SearchCandidate],
    ) -> Result<Vec<String>, TuiError> {
        let query_codepoints = query.chars().count();
        if query_codepoints > u32::MAX as usize {
            let message = format!(
                "search query exceeds matcher length limit ({query_codepoints} code points)"
            );
            return Err(search_failure(message));
        }

        if let Some(too_long) = candidates
            .iter()
            .find(|candidate| candidate.search_text.chars().count() > u32::MAX as usize)
        {
            let message = format!(
                "search candidate {} exceeds matcher length limit",
                too_long.id
            );
            return Err(search_failure(message));
        }

        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let matches = pattern.match_list(candidates.iter(), &mut self.matcher);
        debug!(
            "nucleo matched {} of {} candidates for query={query:?}",
            matches.len(),
            candidates.len()
        );

        Ok(matches
            .into_iter()
            .map(|(candidate, _score)| candidate.id.clone())
            .collect())
    }
}

struct App<S: SessionSearch> {
    state: TableState,
    sessions: Vec<SessionRow>,
    search_candidates: Vec<SearchCandidate>,
    filtered_sessions: Vec<SessionRow>,
    rows: Vec<RowItem>,
    session_index_by_id: HashMap<String, usize>,
    widths: ColumnWidths,
    search: SearchQuery,
    mode: ListViewModes,
    expanded_sessions: HashSet<String>,
    preview_enabled: bool,
    preview_target_pane_id: Option<String>,
    preview_title: String,
    preview_text: String,
    filter: S,
}

impl App<NucleoSessionSearch> {
    /// Creates a list view of sessions and panes
    fn new(sessions: Vec<SessionRow>) -> Result<Self, TuiError> {
        Self::new_with_filter(sessions, NucleoSessionSearch::new())
    }
}

impl<S: SessionSearch> App<S> {
    /// Creates a list view with an injected search backend.
    ///
    /// This keeps search behavior testable without heap allocation or dynamic dispatch.
    fn new_with_filter(sessions: Vec<SessionRow>, filter: S) -> Result<Self, TuiError> {
        let search_candidates = build_search_candidates(&sessions);
        let mut app = Self {
            state: TableState::default(),
            filtered_sessions: sessions.clone(),
            sessions,
            search_candidates,
            rows: Vec::new(),
            session_index_by_id: HashMap::new(),
            widths: ColumnWidths {
                session: 0,
                status: 0,
            },
            search: SearchQuery {
                query: String::new(),
            },
            mode: ListViewModes::NormalMode,
            expanded_sessions: HashSet::new(),
            preview_enabled: false,
            preview_target_pane_id: None,
            preview_title: "Pane Preview".to_string(),
            preview_text: PREVIEW_DEFAULT_TEXT.to_string(),
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

                if matches!(self.mode, ListViewModes::HelpMode) {
                    match key.code {
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                            self.mode = ListViewModes::NormalMode;
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.mode = ListViewModes::NormalMode;
                        }
                        _ => {}
                    }
                } else if matches!(self.mode, ListViewModes::SearchMode) {
                    match key.code {
                        KeyCode::Esc => {
                            self.mode = ListViewModes::NormalMode;
                        }
                        KeyCode::Enter => {
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
                        }
                        // Append a character to the search query
                        KeyCode::Char(c) => {
                            self.search.query.push(c);
                            self.apply_search()?;
                        }
                        _ => {}
                    }
                } else if matches!(self.mode, ListViewModes::CommandMode(_)) {
                    self.handle_command_mode_key(key.code)?;
                } else {
                    // Normal mode keybindings
                    if let KeyCode::Char(c) = key.code
                        && key.modifiers.contains(KeyModifiers::ALT)
                        && let Some(index) = meta_jump_index(c)
                    {
                        self.jump_to_session_index(index);
                        continue;
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
                        KeyCode::Char('?') => {
                            self.mode = ListViewModes::HelpMode;
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
                        KeyCode::Char('L') => self.expand_all(),
                        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                            self.expand_all();
                        }
                        KeyCode::Char('l') => self.expand_selected(),
                        KeyCode::Char('h') => self.collapse_selected(),
                        KeyCode::Char('p') => self.toggle_preview(),
                        KeyCode::Char('x') => self.enter_delete_mode(),
                        KeyCode::Char('r') => {
                            info!("tui refresh requested");
                            self.refresh_panes()?;
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            let index = c.to_digit(10).unwrap_or(0) as usize;
                            self.jump_to_session_index(index);
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
        let previous = if self.search.query.trim().is_empty() {
            self.selected_key()
        } else {
            None
        };
        self.apply_search_with(previous)
    }

    fn apply_search_with(&mut self, previous: Option<RowKey>) -> Result<(), TuiError> {
        if self.search.query.trim().is_empty() {
            self.filtered_sessions = self.sessions.clone();
            self.rebuild_rows();
            self.restore_selection(previous);
            return Ok(());
        }

        debug!(
            "tui applying search query={:?} against {} sessions",
            self.search.query,
            self.search_candidates.len()
        );
        let matched_ids = match self
            .filter
            .filter_ids(&self.search.query, &self.search_candidates)
        {
            Ok(ids) => ids,
            Err(err) => {
                error!("tui search failed for query={:?}: {err}", self.search.query);
                return Err(err);
            }
        };

        let lookup: HashMap<&str, &SessionRow> = self
            .sessions
            .iter()
            .map(|row| (row.id.as_str(), row))
            .collect();
        let mut filtered = Vec::new();
        for id in matched_ids {
            if let Some(row) = lookup.get(id.as_str()) {
                filtered.push((*row).clone());
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
                for window in &session.windows {
                    rows.push(RowItem::Window(window.clone()));
                    for pane in &window.panes {
                        rows.push(RowItem::Pane(pane.clone()));
                    }
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
            match row {
                RowItem::Session(session) => {
                    info!("tui switching to session_id={}", session.id);
                    crate::tmux::switch_client(&session.id).map_err(backend_failure)?;
                }
                RowItem::Window(window) => {
                    info!("tui switching to session_id={}", window.session_id);
                    crate::tmux::switch_client(&window.session_id).map_err(backend_failure)?;
                    crate::tmux::select_window(&window.id).map_err(backend_failure)?;
                }
                RowItem::Pane(pane) => {
                    info!(
                        "tui switching to pane_id={} in session_id={} window_id={}",
                        pane.id, pane.session_id, pane.window_id
                    );
                    crate::tmux::switch_client(&pane.session_id).map_err(backend_failure)?;
                    crate::tmux::select_window(&pane.window_id).map_err(backend_failure)?;
                    crate::tmux::select_pane(&pane.id).map_err(backend_failure)?;
                }
            }
        }
        Ok(())
    }

    fn enter_delete_mode(&mut self) {
        if let Some(key) = self.selected_key() {
            self.mode = ListViewModes::CommandMode(CommandMode::DeleteConfirm(key));
        }
    }

    fn handle_command_mode_key(&mut self, key_code: KeyCode) -> Result<(), TuiError> {
        if !matches!(key_code, KeyCode::Char('x')) {
            self.mode = ListViewModes::NormalMode;
            return Ok(());
        }

        let target = match &self.mode {
            ListViewModes::CommandMode(CommandMode::DeleteConfirm(target)) => Some(target.clone()),
            ListViewModes::NormalMode | ListViewModes::SearchMode | ListViewModes::HelpMode => None,
        };
        self.mode = ListViewModes::NormalMode;

        if let Some(target) = target {
            self.delete_row(&target)?;
        }
        Ok(())
    }

    fn delete_row(&mut self, target: &RowKey) -> Result<(), TuiError> {
        match target {
            RowKey::Session(session_id) => {
                info!("tui deleting session_id={}", session_id);
                crate::tmux::kill_session(session_id).map_err(backend_failure)?;
            }
            RowKey::Window { window_id, .. } => {
                info!("tui deleting window_id={}", window_id);
                crate::tmux::kill_window(window_id).map_err(backend_failure)?;
            }
            RowKey::Pane { pane_id, .. } => {
                info!("tui deleting pane_id={}", pane_id);
                crate::tmux::kill_pane(pane_id).map_err(backend_failure)?;
            }
        }
        self.reload_data()?;
        Ok(())
    }

    fn delete_prompt_text(&self) -> Option<String> {
        let target = match &self.mode {
            ListViewModes::CommandMode(CommandMode::DeleteConfirm(target)) => target,
            ListViewModes::NormalMode | ListViewModes::SearchMode | ListViewModes::HelpMode => {
                return None;
            }
        };

        let subject = match target {
            RowKey::Session(session_id) => format!("session {session_id}"),
            RowKey::Window { window_id, .. } => format!("window {window_id}"),
            RowKey::Pane { pane_id, .. } => format!("pane {pane_id}"),
        };

        Some(format!(
            "Delete {subject}? Press x again to confirm; any other key cancels."
        ))
    }

    fn expand_selected(&mut self) {
        let previous = self.selected_key();
        let session_id = self.selected_row().map(|row| match row {
            RowItem::Session(session) => session.id.clone(),
            RowItem::Window(window) => window.session_id.clone(),
            RowItem::Pane(pane) => pane.session_id.clone(),
        });
        if let Some(session_id) = session_id {
            self.expanded_sessions.insert(session_id);
            self.rebuild_rows();
            self.restore_selection(previous);
        }
    }

    fn expand_all(&mut self) {
        let previous = self.selected_key();
        let all_expanded = !self.sessions.is_empty()
            && self
                .sessions
                .iter()
                .all(|session| self.expanded_sessions.contains(&session.id));
        if all_expanded {
            self.expanded_sessions.clear();
        } else {
            self.expanded_sessions = self
                .sessions
                .iter()
                .map(|session| session.id.clone())
                .collect();
        }
        self.rebuild_rows();
        self.restore_selection(previous);
    }

    fn collapse_selected(&mut self) {
        let previous = self.selected_key();
        let session_id = self.selected_row().map(|row| match row {
            RowItem::Session(session) => session.id.clone(),
            RowItem::Window(window) => window.session_id.clone(),
            RowItem::Pane(pane) => pane.session_id.clone(),
        });
        if let Some(session_id) = session_id {
            self.expanded_sessions.remove(&session_id);
            self.rebuild_rows();
            self.restore_selection(previous);
        }
    }

    fn refresh_panes(&mut self) -> Result<(), TuiError> {
        let live_panes = crate::tmux::list_panes().map_err(backend_failure)?;
        let live_map = collect_live_panes(&live_panes);
        crate::context::prune_panes(&live_map).map_err(context_resolution_failure)?;
        self.reload_data()?;
        Ok(())
    }

    fn reload_data(&mut self) -> Result<(), TuiError> {
        info!("tui reload_data starting");
        let previous = self.selected_key();

        let sessions = crate::tmux::list_sessions().map_err(backend_failure)?;
        info!("reload: loaded {} tmux sessions", sessions.len());

        let contexts = crate::context::load_contexts().map_err(context_resolution_failure)?;
        info!("reload: loaded {} contexts", contexts.len());

        let panes = crate::tmux::list_panes().map_err(backend_failure)?;
        info!("reload: loaded {} panes", panes.len());

        self.sessions = build_sessions(sessions, contexts, panes);
        self.search_candidates = build_search_candidates(&self.sessions);
        self.filtered_sessions = self.sessions.clone();
        self.rebuild_rows();
        self.apply_search_with(previous)?;
        if self.preview_enabled {
            self.sync_preview_to_selection(true);
        }
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
        if self.preview_enabled {
            self.sync_preview_to_selection(false);
            let split =
                Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)])
                    .split(sections[1]);
            self.render_table(frame, split[0]);
            self.render_preview(frame, split[1]);
        } else {
            self.render_table(frame, sections[1]);
        }
        self.render_footer(frame, sections[2]);
        if matches!(self.mode, ListViewModes::HelpMode) {
            self.render_help_overlay(frame);
        }
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
        let widths = self.table_column_widths(area);

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
                Cell::from(truncate_with_ellipsis(
                    &self.row_label(item),
                    widths.session as usize,
                )),
                Cell::from(status_text(row_status(item))).style(status_style(row_status(item))),
                Cell::from(truncate_with_ellipsis(
                    &row_context(item),
                    widths.context as usize,
                )),
            ])
            .style(base_style)
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(widths.session),
                Constraint::Length(widths.status),
                Constraint::Length(widths.context),
            ],
        )
        .header(header)
        .block(Block::default().borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let sections = Layout::horizontal([Constraint::Min(1), Constraint::Length(9)]).split(area);
        let footer_text = if matches!(self.mode, ListViewModes::HelpMode) {
            "Help: q/Q/Esc/Ctrl+C back to list".to_string()
        } else {
            self.delete_prompt_text()
                .unwrap_or_else(|| INFO_TEXT.to_string())
        };
        let footer = Paragraph::new(Text::from(footer_text));
        let mode = match &self.mode {
            ListViewModes::SearchMode => "[SEARCH]",
            ListViewModes::HelpMode => "[HELP]",
            ListViewModes::CommandMode(_) => "[COMMAND]",
            ListViewModes::NormalMode => "[NORMAL]",
        };
        let mode_widget = Paragraph::new(Text::from(mode)).alignment(Alignment::Right);

        frame.render_widget(footer, sections[0]);
        frame.render_widget(mode_widget, sections[1]);
    }

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let preview = Paragraph::new(Text::from(self.preview_text.clone()))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(&self.preview_title),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(preview, area);
    }

    fn render_help_overlay(&self, frame: &mut Frame) {
        let popup_area = frame.area();
        let width = popup_area
            .width
            .saturating_mul(92)
            .saturating_div(100)
            .max(1);
        let height = popup_area
            .height
            .saturating_mul(80)
            .saturating_div(100)
            .max(1);
        let area = centered_rect_size(width, height, popup_area);
        frame.render_widget(Clear, area);

        let block = Block::default().borders(Borders::ALL).title("Key Bindings");
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let sections = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

        let mode_width = HELP_BINDINGS
            .iter()
            .map(|(mode, _, _)| UnicodeWidthStr::width(*mode))
            .max()
            .unwrap_or(0)
            .max(UnicodeWidthStr::width("Mode"));
        let key_width = HELP_BINDINGS
            .iter()
            .map(|(_, keys, _)| UnicodeWidthStr::width(*keys))
            .max()
            .unwrap_or(0)
            .max(UnicodeWidthStr::width("Keys"));

        #[allow(clippy::cast_possible_truncation)]
        let constraints = [
            Constraint::Length(mode_width as u16 + 1),
            Constraint::Length(key_width as u16 + 1),
            Constraint::Min(1),
        ];

        let rows = HELP_BINDINGS
            .iter()
            .map(|(mode, keys, action)| Row::new([*mode, *keys, *action]));

        let table = Table::new(rows, constraints)
            .header(
                Row::new(["Mode", "Keys", "Action"])
                    .style(Style::default().add_modifier(Modifier::BOLD)),
            )
            .column_spacing(1);
        frame.render_widget(table, sections[0]);

        let feedback = Paragraph::new(Text::from(HELP_FEEDBACK_TEXT))
            .style(Style::default().add_modifier(Modifier::DIM));
        frame.render_widget(feedback, sections[1]);
    }

    fn row_label(&self, item: &RowItem) -> String {
        match item {
            RowItem::Session(row) => self
                .session_index_by_id
                .get(&row.id)
                .map(|index| format!("{}: {}", session_shortcut_label(*index), row.name))
                .unwrap_or_else(|| row.name.clone()),
            RowItem::Window(row) => format!("  ◦ {}", row.name),
            RowItem::Pane(row) => format!("    └─ {}", pane_label(row)),
        }
    }

    fn table_column_widths(&self, area: Rect) -> TableColumnWidths {
        #[allow(clippy::cast_possible_truncation)]
        let session_header_width = UnicodeWidthStr::width("Session") as u16;
        #[allow(clippy::cast_possible_truncation)]
        let status_header_width = UnicodeWidthStr::width("Status") as u16;
        #[allow(clippy::cast_possible_truncation)]
        let context_header_width = UnicodeWidthStr::width("Context") as u16;

        // Account for table borders and the default one-cell spacing between three columns.
        let inner_width = area.width.saturating_sub(2);
        let usable_width = inner_width.saturating_sub(2);
        if usable_width == 0 {
            return TableColumnWidths {
                session: 0,
                status: 0,
                context: 0,
            };
        }

        let status_width = self
            .widths
            .status
            .max(status_header_width)
            .min(usable_width);
        let remaining_width = usable_width.saturating_sub(status_width);
        if remaining_width == 0 {
            return TableColumnWidths {
                session: 0,
                status: status_width,
                context: 0,
            };
        }

        // Keep session constrained so context remains visible on medium terminal widths.
        let max_session_by_ratio = remaining_width.saturating_mul(45).saturating_div(100);
        let min_session_width = if remaining_width > 1 {
            session_header_width.min(remaining_width - 1)
        } else {
            0
        };
        let mut session_width = self
            .widths
            .session
            .max(session_header_width)
            .min(max_session_by_ratio.max(min_session_width));

        let mut context_width = remaining_width.saturating_sub(session_width);
        let min_context_width = if remaining_width > min_session_width {
            context_header_width.min(remaining_width - min_session_width)
        } else {
            0
        };
        if context_width < min_context_width {
            let deficit = min_context_width - context_width;
            session_width = session_width.saturating_sub(deficit);
            context_width = remaining_width.saturating_sub(session_width);
        }
        if context_width == 0 && remaining_width > 0 {
            context_width = 1;
            session_width = remaining_width.saturating_sub(context_width);
        }

        TableColumnWidths {
            session: session_width,
            status: status_width,
            context: context_width,
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
        #[allow(clippy::cast_possible_truncation)]
        ColumnWidths {
            session: session as u16,
            status: status as u16,
        }
    }

    fn select_first_session(&mut self) {
        if let Some(index) = self
            .rows
            .iter()
            .position(|row| matches!(row, RowItem::Session(_)))
        {
            self.state.select(Some(index));
        }
    }

    fn select_last_session(&mut self) {
        if let Some(index) = self
            .rows
            .iter()
            .rposition(|row| matches!(row, RowItem::Session(_)))
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
            RowItem::Window(_) | RowItem::Pane(_) => false,
        }) {
            self.state.select(Some(row_index));
        }
    }

    fn toggle_preview(&mut self) {
        self.preview_enabled = !self.preview_enabled;
        if self.preview_enabled {
            self.sync_preview_to_selection(true);
        } else {
            self.preview_target_pane_id = None;
            self.preview_title = "Pane Preview".to_string();
            self.preview_text = PREVIEW_DEFAULT_TEXT.to_string();
        }
    }

    fn sync_preview_to_selection(&mut self, force: bool) {
        if !self.preview_enabled {
            return;
        }

        let target = self.preview_target_for_selected_row();
        let target_id = target.as_ref().map(|target| target.pane_id.clone());
        if !force && self.preview_target_pane_id == target_id {
            return;
        }

        self.preview_target_pane_id = target_id.clone();

        let Some(target) = target else {
            self.preview_title = "Pane Preview".to_string();
            self.preview_text = "No pane available for the current selection.".to_string();
            return;
        };

        self.preview_title = format!("Pane {}", target.pane_id);
        match crate::tmux::capture_pane(&target.pane_id, PREVIEW_CAPTURE_LINES) {
            Ok(text) => {
                self.preview_text = if text.trim().is_empty() {
                    "(pane is empty)".to_string()
                } else {
                    text
                };
            }
            Err(error) => {
                error!(
                    "tui preview capture failed pane_id={} error={error}",
                    target.pane_id
                );
                self.preview_text =
                    format!("Unable to capture pane {}.\n{}", target.pane_id, error);
            }
        }
    }

    fn preview_target_for_selected_row(&self) -> Option<PreviewTarget> {
        match self.selected_row() {
            Some(RowItem::Pane(row)) => Some(PreviewTarget {
                pane_id: row.id.clone(),
            }),
            Some(RowItem::Window(row)) => row.panes.first().map(|pane| PreviewTarget {
                pane_id: pane.id.clone(),
            }),
            Some(RowItem::Session(row)) => row
                .windows
                .iter()
                .find_map(|window| window.panes.first())
                .map(|pane| PreviewTarget {
                    pane_id: pane.id.clone(),
                }),
            None => None,
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
                    KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => {
                        if self.selected == 0 {
                            self.selected = self.options.len() - 1;
                        } else {
                            self.selected -= 1;
                        }
                    }
                    KeyCode::Right | KeyCode::Down | KeyCode::Char('l') | KeyCode::Char('j') => {
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
                                None,
                                None,
                                status,
                                None,
                            )
                            .map_err(context_resolution_failure)?;
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
        let popup_area = frame.area();
        let pane_title = format!("Pane {}", self.pane_id);
        let options_line_width: u16 = self
            .options
            .iter()
            .map(|(label, _)| (UnicodeWidthStr::width(label.as_str()) + 2) as u16)
            .sum();
        let title_width = UnicodeWidthStr::width(pane_title.as_str()) as u16;
        let required_inner_width = options_line_width.max(title_width).max(1);
        let available_inner_width = popup_area.width.max(1);
        let wrapped_lines = required_inner_width.div_ceil(available_inner_width).max(1);
        let selector_width = required_inner_width.min(popup_area.width).max(1);
        let selector_height = (wrapped_lines + 1).min(popup_area.height).max(1);
        let area = centered_rect_size(selector_width, selector_height, popup_area);
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
        let text = Text::from(vec![Line::from(pane_title), Line::from(spans)]);
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });

        frame.render_widget(Clear, popup_area);
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
    let contexts = crate::context::load_contexts().map_err(context_resolution_failure)?;

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

fn centered_rect_size(width: u16, height: u16, rect: Rect) -> Rect {
    let width = width.min(rect.width);
    let height = height.min(rect.height);
    let x = rect.x + rect.width.saturating_sub(width) / 2;
    let y = rect.y + rect.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

fn build_search_candidates(sessions: &[SessionRow]) -> Vec<SearchCandidate> {
    sessions.iter().map(search_candidate_for_session).collect()
}

fn search_candidate_for_session(session: &SessionRow) -> SearchCandidate {
    let session_fields = [
        session.id.clone(),
        session.name.clone(),
        status_text(session.status.as_ref()),
        session.context.clone(),
    ];

    let window_fields = session.windows.iter().flat_map(|window| {
        std::iter::once(window.id.clone())
            .chain(std::iter::once(window.name.clone()))
            .chain(std::iter::once(status_text(window.status.as_ref())))
            .chain(std::iter::once(window.context.clone()))
            .chain(window.panes.iter().flat_map(|pane| {
                std::iter::once(pane.id.clone())
                    .chain(std::iter::once(pane.alias.clone().unwrap_or_default()))
                    .chain(std::iter::once(status_text(pane.status.as_ref())))
                    .chain(std::iter::once(pane.context.clone()))
            }))
    });

    let search_text = session_fields
        .into_iter()
        .chain(window_fields)
        .collect::<Vec<_>>()
        .join("\t");

    SearchCandidate {
        id: session.id.clone(),
        search_text,
    }
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

    let mut panes_by_session_window: HashMap<String, HashMap<String, (String, Vec<String>)>> =
        HashMap::new();
    for pane in panes {
        panes_by_session_window
            .entry(pane.session_name)
            .or_default()
            .entry(pane.window_id)
            .or_insert_with(|| (pane.window_name, Vec::new()))
            .1
            .push(pane.pane_id);
    }

    let built: Vec<SessionRow> = sessions
        .into_iter()
        .map(|session| {
            let key = crate::context::session_key(&session.name);
            let context = contexts.get(&key);

            let context_value =
                normalize_field(context.and_then(|ctx| ctx.session_context.as_ref()));
            let override_status = context.and_then(|ctx| ctx.session_status.clone());

            let mut window_rows = panes_by_session_window
                .remove(&session.name)
                .unwrap_or_default()
                .into_iter()
                .collect::<Vec<_>>();
            window_rows.sort_by(|(left_id, _), (right_id, _)| left_id.cmp(right_id));

            let windows: Vec<WindowRow> = window_rows
                .into_iter()
                .map(|(window_id, (window_live_name, mut pane_rows))| {
                    pane_rows.sort();
                    let window_ctx = context.and_then(|ctx| ctx.windows.get(&window_id));
                    let window_name = window_ctx
                        .and_then(|window| window.window_name.as_ref())
                        .map(|name| name.to_string())
                        .unwrap_or(window_live_name);
                    let window_context = normalize_field(
                        window_ctx.and_then(|window| window.window_context.as_ref()),
                    );
                    let window_status = window_ctx.and_then(|window| window.window_status.clone());

                    let panes: Vec<PaneRow> = pane_rows
                        .into_iter()
                        .map(|pane_id| {
                            let pane_ctx = context.and_then(|ctx| ctx.panes.get(&pane_id));
                            let pane_alias = pane_ctx.and_then(|pane| {
                                pane.pane_name
                                    .as_ref()
                                    .map(|name| name.trim())
                                    .filter(|name| !name.is_empty())
                                    .map(|name| name.to_string())
                            });
                            let pane_status = pane_ctx.and_then(|pane| pane.pane_status.clone());
                            let pane_context_value = normalize_field(
                                pane_ctx.and_then(|pane| pane.pane_context.as_ref()),
                            );
                            PaneRow {
                                id: pane_id,
                                window_id: window_id.clone(),
                                alias: pane_alias,
                                status: pane_status,
                                context: pane_context_value,
                                session_id: session.id.clone(),
                            }
                        })
                        .collect();
                    let status = crate::context::effective_session_status(
                        window_status,
                        panes.iter().map(|pane| pane.status.clone()),
                    );

                    WindowRow {
                        id: window_id,
                        name: window_name,
                        status,
                        context: window_context,
                        panes,
                        session_id: session.id.clone(),
                    }
                })
                .collect();

            let status = crate::context::effective_session_status(
                override_status,
                windows.iter().map(|window| window.status.clone()),
            );
            SessionRow {
                id: session.id,
                name: session.name,
                status,
                context: context_value,
                windows,
            }
        })
        .collect();
    info!(
        "built {} session rows from {} contexts and {} pane groups",
        built.len(),
        contexts.len(),
        panes_by_session_window.len()
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
        RowItem::Window(row) => row.status.as_ref(),
        RowItem::Pane(row) => row.status.as_ref(),
    }
}

fn row_context(item: &RowItem) -> String {
    match item {
        RowItem::Session(row) => row.context.clone(),
        RowItem::Window(row) => row.context.clone(),
        RowItem::Pane(row) => row.context.clone(),
    }
}

fn pane_label(row: &PaneRow) -> String {
    let base = row.alias.as_deref().unwrap_or(&row.id);
    truncate_with_ellipsis(base, PANE_LABEL_MAX_WIDTH)
}

fn meta_jump_index(c: char) -> Option<usize> {
    let lower = c.to_ascii_lowercase();
    if !lower.is_ascii_alphabetic() {
        return None;
    }
    let offset = (lower as u8).saturating_sub(b'a') as usize;
    if offset < 26 { Some(10 + offset) } else { None }
}

fn session_shortcut_label(index: usize) -> String {
    if index < 10 {
        return index.to_string();
    }

    let letter_offset = index - 10;
    if letter_offset < 26 {
        let letter = (b'a' + letter_offset as u8) as char;
        return format!("M-{letter}");
    }

    index.to_string()
}

fn normalize_field(value: Option<&String>) -> String {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|name| name.to_string())
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
    use crate::context::{AgentStatus, PaneContext, SessionContext, WindowContext, session_key};
    #[cfg(unix)]
    use crate::test_utils::EnvGuard;
    use crate::tmux::{TmuxPane, TmuxSession};
    use std::collections::HashMap;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn session_row(id: &str, name: &str) -> SessionRow {
        SessionRow {
            id: id.to_string(),
            name: name.to_string(),
            status: None,
            context: "ctx".to_string(),
            windows: Vec::new(),
        }
    }

    fn session_with_window_and_pane() -> SessionRow {
        SessionRow {
            id: "@1".to_string(),
            name: "alpha".to_string(),
            status: None,
            context: "ctx".to_string(),
            windows: vec![WindowRow {
                id: "@10".to_string(),
                name: "editor".to_string(),
                status: None,
                context: "wctx".to_string(),
                panes: vec![PaneRow {
                    id: "%1".to_string(),
                    window_id: "@10".to_string(),
                    alias: None,
                    status: None,
                    context: "pctx".to_string(),
                    session_id: "@1".to_string(),
                }],
                session_id: "@1".to_string(),
            }],
        }
    }

    fn session_with_sparse_windows_for_preview() -> SessionRow {
        SessionRow {
            id: "@1".to_string(),
            name: "alpha".to_string(),
            status: None,
            context: "ctx".to_string(),
            windows: vec![
                WindowRow {
                    id: "@10".to_string(),
                    name: "empty".to_string(),
                    status: None,
                    context: "wctx".to_string(),
                    panes: Vec::new(),
                    session_id: "@1".to_string(),
                },
                WindowRow {
                    id: "@20".to_string(),
                    name: "editor".to_string(),
                    status: None,
                    context: "wctx".to_string(),
                    panes: vec![
                        PaneRow {
                            id: "%9".to_string(),
                            window_id: "@20".to_string(),
                            alias: None,
                            status: None,
                            context: "pctx".to_string(),
                            session_id: "@1".to_string(),
                        },
                        PaneRow {
                            id: "%10".to_string(),
                            window_id: "@20".to_string(),
                            alias: None,
                            status: None,
                            context: "pctx".to_string(),
                            session_id: "@1".to_string(),
                        },
                    ],
                    session_id: "@1".to_string(),
                },
            ],
        }
    }

    #[cfg(unix)]
    fn setup_fake_tmux(env: &mut EnvGuard) {
        let bin_dir = env.temp_dir().join("bin");
        fs::create_dir_all(&bin_dir).expect("create bin dir");
        let script_path = bin_dir.join("tmux");
        let script = r#"#!/bin/sh
set -eu
log_file="${TMUX_LOG_FILE:?}"
cmd="${1:-}"
target="${3:-}"
case "$cmd" in
  list-sessions)
    printf "%s" "${TMUX_LIST_SESSIONS:-}"
    exit 0
    ;;
  list-panes)
    printf "%s" "${TMUX_LIST_PANES:-}"
    exit 0
    ;;
  switch-client)
    printf "switch-client:%s\n" "$target" >> "$log_file"
    exit 0
    ;;
  select-window)
    printf "select-window:%s\n" "$target" >> "$log_file"
    exit 0
    ;;
  select-pane)
    printf "select-pane:%s\n" "$target" >> "$log_file"
    exit 0
    ;;
  kill-session)
    printf "kill-session:%s\n" "$target" >> "$log_file"
    exit 0
    ;;
  kill-window)
    printf "kill-window:%s\n" "$target" >> "$log_file"
    exit 0
    ;;
  kill-pane)
    printf "kill-pane:%s\n" "$target" >> "$log_file"
    exit 0
    ;;
  *)
    printf "unsupported:%s\n" "$cmd" >> "$log_file"
    exit 1
    ;;
esac
"#;
        fs::write(&script_path, script).expect("write tmux script");
        let mut perms = fs::metadata(&script_path).expect("metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).expect("chmod");

        let old_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bin_dir.display(), old_path);
        env.set_var("PATH", new_path);
    }

    struct StubSearch {
        expected_query: Option<String>,
        matched_ids: Vec<String>,
    }

    impl SessionSearch for StubSearch {
        fn filter_ids(
            &mut self,
            query: &str,
            candidates: &[SearchCandidate],
        ) -> Result<Vec<String>, TuiError> {
            if let Some(expected) = &self.expected_query {
                assert_eq!(query, expected);
            }
            assert!(!candidates.is_empty());
            Ok(self.matched_ids.clone())
        }
    }

    struct PassthroughSearch;

    impl SessionSearch for PassthroughSearch {
        fn filter_ids(
            &mut self,
            _query: &str,
            candidates: &[SearchCandidate],
        ) -> Result<Vec<String>, TuiError> {
            Ok(candidates
                .iter()
                .map(|candidate| candidate.id.clone())
                .collect())
        }
    }

    fn passthrough_filter() -> PassthroughSearch {
        PassthroughSearch
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
                window_id: "@10".to_string(),
                window_name: "editor".to_string(),
                pane_id: "%1".to_string(),
            },
            TmuxPane {
                session_name: "alpha".to_string(),
                window_id: "@10".to_string(),
                window_name: "editor".to_string(),
                pane_id: "%2".to_string(),
            },
            TmuxPane {
                session_name: "beta".to_string(),
                window_id: "@20".to_string(),
                window_name: "server".to_string(),
                pane_id: "%3".to_string(),
            },
        ];
        let map = collect_live_panes(&panes);
        assert_eq!(map.get("alpha").map(|set| set.len()), Some(2));
        assert!(map.get("alpha").expect("alpha set").contains("%1"));
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
                window_id: Some("@10".to_string()),
                window_name: Some("editor".to_string()),
                pane_name: Some("pane-two".to_string()),
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
                windows: HashMap::from([(
                    "@10".to_string(),
                    WindowContext {
                        window_id: Some("@10".to_string()),
                        window_name: Some("editor".to_string()),
                        window_status: None,
                        window_context: None,
                    },
                )]),
                panes: pane_map,
            },
        );

        let panes = vec![
            TmuxPane {
                session_name: "alpha".to_string(),
                window_id: "@10".to_string(),
                window_name: "editor".to_string(),
                pane_id: "%2".to_string(),
            },
            TmuxPane {
                session_name: "alpha".to_string(),
                window_id: "@10".to_string(),
                window_name: "editor".to_string(),
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
        assert_eq!(alpha.windows.len(), 1);
        assert_eq!(alpha.windows[0].name, "editor");
        assert_eq!(alpha.windows[0].panes.len(), 2);
        assert_eq!(alpha.windows[0].panes[0].id, "%1");
        assert_eq!(alpha.windows[0].panes[0].context, DATA_NOT_RECEIVED);
        assert_eq!(alpha.windows[0].panes[1].id, "%2");
        assert_eq!(alpha.windows[0].panes[1].alias.as_deref(), Some("pane-two"));
        assert_eq!(alpha.windows[0].panes[1].status, Some(AgentStatus::Done));
        assert_eq!(alpha.windows[0].panes[1].context, "pctx");
    }

    #[test]
    fn apply_search_with_uses_injected_filter() {
        let sessions = vec![session_row("@1", "one"), session_row("@2", "two")];
        let filter = StubSearch {
            expected_query: Some("one".to_string()),
            matched_ids: vec!["@1".to_string()],
        };
        let mut app = App::new_with_filter(sessions, filter).expect("app");
        app.search.query = "one".to_string();

        app.apply_search_with(None).expect("search");

        assert_eq!(app.filtered_sessions.len(), 1);
        assert_eq!(app.filtered_sessions[0].id, "@1");
    }

    #[test]
    fn apply_search_prefers_first_match_over_previous_selection() {
        let sessions = vec![
            session_row("@1", "alpha"),
            session_row("@2", "alpha-dev"),
            session_row("@3", "alpha-prod"),
        ];
        let filter = StubSearch {
            expected_query: Some("alpha-".to_string()),
            matched_ids: vec!["@2".to_string(), "@3".to_string(), "@1".to_string()],
        };
        let mut app = App::new_with_filter(sessions, filter).expect("app");
        app.state.select(Some(2));
        app.search.query = "alpha-".to_string();

        app.apply_search().expect("search");

        match app.selected_row().expect("selected row") {
            RowItem::Session(row) => assert_eq!(row.id, "@2"),
            RowItem::Window(_) | RowItem::Pane(_) => panic!("expected session row"),
        }
    }

    #[test]
    fn apply_search_preserves_selection_when_query_empty() {
        let sessions = vec![session_row("@1", "alpha"), session_row("@2", "beta")];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.state.select(Some(1));
        app.search.query.clear();

        app.apply_search().expect("search");

        match app.selected_row().expect("selected row") {
            RowItem::Session(row) => assert_eq!(row.id, "@2"),
            RowItem::Window(_) | RowItem::Pane(_) => panic!("expected session row"),
        }
    }

    #[test]
    fn build_search_candidates_include_window_and_pane_fields() {
        let sessions = vec![SessionRow {
            id: "@1".to_string(),
            name: "alpha".to_string(),
            status: None,
            context: "sctx".to_string(),
            windows: vec![
                WindowRow {
                    id: "@10".to_string(),
                    name: "editor".to_string(),
                    status: Some(AgentStatus::Working),
                    context: "wctx".to_string(),
                    panes: vec![PaneRow {
                        id: "%1".to_string(),
                        window_id: "@10".to_string(),
                        alias: Some("deploy-pane".to_string()),
                        status: Some(AgentStatus::Waiting),
                        context: "pctx".to_string(),
                        session_id: "@1".to_string(),
                    }],
                    session_id: "@1".to_string(),
                },
                WindowRow {
                    id: "@11".to_string(),
                    name: "server".to_string(),
                    status: Some(AgentStatus::Done),
                    context: "ops".to_string(),
                    panes: vec![PaneRow {
                        id: "%2".to_string(),
                        window_id: "@11".to_string(),
                        alias: Some("logs".to_string()),
                        status: Some(AgentStatus::Working),
                        context: "tail".to_string(),
                        session_id: "@1".to_string(),
                    }],
                    session_id: "@1".to_string(),
                },
            ],
        }];

        let candidates = build_search_candidates(&sessions);
        assert_eq!(candidates.len(), 1);
        let text = &candidates[0].search_text;
        assert!(text.contains("alpha"));
        assert!(text.contains("editor"));
        assert!(text.contains("deploy-pane"));
        assert!(text.contains("pctx"));
        assert!(text.contains("server"));
        assert!(text.contains("logs"));
        assert!(text.contains("tail"));
    }

    #[test]
    fn nucleo_session_search_supports_fuzzy_filtering() {
        let mut search = NucleoSessionSearch::new();
        let candidates = vec![
            SearchCandidate {
                id: "@1".to_string(),
                search_text: "@1\talpha\t-\tctx".to_string(),
            },
            SearchCandidate {
                id: "@2".to_string(),
                search_text: "@2\tbeta\t-\tctx".to_string(),
            },
            SearchCandidate {
                id: "@3".to_string(),
                search_text: "@3\tgamma\t-\tctx".to_string(),
            },
        ];

        let matches = search
            .filter_ids("gma", &candidates)
            .expect("nucleo fuzzy search");
        assert_eq!(matches, vec!["@3".to_string()]);
    }

    #[test]
    fn meta_jump_index_maps_option_letters() {
        assert_eq!(meta_jump_index('a'), Some(10));
        assert_eq!(meta_jump_index('z'), Some(35));
        assert_eq!(meta_jump_index('A'), Some(10));
        assert_eq!(meta_jump_index('0'), None);
        assert_eq!(meta_jump_index('-'), None);
    }

    #[test]
    fn row_label_prefixes_session_index() {
        let sessions = vec![session_row("@1", "alpha"), session_row("@2", "beta")];
        let app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        assert_eq!(app.row_label(&app.rows[0]), "0: alpha");
        assert_eq!(app.row_label(&app.rows[1]), "1: beta");
    }

    #[test]
    fn row_label_uses_meta_shortcuts_after_nine() {
        let sessions = (0..12)
            .map(|index| session_row(&format!("@{index}"), &format!("session-{index}")))
            .collect();
        let app = App::new_with_filter(sessions, passthrough_filter()).expect("app");

        assert_eq!(app.row_label(&app.rows[10]), "M-a: session-10");
        assert_eq!(app.row_label(&app.rows[11]), "M-b: session-11");
    }

    #[test]
    fn row_label_prefers_pane_alias_and_truncates_to_limit() {
        let sessions = vec![SessionRow {
            id: "@1".to_string(),
            name: "alpha".to_string(),
            status: None,
            context: "ctx".to_string(),
            windows: vec![WindowRow {
                id: "@10".to_string(),
                name: "editor".to_string(),
                status: None,
                context: "wctx".to_string(),
                panes: vec![PaneRow {
                    id: "%1".to_string(),
                    window_id: "@10".to_string(),
                    alias: Some("verylongalias".to_string()),
                    status: None,
                    context: "p1".to_string(),
                    session_id: "@1".to_string(),
                }],
                session_id: "@1".to_string(),
            }],
        }];

        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();

        assert_eq!(app.row_label(&app.rows[2]), "    └─ verylon...");
    }

    #[test]
    fn table_column_widths_cap_session_and_preserve_context() {
        let sessions = vec![session_row(
            "@1",
            "this-is-a-very-long-session-name-that-should-not-hide-context",
        )];
        let app = App::new_with_filter(sessions, passthrough_filter()).expect("app");

        let widths = app.table_column_widths(Rect::new(0, 0, 60, 10));
        assert!(widths.session < app.widths.session);
        assert!(usize::from(widths.context) >= UnicodeWidthStr::width("Context"));

        let rendered_label =
            truncate_with_ellipsis(&app.row_label(&app.rows[0]), widths.session as usize);
        assert!(rendered_label.ends_with("..."));
    }

    #[test]
    fn jump_to_session_index_targets_session_rows_with_expanded_panes() {
        let sessions = vec![
            SessionRow {
                id: "@1".to_string(),
                name: "alpha".to_string(),
                status: None,
                context: "ctx".to_string(),
                windows: vec![WindowRow {
                    id: "@10".to_string(),
                    name: "editor".to_string(),
                    status: None,
                    context: "wctx".to_string(),
                    panes: vec![
                        PaneRow {
                            id: "%1".to_string(),
                            window_id: "@10".to_string(),
                            alias: None,
                            status: None,
                            context: "p1".to_string(),
                            session_id: "@1".to_string(),
                        },
                        PaneRow {
                            id: "%2".to_string(),
                            window_id: "@10".to_string(),
                            alias: None,
                            status: None,
                            context: "p2".to_string(),
                            session_id: "@1".to_string(),
                        },
                    ],
                    session_id: "@1".to_string(),
                }],
            },
            session_row("@2", "beta"),
        ];

        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.jump_to_session_index(1);

        let selected = app.selected_row().expect("selected row");
        match selected {
            RowItem::Session(row) => assert_eq!(row.id, "@2"),
            RowItem::Window(_) | RowItem::Pane(_) => panic!("expected session row"),
        }
    }

    #[test]
    fn preview_is_disabled_by_default() {
        let sessions = vec![session_row("@1", "alpha")];
        let app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        assert!(!app.preview_enabled);
        assert_eq!(app.preview_text, PREVIEW_DEFAULT_TEXT);
    }

    #[test]
    fn preview_target_for_pane_row_uses_selected_pane() {
        let sessions = vec![session_with_window_and_pane()];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.state.select(Some(2));

        let target = app
            .preview_target_for_selected_row()
            .expect("preview target");
        assert_eq!(target.pane_id, "%1");
    }

    #[test]
    fn preview_target_for_window_row_uses_first_pane() {
        let sessions = vec![session_with_sparse_windows_for_preview()];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.state.select(Some(2));

        let target = app
            .preview_target_for_selected_row()
            .expect("preview target");
        assert_eq!(target.pane_id, "%9");
    }

    #[test]
    fn preview_target_for_session_row_uses_first_available_pane() {
        let sessions = vec![session_with_sparse_windows_for_preview()];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.state.select(Some(0));

        let target = app
            .preview_target_for_selected_row()
            .expect("preview target");
        assert_eq!(target.pane_id, "%9");
    }

    #[test]
    fn expand_all_toggles_between_expanded_and_collapsed() {
        let sessions = vec![
            session_with_window_and_pane(),
            SessionRow {
                id: "@2".to_string(),
                name: "beta".to_string(),
                status: None,
                context: "ctx".to_string(),
                windows: vec![WindowRow {
                    id: "@20".to_string(),
                    name: "server".to_string(),
                    status: None,
                    context: "wctx".to_string(),
                    panes: vec![PaneRow {
                        id: "%2".to_string(),
                        window_id: "@20".to_string(),
                        alias: None,
                        status: None,
                        context: "pctx".to_string(),
                        session_id: "@2".to_string(),
                    }],
                    session_id: "@2".to_string(),
                }],
            },
        ];

        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        assert_eq!(app.rows.len(), 2);

        app.expand_all();

        assert_eq!(app.rows.len(), 6);
        assert!(app.expanded_sessions.contains("@1"));
        assert!(app.expanded_sessions.contains("@2"));

        app.expand_all();

        assert_eq!(app.rows.len(), 2);
        assert!(app.expanded_sessions.is_empty());
    }

    #[test]
    fn delete_mode_exits_on_non_x_key() {
        let sessions = vec![session_row("@1", "alpha")];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");

        app.enter_delete_mode();
        match &app.mode {
            ListViewModes::CommandMode(CommandMode::DeleteConfirm(RowKey::Session(session_id))) => {
                assert_eq!(session_id, "@1");
            }
            ListViewModes::NormalMode | ListViewModes::SearchMode | ListViewModes::HelpMode => {
                panic!("expected delete confirmation mode")
            }
            ListViewModes::CommandMode(CommandMode::DeleteConfirm(RowKey::Window { .. }))
            | ListViewModes::CommandMode(CommandMode::DeleteConfirm(RowKey::Pane { .. })) => {
                panic!("expected session deletion target")
            }
        }

        app.handle_command_mode_key(KeyCode::Char('j'))
            .expect("cancel delete mode");
        assert!(matches!(app.mode, ListViewModes::NormalMode));
    }

    #[test]
    #[cfg(unix)]
    fn delete_mode_on_session_deletes_on_second_x() {
        let mut env = EnvGuard::new("tui-delete-session");
        env.set_temp_home();
        setup_fake_tmux(&mut env);
        let log_path = env.temp_dir().join("tmux.log");
        env.set_var("TMUX_LOG_FILE", &log_path);
        env.set_var("TMUX_LIST_SESSIONS", "");
        env.set_var("TMUX_LIST_PANES", "");

        let sessions = vec![session_row("@1", "alpha")];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.enter_delete_mode();

        app.handle_command_mode_key(KeyCode::Char('x'))
            .expect("confirm delete");

        assert!(matches!(app.mode, ListViewModes::NormalMode));
        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(log, "kill-session:@1\n");
    }

    #[test]
    #[cfg(unix)]
    fn delete_mode_on_window_deletes_on_second_x() {
        let mut env = EnvGuard::new("tui-delete-window");
        env.set_temp_home();
        setup_fake_tmux(&mut env);
        let log_path = env.temp_dir().join("tmux.log");
        env.set_var("TMUX_LOG_FILE", &log_path);
        env.set_var("TMUX_LIST_SESSIONS", "");
        env.set_var("TMUX_LIST_PANES", "");

        let sessions = vec![session_with_window_and_pane()];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.state.select(Some(1));
        app.enter_delete_mode();

        app.handle_command_mode_key(KeyCode::Char('x'))
            .expect("confirm delete");

        assert!(matches!(app.mode, ListViewModes::NormalMode));
        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(log, "kill-window:@10\n");
    }

    #[test]
    #[cfg(unix)]
    fn delete_mode_on_pane_deletes_on_second_x() {
        let mut env = EnvGuard::new("tui-delete-pane");
        env.set_temp_home();
        setup_fake_tmux(&mut env);
        let log_path = env.temp_dir().join("tmux.log");
        env.set_var("TMUX_LOG_FILE", &log_path);
        env.set_var("TMUX_LIST_SESSIONS", "");
        env.set_var("TMUX_LIST_PANES", "");

        let sessions = vec![session_with_window_and_pane()];
        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.state.select(Some(2));
        app.enter_delete_mode();

        app.handle_command_mode_key(KeyCode::Char('x'))
            .expect("confirm delete");

        assert!(matches!(app.mode, ListViewModes::NormalMode));
        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(log, "kill-pane:%1\n");
    }

    #[test]
    #[cfg(unix)]
    fn switch_session_on_pane_selects_target_pane() {
        let mut env = EnvGuard::new("tui-switch-pane-target");
        setup_fake_tmux(&mut env);
        let log_path = env.temp_dir().join("tmux.log");
        env.set_var("TMUX_LOG_FILE", &log_path);

        let sessions = vec![SessionRow {
            id: "@1".to_string(),
            name: "alpha".to_string(),
            status: None,
            context: "ctx".to_string(),
            windows: vec![WindowRow {
                id: "@10".to_string(),
                name: "editor".to_string(),
                status: None,
                context: "wctx".to_string(),
                panes: vec![PaneRow {
                    id: "%9".to_string(),
                    window_id: "@10".to_string(),
                    alias: None,
                    status: None,
                    context: "pctx".to_string(),
                    session_id: "@1".to_string(),
                }],
                session_id: "@1".to_string(),
            }],
        }];

        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.state.select(Some(2));

        app.switch_session().expect("switch");

        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(log, "switch-client:@1\nselect-window:@10\nselect-pane:%9\n");
    }

    #[test]
    #[cfg(unix)]
    fn switch_session_on_pane_switches_to_target_window() {
        let mut env = EnvGuard::new("tui-switch-pane-window-target");
        setup_fake_tmux(&mut env);
        let log_path = env.temp_dir().join("tmux.log");
        env.set_var("TMUX_LOG_FILE", &log_path);

        let sessions = vec![SessionRow {
            id: "@1".to_string(),
            name: "alpha".to_string(),
            status: None,
            context: "ctx".to_string(),
            windows: vec![
                WindowRow {
                    id: "@10".to_string(),
                    name: "editor".to_string(),
                    status: None,
                    context: "wctx1".to_string(),
                    panes: vec![PaneRow {
                        id: "%1".to_string(),
                        window_id: "@10".to_string(),
                        alias: None,
                        status: None,
                        context: "p1".to_string(),
                        session_id: "@1".to_string(),
                    }],
                    session_id: "@1".to_string(),
                },
                WindowRow {
                    id: "@20".to_string(),
                    name: "server".to_string(),
                    status: None,
                    context: "wctx2".to_string(),
                    panes: vec![PaneRow {
                        id: "%9".to_string(),
                        window_id: "@20".to_string(),
                        alias: None,
                        status: None,
                        context: "p9".to_string(),
                        session_id: "@1".to_string(),
                    }],
                    session_id: "@1".to_string(),
                },
            ],
        }];

        let mut app = App::new_with_filter(sessions, passthrough_filter()).expect("app");
        app.expanded_sessions.insert("@1".to_string());
        app.rebuild_rows();
        app.state.select(Some(4));

        app.switch_session().expect("switch");

        let log = fs::read_to_string(&log_path).expect("read log");
        assert_eq!(log, "switch-client:@1\nselect-window:@20\nselect-pane:%9\n");
    }
    #[test]
    fn backend_failure_preserves_message() {
        let error = backend_failure("boom");
        assert_eq!(error.to_string(), "boom");
    }

    #[test]
    fn context_resolution_failure_preserves_message() {
        let error = context_resolution_failure("failed to load contexts");
        assert_eq!(error.to_string(), "failed to load contexts");
    }

    #[test]
    fn search_failure_preserves_message() {
        let error = search_failure("query too long".to_string());
        assert_eq!(error.to_string(), "Search failure: query too long");
    }
}
