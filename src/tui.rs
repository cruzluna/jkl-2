use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::process::{Command, Stdio};
use unicode_width::UnicodeWidthStr;

const DATA_NOT_RECEIVED: &str = "-";
const INFO_TEXT: &str = "(Esc/Ctrl+C) back/quit | (/) search | (Enter) switch | (↑/↓) move | (l/h) expand/collapse | (r) refresh";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let sessions = crate::tmux::list_sessions()?;
    let contexts = crate::context::load_contexts()?;
    let panes = crate::tmux::list_panes()?;
    let items = build_sessions(sessions, contexts, panes);
    let mut app = App::new(items)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

pub fn run_pane_selector(
    session_name: String,
    pane_id: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut selector = PaneSelector::new(session_name, pane_id)?;
    let mut terminal = ratatui::init();
    let result = selector.run(&mut terminal);
    ratatui::restore();
    result
}

#[derive(Clone)]
struct SessionRow {
    id: String,
    name: String,
    status: Option<crate::context::AgentStatus>,
    context: String,
    panes: Vec<PaneRow>,
}

#[derive(Clone)]
struct PaneRow {
    id: String,
    status: Option<crate::context::AgentStatus>,
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

struct App {
    state: TableState,
    sessions: Vec<SessionRow>,
    filtered_sessions: Vec<SessionRow>,
    rows: Vec<RowItem>,
    widths: (u16, u16, u16),
    search_query: String,
    search_mode: bool,
    expanded_sessions: HashSet<String>,
}

impl App {
    fn new(sessions: Vec<SessionRow>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut app = Self {
            state: TableState::default(),
            filtered_sessions: sessions.clone(),
            sessions,
            rows: Vec::new(),
            widths: (0, 0, 0),
            search_query: String::new(),
            search_mode: false,
            expanded_sessions: HashSet::new(),
        };
        app.rebuild_rows();
        app.ensure_selection();
        Ok(app)
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), Box<dyn std::error::Error>> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                if self.search_mode {
                    match key.code {
                        KeyCode::Esc => {
                            self.search_mode = false;
                        }
                        KeyCode::Enter => {
                            self.switch_selected()?;
                            return Ok(());
                        }
                        KeyCode::Backspace => {
                            self.search_query.pop();
                            self.apply_search()?;
                        }
                        KeyCode::Down => self.next_row(),
                        KeyCode::Up => self.previous_row(),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.search_mode = false;
                        }
                        KeyCode::Char(c) => {
                            self.search_query.push(c);
                            self.apply_search()?;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        KeyCode::Char('/') => {
                            self.search_mode = true;
                            self.apply_search()?;
                        }
                        KeyCode::Enter => {
                            self.switch_selected()?;
                            return Ok(());
                        }
                        KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                        KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
                        KeyCode::Char('l') => self.expand_selected(),
                        KeyCode::Char('h') => self.collapse_selected(),
                        KeyCode::Char('r') => {
                            self.refresh_panes()?;
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

    fn apply_search(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let previous = self.selected_key();
        self.apply_search_with(previous)
    }

    fn apply_search_with(
        &mut self,
        previous: Option<RowKey>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.search_query.trim().is_empty() {
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

        let output = run_fzf_filter(&self.search_query, &candidates)?;
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
        self.widths = measure_widths(&self.rows);
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

    fn switch_selected(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(row) = self.selected_row() {
            let session_id = match row {
                RowItem::Session(session) => session.id.as_str(),
                RowItem::Pane(pane) => pane.session_id.as_str(),
            };
            crate::tmux::switch_client(session_id)?;
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

    fn refresh_panes(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let live_panes = crate::tmux::list_panes()?;
        let live_map = collect_live_panes(&live_panes);
        crate::context::prune_panes(&live_map)?;
        self.reload_data()?;
        Ok(())
    }

    fn reload_data(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let previous = self.selected_key();
        let sessions = crate::tmux::list_sessions()?;
        let contexts = crate::context::load_contexts()?;
        let panes = crate::tmux::list_panes()?;
        self.sessions = build_sessions(sessions, contexts, panes);
        self.filtered_sessions = self.sessions.clone();
        self.rebuild_rows();
        self.apply_search_with(previous)?;
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
        let (text, style) = if self.search_query.is_empty() {
            (
                "Search: ".to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )
        } else {
            (format!("Search: {}", self.search_query), Style::default())
        };
        let search = Paragraph::new(Text::from(text)).style(style);
        frame.render_widget(search, area);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header = Row::new(["Session", "Status", "Context"])
            .style(Style::default().add_modifier(Modifier::BOLD));

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
                Cell::from(row_label(item)),
                Cell::from(status_text(row_status(item))).style(status_style(row_status(item))),
                Cell::from(row_context(item)),
            ])
            .style(base_style)
        });

        let table = Table::new(
            rows,
            [
                Constraint::Length(self.widths.0 + 1),
                Constraint::Length(self.widths.1 + 1),
                Constraint::Min(self.widths.2 + 1),
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
        let mode = if self.search_mode {
            "[SEARCH]"
        } else {
            "[NORM]"
        };
        let mode_widget = Paragraph::new(Text::from(mode)).alignment(Alignment::Right);

        frame.render_widget(footer, sections[0]);
        frame.render_widget(mode_widget, sections[1]);
    }
}

struct PaneSelector {
    session_name: String,
    pane_id: String,
    options: Vec<(String, Option<crate::context::AgentStatus>)>,
    selected: usize,
}

impl PaneSelector {
    fn new(session_name: String, pane_id: String) -> Result<Self, Box<dyn std::error::Error>> {
        let options = pane_status_options();
        let current = current_pane_status(&session_name, &pane_id)?;
        let selected = options
            .iter()
            .position(|(_, status)| *status == current)
            .unwrap_or(0);
        Ok(Self {
            session_name,
            pane_id,
            options,
            selected,
        })
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<(), Box<dyn std::error::Error>> {
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
                        crate::context::upsert_pane(
                            &self.session_name,
                            &self.pane_id,
                            status,
                            None,
                        )?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }

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
        let pane_title = if self.pane_id.trim().is_empty() {
            "Pane (unknown)".to_string()
        } else {
            format!("Pane {}", self.pane_id)
        };
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
        ("idle".to_string(), Some(crate::context::AgentStatus::Idle)),
        ("done".to_string(), Some(crate::context::AgentStatus::Done)),
        ("none".to_string(), Some(crate::context::AgentStatus::None)),
    ]
}

fn current_pane_status(
    session_name: &str,
    pane_id: &str,
) -> Result<Option<crate::context::AgentStatus>, Box<dyn std::error::Error>> {
    let contexts = crate::context::load_contexts()?;
    let key = crate::context::session_key(session_name);
    let status = contexts
        .get(&key)
        .and_then(|session| session.panes.get(pane_id))
        .and_then(|pane| pane.status.clone());
    Ok(status)
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

fn run_fzf_filter(
    query: &str,
    candidates: &[String],
) -> Result<String, Box<dyn std::error::Error>> {
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
        return Err(Box::new(io::Error::new(io::ErrorKind::Other, message)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn build_sessions(
    sessions: Vec<crate::tmux::TmuxSession>,
    contexts: HashMap<String, crate::context::SessionContext>,
    panes: Vec<crate::tmux::TmuxPane>,
) -> Vec<SessionRow> {
    let mut panes_by_session: HashMap<String, Vec<String>> = HashMap::new();
    for pane in panes {
        panes_by_session
            .entry(pane.session_name)
            .or_default()
            .push(pane.pane_id);
    }

    sessions
        .into_iter()
        .map(|session| {
            let key = crate::context::session_key(&session.name);
            let context = contexts.get(&key);
            let status = context.and_then(|ctx| ctx.status.clone());
            let context_value = normalize_field(context.and_then(|ctx| ctx.context.as_ref()));
            let mut pane_rows = panes_by_session
                .get(&session.name)
                .cloned()
                .unwrap_or_default();
            pane_rows.sort();
            let panes = pane_rows
                .into_iter()
                .map(|pane_id| {
                    let pane_status = context
                        .and_then(|ctx| ctx.panes.get(&pane_id))
                        .and_then(|pane| pane.status.clone());
                    PaneRow {
                        id: pane_id,
                        status: pane_status,
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
        .collect()
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

fn row_label(item: &RowItem) -> String {
    match item {
        RowItem::Session(row) => row.name.clone(),
        RowItem::Pane(row) => format!("  └─ {}", row.id),
    }
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
        RowItem::Pane(_) => DATA_NOT_RECEIVED.to_string(),
    }
}

fn normalize_field(value: Option<&String>) -> String {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| DATA_NOT_RECEIVED.to_string())
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
        Some(crate::context::AgentStatus::Waiting | crate::context::AgentStatus::Idle) => {
            Style::default().fg(Color::Yellow)
        }
        None => Style::default(),
    }
}

fn measure_widths(items: &[RowItem]) -> (u16, u16, u16) {
    let name_len = items
        .iter()
        .map(|item| UnicodeWidthStr::width(row_label(item).as_str()))
        .max()
        .unwrap_or(0)
        .max(UnicodeWidthStr::width("Session"));
    let status_len = items
        .iter()
        .map(|item| UnicodeWidthStr::width(status_text(row_status(item)).as_str()))
        .max()
        .unwrap_or(0)
        .max(UnicodeWidthStr::width("Status"));
    let context_len = items
        .iter()
        .map(|item| UnicodeWidthStr::width(row_context(item).as_str()))
        .max()
        .unwrap_or(0)
        .max(UnicodeWidthStr::width("Context"));

    #[allow(clippy::cast_possible_truncation)]
    (name_len as u16, status_len as u16, context_len as u16)
}
