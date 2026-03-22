mod generator;
mod grid;
mod rater;
mod solver;

use std::fs;
use std::io::{self, Write as _};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
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

use generator::{Difficulty, KILLER_TOTAL_ATTEMPTS, generate_killer_puzzle, generate_puzzle};
use grid::{CageRenderInfo, Cage, GameMode, Grid, PuzzleState, compute_cage_render_info};

#[derive(Parser)]
#[command(name = "sudokli", version, about = "Terminal-based sudoku puzzle game")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Generate puzzles in batch and write to a JSON file
    Generate {
        /// Puzzle mode (classic or killer)
        mode: GameMode,
        /// Puzzle difficulty
        #[arg(short, long, default_value = "easy")]
        difficulty: Difficulty,
        /// Number of puzzles to generate
        #[arg(short, long, default_value_t = 1)]
        count: usize,
        /// Output file path
        #[arg(long, default_value = "puzzles.json")]
        output: String,
    },
}

#[derive(serde::Serialize)]
struct PuzzleOutput {
    mode: String,
    difficulty: String,
    rating: u8,
    solution: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    grid: Option<[[u8; 9]; 9]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cages: Option<Vec<CageOutput>>,
}

#[derive(serde::Serialize)]
struct CageOutput {
    cells: Vec<[usize; 2]>,
    sum: u8,
}

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

enum GenerationStatus {
    Idle,
    Generating {
        rx: mpsc::Receiver<Result<PuzzleState, generator::GenerationError>>,
        started: Instant,
        progress: Arc<AtomicU32>,
        total: u32,
    },
}

struct App {
    grid: Grid,
    givens: [[bool; 9]; 9],
    solution: Grid,
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
    obvious_hints: Vec<(usize, usize, u8)>,
    all_hints: Vec<(usize, usize, u8)>,
    hint_index: usize,
    show_non_obvious: bool,
    show_errors: bool,
    generation_status: GenerationStatus,
    generation_error: Option<String>,
}

impl App {
    fn new() -> Self {
        Self {
            grid: Grid::empty(),
            givens: [[false; 9]; 9],
            solution: Grid::empty(),
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
            obvious_hints: Vec::new(),
            all_hints: Vec::new(),
            hint_index: 0,
            show_non_obvious: false,
            show_errors: false,
            generation_status: GenerationStatus::Idle,
            generation_error: None,
        }
    }

    fn refresh_hints(&mut self) {
        self.obvious_hints.clear();
        self.all_hints.clear();
        self.show_non_obvious = false;

        // Build cell-to-cage lookup for killer mode
        let cage_lookup: Option<[[Option<usize>; 9]; 9]> = self.cages.as_ref().map(|cages| {
            let mut lookup = [[None; 9]; 9];
            for (i, cage) in cages.iter().enumerate() {
                for &(r, c) in &cage.cells {
                    lookup[r][c] = Some(i);
                }
            }
            lookup
        });

        for r in 0..9 {
            for c in 0..9 {
                if self.grid.get(r, c) == 0 && !self.givens[r][c] {
                    let val = self.solution.get(r, c);
                    self.all_hints.push((r, c, val));

                    let mut cands = self.grid.candidates(r, c);

                    // Further constrain by cage if in killer mode
                    if let Some(ref lookup) = cage_lookup
                        && let Some(ci) = lookup[r][c]
                    {
                        let cage = &self.cages.as_ref().unwrap()[ci];
                        let mut used = 0u16;
                        let mut filled_sum: u8 = 0;
                        let mut empty_count: usize = 0;
                        for &(cr, cc) in &cage.cells {
                            let v = self.grid.get(cr, cc);
                            if v != 0 {
                                used |= 1 << v;
                                filled_sum += v;
                            } else {
                                empty_count += 1;
                            }
                        }
                        let remaining_sum = cage.sum - filled_sum;
                        cands.retain(|&d| {
                            used & (1 << d) == 0
                                && (empty_count > 1 || d == remaining_sum)
                        });
                    }

                    if cands.len() == 1 {
                        self.obvious_hints.push((r, c, val));
                    }
                }
            }
        }
        self.hint_index = 0;
    }

    fn active_hints(&self) -> &[(usize, usize, u8)] {
        if self.show_non_obvious {
            &self.all_hints
        } else {
            &self.obvious_hints
        }
    }

