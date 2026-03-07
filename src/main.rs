mod generator;
mod grid;
mod solver;

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::SeedableRng;
use rand::rngs::StdRng;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Table},
};

use generator::{Difficulty, generate_puzzle};
use grid::{Grid, PuzzleState};

const MENU_ITEMS: &[&str] = &["Difficulty", "New Puzzle", "Play", "Quit"];

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Menu,
    Playing,
}

struct App {
    grid: Grid,
    givens: [[bool; 9]; 9],
    running: bool,
    difficulty: Difficulty,
    menu_index: usize,
    has_puzzle: bool,
    mode: Mode,
    cursor_row: usize,
    cursor_col: usize,
    hints: Vec<(usize, usize, u8)>,
    hint_index: usize,
    show_errors: bool,
}

impl App {
    fn new() -> Self {
        Self {
            grid: Grid::empty(),
            givens: [[false; 9]; 9],
            running: true,
            difficulty: Difficulty::default(),
            menu_index: 0,
            has_puzzle: false,
            mode: Mode::default(),
            cursor_row: 0,
            cursor_col: 0,
            hints: Vec::new(),
            hint_index: 0,
            show_errors: false,
        }
    }

    fn refresh_hints(&mut self) {
        self.hints.clear();
        for r in 0..9 {
            for c in 0..9 {
                if self.grid.get(r, c) == 0 && !self.givens[r][c] {
                    let cands = self.grid.candidates(r, c);
                    if cands.len() == 1 {
                        self.hints.push((r, c, cands[0]));
                    }
                }
            }
        }
        if self.hints.is_empty() {
            self.hint_index = 0;
        } else {
            self.hint_index = self.hint_index.min(self.hints.len() - 1);
        }
    }

    fn error_count(&self) -> usize {
        (0..9)
            .flat_map(|r| (0..9).map(move |c| (r, c)))
            .filter(|&(r, c)| {
                let val = self.grid.get(r, c);
                val != 0 && !self.givens[r][c] && !self.grid.is_valid_placement(r, c, val)
            })
            .count()
    }

    fn generate_puzzle(&mut self) {
        let mut rng = StdRng::from_os_rng();
        let PuzzleState { grid, givens } = generate_puzzle(self.difficulty, &mut rng);
        self.grid = grid;
        self.givens = givens;
        self.has_puzzle = true;
    }

