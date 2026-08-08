mod win;

use std::collections::HashMap;
use std::io;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;

use win::WindowInfo;

struct App {
    windows: Vec<WindowInfo>,
    list_state: ListState,
    fullscreened: HashMap<isize, win::SavedState>,
    status: String,
}

impl App {
    fn new() -> Self {
        let windows = win::list_windows();
        let mut list_state = ListState::default();
        if !windows.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            windows,
            list_state,
            fullscreened: HashMap::new(),
            status: "↑/↓ move  ·  Enter toggle fullscreen  ·  r refresh  ·  Esc quit".into(),
        }
    }

    fn refresh(&mut self) {
        let selected_hwnd = self
            .list_state
            .selected()
            .and_then(|i| self.windows.get(i))
            .map(|w| w.hwnd.0);

        self.windows = win::list_windows();

        let new_index = selected_hwnd
            .and_then(|h| self.windows.iter().position(|w| w.hwnd.0 == h))
            .or(if self.windows.is_empty() {
                None
            } else {
                Some(0)
            });
        self.list_state.select(new_index);
        self.status = "Refreshed window list.".into();
    }

    fn next(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.windows.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        if self.windows.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(0) | None => self.windows.len() - 1,
            Some(i) => i - 1,
        };
        self.list_state.select(Some(i));
    }

    fn toggle_fullscreen(&mut self) {
        let Some(i) = self.list_state.selected() else {
            return;
        };
        let Some(window) = self.windows.get(i) else {
            return;
        };
        let hwnd = window.hwnd;
        let key = hwnd.0 as isize;
        let title = window.title.clone();

        if let Some(saved) = self.fullscreened.remove(&key) {
            match win::restore_window(hwnd, &saved) {
                Ok(()) => self.status = format!("Restored \"{title}\"."),
                Err(e) => self.status = format!("Failed to restore \"{title}\": {e}"),
            }
        } else {
            match win::fullscreen_window(hwnd) {
                Ok(saved) => {
                    self.fullscreened.insert(key, saved);
                    self.status = format!("Fullscreened \"{title}\".");
                }
                Err(e) => self.status = format!("Failed to fullscreen \"{title}\": {e}"),
            }
        }
    }

    /// Put back every window we've borderless-fullscreened, so nothing is
    /// left in a broken state when the TUI exits.
    fn restore_all(&mut self) {
        for (raw_hwnd, saved) in self.fullscreened.drain() {
            let hwnd = windows::Win32::Foundation::HWND(raw_hwnd as *mut _);
            let _ = win::restore_window(hwnd, &saved);
        }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let run_result = run_app(&mut terminal, &mut app);

    app.restore_all();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    run_result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') => break,
                    KeyCode::Up => app.previous(),
                    KeyCode::Down => app.next(),
                    KeyCode::Char('r') => app.refresh(),
                    KeyCode::Enter => app.toggle_fullscreen(),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn draw(f: &mut ratatui::Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(f.area());

    let items: Vec<ListItem> = app
        .windows
        .iter()
        .map(|w| {
            let is_full = app.fullscreened.contains_key(&(w.hwnd.0 as isize));
            let tag = if is_full { "[FULL] " } else { "" };
            let line = Line::from(vec![
                Span::styled(
                    tag,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(w.title.clone()),
                Span::styled(
                    format!("  —  {}", w.process_name),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!(" Windows ({}) ", app.windows.len());
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(
            Style::default()
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    let status = Paragraph::new(app.status.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Status "));
    f.render_widget(status, chunks[1]);
}