    fn error_count(&self) -> usize {
        (0..9)
            .flat_map(|r| (0..9).map(move |c| (r, c)))
            .filter(|&(r, c)| {
                let val = self.grid.get(r, c);
                val != 0 && !self.givens[r][c] && val != self.solution.get(r, c)
            })
            .count()
    }

    fn is_generating(&self) -> bool {
        matches!(self.generation_status, GenerationStatus::Generating { .. })
    }

    fn start_generation(&mut self) {
        self.generation_error = None;
        let difficulty = self.difficulty;
        let game_mode = self.game_mode;
        let progress = Arc::new(AtomicU32::new(0));
        let progress_clone = Arc::clone(&progress);

        let total = match game_mode {
            GameMode::Classic => 1,
            GameMode::Killer => KILLER_TOTAL_ATTEMPTS,
        };

        let (tx, rx) = mpsc::channel();
        self.generation_status = GenerationStatus::Generating {
            rx,
            started: Instant::now(),
            progress,
            total,
        };

        std::thread::spawn(move || {
            let mut rng = StdRng::from_os_rng();
            let result = match game_mode {
                GameMode::Classic => Ok(generate_puzzle(difficulty, &mut rng)),
                GameMode::Killer => generate_killer_puzzle(difficulty, &mut rng, &progress_clone),
            };
            let _ = tx.send(result);
        });
    }

    fn check_generation(&mut self) {
        let received = match &self.generation_status {
            GenerationStatus::Idle => return,
            GenerationStatus::Generating { rx, .. } => rx.try_recv().ok(),
        };

        if let Some(result) = received {
            self.generation_status = GenerationStatus::Idle;
            match result {
                Ok(PuzzleState { grid, givens, cages, solution }) => {
                    self.grid = grid;
                    self.givens = givens;
                    self.solution = solution;
                    self.cage_render = cages.as_ref().map(|c| compute_cage_render_info(c));
                    self.cages = cages;
                    self.has_puzzle = true;
                    self.generation_error = None;
                }
                Err(e) => {
                    self.generation_error = Some(e.to_string());
                }
            }
        }
    }

    fn cancel_generation(&mut self) {
        // Drop the receiver — the background thread will see a send error and exit
        self.generation_status = GenerationStatus::Idle;
    }