    fn enter_play_mode(&mut self) {
        if !self.has_puzzle {
            self.generate_puzzle();
        }
        self.mode = Mode::Playing;
        self.refresh_hints();
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while self.running {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        let area = frame.area();

        // Vertical split: main area + bottom status line
        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        // Main layout: left panel | right panel
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)])
            .split(outer[0]);

        self.draw_menu(frame, chunks[0]);
        self.draw_grid_panel(frame, chunks[1]);
        self.draw_status_line(frame, outer[1]);
    }

    fn draw_status_line(&self, frame: &mut Frame, area: Rect) {
        let text = match self.mode {
            Mode::Menu => "  ↑↓ Navigate  Enter Select  q Quit",
            Mode::Playing => "  ↑↓←→ Move  1-9 Set  Bksp Clear  ? Hint  Tab Fill  e Errors  q Menu",
        };
        let line = Line::from(Span::styled(
            text,
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(Paragraph::new(line), area);
    }

    fn draw_menu(&self, frame: &mut Frame, area: Rect) {
        let block = Block::default()
            .title(" Sudokli ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        // Split inner area for progress bar at bottom when playing
        let (content_area, progress_area) = if self.mode == Mode::Playing {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(inner);
            (chunks[0], Some(chunks[1]))
        } else {
            (inner, None)
        };

        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(""));

        if self.mode == Mode::Playing {
            // Play mode: show hints and errors only (no menu items)

            // Hints section
            lines.push(Line::from(Span::styled(
                "── Hints ──────────────",
                Style::default().fg(Color::DarkGray),
            )));
            if self.hints.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No obvious moves",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    format!("  {} cells with 1 option", self.hints.len()),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
                let (r, c, val) = self.hints[self.hint_index];
                lines.push(Line::from(Span::styled(
                    format!("  ▸ R{}C{} → {}", r + 1, c + 1, val),
                    Style::default().fg(Color::Cyan),
                )));
            }

            // Error count
            let errors = self.error_count();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "── Errors ─────────────",
                Style::default().fg(Color::DarkGray),
            )));
            let (error_text, error_color) = if errors == 0 {
                ("  Errors: 0".to_string(), Color::Green)
            } else {
                (format!("  Errors: {errors}"), Color::Red)
            };
            lines.push(Line::from(Span::styled(
                error_text,
                Style::default().fg(error_color),
            )));
        } else {
            // Menu mode: show menu items
            for (i, &item) in MENU_ITEMS.iter().enumerate() {
                let marker = if i == self.menu_index { "> " } else { "  " };
                let label = match i {
                    0 => format!("[D]ifficulty: {}", self.difficulty),
                    1 => "[N]ew Puzzle".to_string(),
                    2 => "[P]lay".to_string(),
                    3 => "[Q]uit".to_string(),
                    _ => item.to_string(),
                };

                let style = if i == self.menu_index {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                lines.push(Line::from(Span::styled(format!("{marker}{label}"), style)));
                lines.push(Line::from(""));
            }
        }

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, content_area);

        // Progress bar (play mode only, 1 row)
        if let Some(progress_area) = progress_area {
            let filled = (0..9)
                .flat_map(|r| (0..9).map(move |c| (r, c)))
                .filter(|&(r, c)| self.grid.get(r, c) != 0)
                .count();
            let ratio = filled as f64 / 81.0;
            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Green).bg(Color::DarkGray))
                .ratio(ratio)
                .label(format!("{filled}/81"));
            frame.render_widget(gauge, progress_area);
        }
    }

    fn draw_grid_panel(&self, frame: &mut Frame, area: Rect) {
        if !self.has_puzzle {
            let block = Block::default().borders(Borders::ALL);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let msg = Paragraph::new(Text::from("Press 'n' to generate a puzzle").centered())
                .style(Style::default().fg(Color::DarkGray));

            // Center vertically
            let [centered] = Layout::vertical([Constraint::Length(1)])
                .flex(ratatui::layout::Flex::Center)
                .areas(inner);
            frame.render_widget(msg, centered);
            return;
        }

        let is_playing = self.mode == Mode::Playing;

        let rows: Vec<Row> = (0..9)
            .map(|r| {
                let cells: Vec<Cell> = (0..9)
                    .map(|c| {
                        let val = self.grid.get(r, c);
                        let text = if val == 0 {
                            " ".to_string()
                        } else {
                            val.to_string()
                        };

                        let is_cursor = is_playing && r == self.cursor_row && c == self.cursor_col;
                        let is_hint = is_playing
                            && !self.hints.is_empty()
                            && {
                                let (hr, hc, _) = self.hints[self.hint_index];
                                hr == r && hc == c
                            };
                        let is_given = self.givens[r][c];
                        let is_error = self.show_errors
                            && val != 0
                            && !is_given
                            && !self.grid.is_valid_placement(r, c, val);
                        let box_shaded = (r / 3 + c / 3) % 2 == 0;

                        let style = if is_cursor {
                            let mut s = Style::default().bg(Color::Yellow).fg(Color::Black);
                            if is_given {
                                s = s.add_modifier(Modifier::BOLD);
                            }
                            s
                        } else if is_hint {
                            Style::default().bg(Color::Cyan).fg(Color::Black)
                        } else if is_error {
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                        } else if is_given {
                            let mut s = Style::default().add_modifier(Modifier::BOLD);
                            if box_shaded {
                                s = s.bg(Color::DarkGray);
                            }
                            s
                        } else if box_shaded {
                            Style::default().bg(Color::DarkGray)
                        } else {
                            Style::default()
                        };

                        Cell::from(Text::from(text).centered()).style(style)
                    })
                    .collect();

                Row::new(cells).height(2)
            })
            .collect();

        let widths = [Constraint::Length(4); 9];

        let border_style = if is_playing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .title(" Sudoku ")
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .column_spacing(0);

        // Center the table in the panel
        let table_width = 4 * 9 + 2;
        let table_height = 2 * 9 + 2;

        let [vert] = Layout::vertical([Constraint::Length(table_height)])
            .flex(ratatui::layout::Flex::Center)
            .areas(area);
        let [centered] = Layout::horizontal([Constraint::Length(table_width)])
            .flex(ratatui::layout::Flex::Center)
            .areas(vert);

        frame.render_widget(table, centered);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match self.mode {
                Mode::Menu => self.handle_menu_input(key.code),
                Mode::Playing => self.handle_play_input(key.code),
            }
        }
        Ok(())
    }

    fn handle_menu_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('d') => self.difficulty = self.difficulty.next(),
            KeyCode::Char('n') => self.generate_puzzle(),
            KeyCode::Char('p') => self.enter_play_mode(),
            KeyCode::Up => {
                if self.menu_index > 0 {
                    self.menu_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.menu_index < MENU_ITEMS.len() - 1 {
                    self.menu_index += 1;
                }
            }
            KeyCode::Enter => match self.menu_index {
                0 => self.difficulty = self.difficulty.next(),
                1 => self.generate_puzzle(),
                2 => self.enter_play_mode(),
                3 => self.running = false,
                _ => {}
            },
            _ => {}
        }
    }

    fn handle_play_input(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Menu,
            KeyCode::Up => {
                if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                }
            }
            KeyCode::Down => {
                if self.cursor_row < 8 {
                    self.cursor_row += 1;
                }
            }
            KeyCode::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_col < 8 {
                    self.cursor_col += 1;
                }
            }
            KeyCode::Char(c @ '1'..='9') => {
                if !self.givens[self.cursor_row][self.cursor_col] {
                    let val = c as u8 - b'0';
                    self.grid.set(self.cursor_row, self.cursor_col, val);
                    self.refresh_hints();
                }
            }
            KeyCode::Char('0') | KeyCode::Backspace | KeyCode::Delete => {
                if !self.givens[self.cursor_row][self.cursor_col] {
                    self.grid.set(self.cursor_row, self.cursor_col, 0);
                    self.refresh_hints();
                }
            }
            KeyCode::Char('?') => {
                if !self.hints.is_empty() {
                    self.hint_index = (self.hint_index + 1) % self.hints.len();
                    let (r, c, _) = self.hints[self.hint_index];
                    self.cursor_row = r;
                    self.cursor_col = c;
                }
            }
            KeyCode::Char('e') => self.show_errors = !self.show_errors,
            KeyCode::Tab => {
                if let Some(&(r, c, val)) = self.hints.get(self.hint_index) {
                    self.grid.set(r, c, val);
                    self.refresh_hints();
                }
            }
            _ => {}
        }
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    ratatui::restore();
    result
}
