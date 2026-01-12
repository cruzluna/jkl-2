use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState};
use ratatui::{DefaultTerminal, Frame};
use std::collections::HashMap;
use std::io::{self, Write};
use std::process::{Command, Stdio};
use unicode_width::UnicodeWidthStr;

const DATA_NOT_RECEIVED: &str = "-";
const INFO_TEXT: &str = "(Esc/Ctrl+C) back/quit | (/) search | (Enter) switch | (↑/↓) move";

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let sessions = crate::tmux::list_sessions()?;
    let contexts = crate::context::load_contexts()?;
    let items = build_rows(sessions, contexts);
    let mut app = App::new(items)?;
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

#[derive(Clone)]
struct SessionRow {
    id: String,
    name: String,
    status: Option<crate::context::AgentStatus>,
    context: String,
}

struct App {
    state: TableState,
    items: Vec<SessionRow>,
    filtered_items: Vec<SessionRow>,
    widths: (u16, u16, u16),
    search_query: String,
    search_mode: bool,
}

impl App {
    fn new(items: Vec<SessionRow>) -> Result<Self, Box<dyn std::error::Error>> {
        let widths = measure_widths(&items);
        let mut app = Self {
            state: TableState::default(),
            filtered_items: items.clone(),
            items,
            widths,
            search_query: String::new(),
            search_mode: false,
        };
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
                        _ => {}
                    }
                }
            }
        }
    }

    fn next_row(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        let next = match self.state.selected() {
            Some(index) if index + 1 < self.filtered_items.len() => index + 1,
            _ => 0,
        };
        self.state.select(Some(next));
    }

    fn previous_row(&mut self) {
        if self.filtered_items.is_empty() {
            return;
        }
        let prev = match self.state.selected() {
            Some(0) | None => self.filtered_items.len() - 1,
            Some(index) => index - 1,
        };
        self.state.select(Some(prev));
    }

    fn ensure_selection(&mut self) {
        if self.filtered_items.is_empty() {
            self.state.select(None);
        } else if self.state.selected().is_none() {
            self.state.select(Some(0));
        }
    }

    fn selected_row(&self) -> Option<&SessionRow> {
        let index = self.state.selected()?;
        self.filtered_items.get(index)
    }

    fn apply_search(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let previous = self.selected_row().map(|row| row.id.clone());
        if self.search_query.trim().is_empty() {
            self.filtered_items = self.items.clone();
            self.restore_selection(previous);
            return Ok(());
        }

        let candidates = self
            .items
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
            .items
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
        self.filtered_items = filtered;
        self.restore_selection(previous);
        Ok(())
    }

    fn restore_selection(&mut self, previous: Option<String>) {
        if self.filtered_items.is_empty() {
            self.state.select(None);
            return;
        }
        if let Some(id) = previous {
            if let Some(index) = self.filtered_items.iter().position(|row| row.id == id) {
                self.state.select(Some(index));
                return;
            }
        }
        self.state.select(Some(0));
    }

    fn switch_selected(&self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(row) = self.selected_row() {
            crate::tmux::switch_client(&row.id)?;
        }
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

        let rows = self.filtered_items.iter().enumerate().map(|(index, item)| {
            let base_style = if index % 2 == 0 {
                Style::default()
            } else {
                Style::default().add_modifier(Modifier::DIM)
            };
            Row::new(vec![
                Cell::from(item.name.clone()),
                Cell::from(status_text(item.status.as_ref()))
                    .style(status_style(item.status.as_ref())),
                Cell::from(item.context.clone()),
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

fn build_rows(
    sessions: Vec<crate::tmux::TmuxSession>,
    contexts: HashMap<String, crate::context::SessionContext>,
) -> Vec<SessionRow> {
    sessions
        .into_iter()
        .map(|session| {
            let context = contexts.get(&session.id);
            let status = context.and_then(|ctx| ctx.status.clone());
            let context_value = normalize_field(context.and_then(|ctx| ctx.context.as_ref()));
            SessionRow {
                id: session.id,
                name: session.name,
                status,
                context: context_value,
            }
        })
        .collect()
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
        Some(crate::context::AgentStatus::Working) => Style::default().fg(Color::Blue),
        Some(crate::context::AgentStatus::Waiting | crate::context::AgentStatus::Idle) => {
            Style::default().fg(Color::Yellow)
        }
        None => Style::default(),
    }
}

fn measure_widths(items: &[SessionRow]) -> (u16, u16, u16) {
    let name_len = items
        .iter()
        .map(|item| UnicodeWidthStr::width(item.name.as_str()))
        .max()
        .unwrap_or(0)
        .max(UnicodeWidthStr::width("Session"));
    let status_len = items
        .iter()
        .map(|item| UnicodeWidthStr::width(status_text(item.status.as_ref()).as_str()))
        .max()
        .unwrap_or(0)
        .max(UnicodeWidthStr::width("Status"));
    let context_len = items
        .iter()
        .map(|item| UnicodeWidthStr::width(item.context.as_str()))
        .max()
        .unwrap_or(0)
        .max(UnicodeWidthStr::width("Context"));

    #[allow(clippy::cast_possible_truncation)]
    (name_len as u16, status_len as u16, context_len as u16)
}