    fn enter_play_mode(&mut self) {
        if !self.has_puzzle && !self.is_generating() {
            self.start_generation();
        }
        if self.has_puzzle {
            self.mode = Mode::Playing;
            self.refresh_hints();
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while self.running {
            self.check_generation();
            terminal.draw(|frame| self.draw(frame))?;

            // Use poll-based event reading so we can check generation status
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match self.mode {
                    Mode::Menu => self.handle_menu_input(key.code),
                    Mode::Playing => self.handle_play_input(key.code),
                }
            }
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
        let text = if self.is_generating() {
            "  Generating...  Esc Cancel"
        } else {
            match self.mode {
                Mode::Menu => "  ↑↓ Navigate  Enter Select  q Quit",
                Mode::Playing => "  ↑↓←→ Move  1-9 Set  Bksp Clear  ? Hint  Tab Fill  e Errors  q Menu",
            }
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
            let active = self.active_hints();
            if active.is_empty() && self.all_hints.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No unsolved cells",
                    Style::default().fg(Color::DarkGray),
                )));
            } else if active.is_empty() {
                lines.push(Line::from(Span::styled(
                    "  No obvious moves",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(Span::styled(
                    format!("  {} unsolved cells", self.all_hints.len()),
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(Span::styled(
                    "  Press ? for solution hint",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                if self.show_non_obvious {
                    lines.push(Line::from(Span::styled(
                        format!("  {} unsolved cells", active.len()),
                        Style::default().fg(Color::DarkGray),
                    )));
                } else {
                    lines.push(Line::from(Span::styled(
                        format!("  {} cells with 1 option", active.len()),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                lines.push(Line::from(""));
                let (r, c, val) = active[self.hint_index];
                let label = if self.show_non_obvious { "solution" } else { "hint" };
                lines.push(Line::from(Span::styled(
                    format!("  ▸ R{}C{} → {} ({})", r + 1, c + 1, val, label),
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

            // Show generation error if any
            if let Some(ref err) = self.generation_error {
                lines.push(Line::from(Span::styled(
                    format!("  Error: {err}"),
                    Style::default().fg(Color::Red),
                )));
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
        if let GenerationStatus::Generating {
            started,
            progress,
            total,
            ..
        } = &self.generation_status
        {
            let block = Block::default().borders(Borders::ALL);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let elapsed = started.elapsed();
            let secs = elapsed.as_secs();
            let timer_text = format!("{secs}.{}s", elapsed.subsec_millis() / 100);

            let current = progress.load(Ordering::Relaxed);
            let total = *total;
            let ratio = if total > 0 {
                (current as f64 / total as f64).min(1.0)
            } else {
                0.0
            };

            let content_height = 5; // text + gap + gauge + gap + hint
            let [vert] = Layout::vertical([Constraint::Length(content_height)])
                .flex(ratatui::layout::Flex::Center)
                .areas(inner);
            let chunks = Layout::vertical([
                Constraint::Length(1), // "Generating..."
                Constraint::Length(1), // spacer
                Constraint::Length(1), // gauge
                Constraint::Length(1), // spacer
                Constraint::Length(1), // timer
            ])
            .split(vert);

            // Center horizontally (gauge width 30)
            let gauge_width = 30u16;
            let [gauge_area] = Layout::horizontal([Constraint::Length(gauge_width)])
                .flex(ratatui::layout::Flex::Center)
                .areas(chunks[2]);

            let msg = Paragraph::new(Text::from("Generating...").centered())
                .style(Style::default().fg(Color::Yellow));
            frame.render_widget(msg, chunks[0]);

            let gauge = Gauge::default()
                .gauge_style(Style::default().fg(Color::Yellow).bg(Color::DarkGray))
                .ratio(ratio)
                .label(format!("{current}/{total}"));
            frame.render_widget(gauge, gauge_area);

            let timer = Paragraph::new(
                Text::from(format!("Elapsed: {timer_text}")).centered(),
            )
            .style(Style::default().fg(Color::DarkGray));
            frame.render_widget(timer, chunks[4]);

            return;
        }

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
                        let active = self.active_hints();
                        let is_hint = is_playing
                            && !active.is_empty()
                            && {
                                let (hr, hc, _) = active[self.hint_index];
                                hr == r && hc == c
                            };
                        let is_given = self.givens[r][c];
                        let is_error = self.show_errors
                            && val != 0
                            && !is_given
                            && val != self.solution.get(r, c);
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

    fn handle_menu_input(&mut self, code: KeyCode) {
        // During generation, only allow Escape to cancel
        if self.is_generating() {
            if matches!(code, KeyCode::Esc) {
                self.cancel_generation();
            }
            return;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('d') => self.difficulty = self.difficulty.next(),
            KeyCode::Char('m') => self.game_mode = self.game_mode.next(),
            KeyCode::Char('n') => self.start_generation(),
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
                2 => self.start_generation(),
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
                let len = self.active_hints().len();
                if len > 0 {
                    self.hint_index = (self.hint_index + 1) % len;
                    let (r, c, _) = self.active_hints()[self.hint_index];
                    self.cursor_row = r;
                    self.cursor_col = c;
                } else if !self.show_non_obvious && !self.all_hints.is_empty() {
                    // No obvious hints — switch to all hints
                    self.show_non_obvious = true;
                    self.hint_index = 0;
                    let (r, c, _) = self.all_hints[0];
                    self.cursor_row = r;
                    self.cursor_col = c;
                }
            }
            KeyCode::Char('e') => self.show_errors = !self.show_errors,
            KeyCode::Tab => {
                if let Some(&(r, c, val)) = self.active_hints().get(self.hint_index) {
                    self.grid.set(r, c, val);
                    self.refresh_hints();
                }
            }
            _ => {}
        }
    }
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Command::Generate {
            mode,
            difficulty,
            count,
            output,
        }) => run_batch_generate(mode, difficulty, count, &output),
        None => {
            let mut terminal = ratatui::init();
            let result = App::new().run(&mut terminal);
            ratatui::restore();
            result
        }
    }
}

fn run_batch_generate(
    mode: GameMode,
    difficulty: Difficulty,
    count: usize,
    output: &str,
) -> io::Result<()> {
    let mut rng = StdRng::from_os_rng();
    let mut puzzles: Vec<PuzzleOutput> = Vec::with_capacity(count);
    let mode_str = mode.to_string().to_lowercase();
    let diff_str = difficulty.to_string().to_lowercase();

    for i in 0..count {
        let label = format!(
            "[{}/{}] Generating {} {} puzzle...",
            i + 1,
            count,
            mode_str,
            diff_str
        );
        let start = Instant::now();

        match mode {
            GameMode::Classic => {
                print!("{label}");
                io::stdout().flush()?;
                let state = generate_puzzle(difficulty, &mut rng);
                let elapsed = start.elapsed();
                println!(" done ({:.1}s)", elapsed.as_secs_f64());
                puzzles.push(puzzle_to_output(&state, &mode_str, &diff_str));
            }
            GameMode::Killer => {
                let max_retries = 3;
                let mut succeeded = false;

                for retry in 0..=max_retries {
                    if retry > 0 {
                        print!(
                            "[{}/{}] Generating {} {} puzzle... retrying ({}/{})",
                            i + 1,
                            count,
                            mode_str,
                            diff_str,
                            retry,
                            max_retries
                        );
                        io::stdout().flush()?;
                    } else {
                        print!("{label}");
                        io::stdout().flush()?;
                    }

                    let progress = Arc::new(AtomicU32::new(0));
                    let progress_monitor = Arc::clone(&progress);
                    let monitor_label = format!(
                        "[{}/{}] Generating {} {} puzzle...",
                        i + 1,
                        count,
                        mode_str,
                        diff_str
                    );

                    let done_flag = Arc::new(AtomicU32::new(0));
                    let done_monitor = Arc::clone(&done_flag);

                    let monitor = std::thread::spawn(move || {
                        loop {
                            std::thread::sleep(Duration::from_millis(500));
                            if done_monitor.load(Ordering::Relaxed) != 0 {
                                break;
                            }
                            let current = progress_monitor.load(Ordering::Relaxed);
                            print!(
                                "\r{} attempt {}/{}",
                                monitor_label, current, KILLER_TOTAL_ATTEMPTS
                            );
                            let _ = io::stdout().flush();
                        }
                    });

                    let result = generate_killer_puzzle(difficulty, &mut rng, &progress);
                    done_flag.store(1, Ordering::Relaxed);
                    let _ = monitor.join();

                    match result {
                        Ok(state) => {
                            let elapsed = start.elapsed();
                            println!(
                                "\r{} done ({:.1}s)          ",
                                label,
                                elapsed.as_secs_f64()
                            );
                            puzzles.push(puzzle_to_output(&state, &mode_str, &diff_str));
                            succeeded = true;
                            break;
                        }
                        Err(_) => {
                            if retry == max_retries {
                                println!(
                                    "\r{} failed (skipped after {} retries)          ",
                                    label, max_retries
                                );
                            } else {
                                print!("\r{} failed (retrying)          \n", label);
                            }
                        }
                    }
                }

                if !succeeded {
                    continue;
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&puzzles)
        .map_err(io::Error::other)?;
    fs::write(output, json)?;
    println!("Wrote {} puzzles to {}", puzzles.len(), output);
    Ok(())
}

fn puzzle_to_output(state: &PuzzleState, mode: &str, difficulty: &str) -> PuzzleOutput {
    let rating = if let Some(ref cages) = state.cages {
        rater::rate_killer_puzzle(&grid::Grid::empty(), cages)
    } else {
        rater::rate_puzzle(&state.grid)
    };

    let cages = state.cages.as_ref().map(|cages| {
        cages
            .iter()
            .map(|cage| CageOutput {
                cells: cage.cells.iter().map(|&(r, c)| [r, c]).collect(),
                sum: cage.sum,
            })
            .collect()
    });

    // For killer mode the grid is always empty — omit it
    let grid = if state.cages.is_some() {
        None
    } else {
        Some(*state.grid.cells())
    };

    let solution: String = (0..9)
        .flat_map(|r| (0..9).map(move |c| state.solution.get(r, c)))
        .map(|d| char::from(b'0' + d))
        .collect();

    PuzzleOutput {
        mode: mode.to_string(),
        difficulty: difficulty.to_string(),
        rating: rating.score,
        solution,
        grid,
        cages,
    }
}
