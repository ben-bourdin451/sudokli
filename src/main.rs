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

use generator::{Difficulty, generate_killer_puzzle, generate_puzzle};
use grid::{CageRenderInfo, Cage, GameMode, Grid, PuzzleState, compute_cage_render_info};

const MENU_ITEMS: &[&str] = &["Difficulty", "Mode", "New Puzzle", "Play", "Quit"];

const CAGE_PALETTE: [Color; 6] = [
    Color::Rgb(60, 60, 90),  // navy
    Color::Rgb(70, 50, 70),  // plum
    Color::Rgb(45, 70, 60),  // teal
    Color::Rgb(80, 65, 45),  // amber
    Color::Rgb(55, 55, 55),  // gray
    Color::Rgb(70, 45, 55),  // rose
];

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
    game_mode: GameMode,
    cages: Option<Vec<Cage>>,
    cage_render: Option<CageRenderInfo>,
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
            game_mode: GameMode::default(),
            cages: None,
            cage_render: None,
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
        let PuzzleState { grid, givens, cages } = match self.game_mode {
            GameMode::Classic => generate_puzzle(self.difficulty, &mut rng),
            GameMode::Killer => generate_killer_puzzle(self.difficulty, &mut rng),
        };
        self.grid = grid;
        self.givens = givens;
        self.cage_render = cages.as_ref().map(|c| compute_cage_render_info(c));
        self.cages = cages;
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
                    1 => format!("[M]ode: {}", self.game_mode),
                    2 => "[N]ew Puzzle".to_string(),
                    3 => "[P]lay".to_string(),
                    4 => "[Q]uit".to_string(),
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

        // Grid rows 0-8, with spacer rows inserted at box boundaries
        // Table row indices: 0,1,2 = grid 0,1,2; 3 = spacer; 4,5,6 = grid 3,4,5; 7 = spacer; 8,9,10 = grid 6,7,8
        let grid_row_indices: [Option<usize>; 11] = [
            Some(0), Some(1), Some(2), None,
            Some(3), Some(4), Some(5), None,
            Some(6), Some(7), Some(8),
        ];
        let grid_col_indices: [Option<usize>; 11] = [
            Some(0), Some(1), Some(2), None,
            Some(3), Some(4), Some(5), None,
            Some(6), Some(7), Some(8),
        ];

        let rows: Vec<Row> = grid_row_indices
            .iter()
            .map(|maybe_r| {
                if maybe_r.is_none() {
                    // Spacer row: 11 empty cells
                    return Row::new(vec![Cell::from(""); 11]).height(1);
                }
                let r = maybe_r.unwrap();

                let cells: Vec<Cell> = grid_col_indices
                    .iter()
                    .map(|maybe_c| {
                        if maybe_c.is_none() {
                            return Cell::from("");
                        }
                        let c = maybe_c.unwrap();

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
                        let bg_color: Option<Color> = if let Some(ref cr) = self.cage_render {
                            let idx = cr.cage_colors[cr.cage_map[r][c]] as usize;
                            Some(CAGE_PALETTE[idx])
                        } else if (r / 3 + c / 3) % 2 == 0 {
                            Some(Color::DarkGray)
                        } else {
                            None
                        };

                        let style = if is_cursor {
                            let mut s = Style::default().bg(Color::Yellow).fg(Color::Black);
                            if is_given {
                                s = s.add_modifier(Modifier::BOLD);
                            }
                            s
                        } else if is_hint {
                            Style::default().bg(Color::Cyan).fg(Color::Black)
                        } else if is_error {
                            let mut s = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
                            if let Some(bg) = bg_color {
                                s = s.bg(bg);
                            }
                            s
                        } else if is_given {
                            let mut s = Style::default().add_modifier(Modifier::BOLD);
                            if let Some(bg) = bg_color {
                                s = s.bg(bg);
                            }
                            s
                        } else {
                            let mut s = Style::default();
                            if let Some(bg) = bg_color {
                                s = s.bg(bg);
                            }
                            s
                        };

                        let is_label = self.cage_render.as_ref()
                            .is_some_and(|cr| cr.label_cells[cr.cage_map[r][c]] == (r, c));

                        let content = if is_label {
                            let cr = self.cage_render.as_ref().unwrap();
                            let sum = self.cages.as_ref().unwrap()[cr.cage_map[r][c]].sum;
                            Text::from(vec![
                                Line::from(Span::styled(
                                    format!("{sum}"),
                                    Style::default().add_modifier(Modifier::DIM),
                                )),
                                Line::from(text).centered(),
                            ])
                        } else {
                            Text::from(vec![
                                Line::from(""),
                                Line::from(text).centered(),
                            ])
                        };

                        Cell::from(content).style(style)
                    })
                    .collect();

                Row::new(cells).height(2)
            })
            .collect();

        // 9 data columns (width 4) + 2 spacer columns (width 1)
        let widths: Vec<Constraint> = grid_col_indices
            .iter()
            .map(|c| {
                if c.is_some() {
                    Constraint::Length(4)
                } else {
                    Constraint::Length(1)
                }
            })
            .collect();

        let border_style = if is_playing {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .title(if self.game_mode == GameMode::Killer { " Killer " } else { " Sudoku " })
                    .borders(Borders::ALL)
                    .border_style(border_style),
            )
            .column_spacing(0);

        // Center the table in the panel
        // 9 data cols * 4 + 2 spacer cols * 1 + 2 border
        let table_width = 4 * 9 + 2 + 2; // 9 data cols * 4 + 2 spacers + 2 border
        let table_height = 2 * 9 + 2 + 2; // 9 data rows * 2 + 2 spacers + 2 border

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
            KeyCode::Char('m') => self.game_mode = self.game_mode.next(),
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
                1 => self.game_mode = self.game_mode.next(),
                2 => self.generate_puzzle(),
                3 => self.enter_play_mode(),
                4 => self.running = false,
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
