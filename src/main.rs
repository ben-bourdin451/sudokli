mod generator;
mod grid;
mod solver;

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use rand::SeedableRng;
use rand::rngs::StdRng;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Flex, Layout},
    style::{Color, Style, Stylize},
    text::Text,
    widgets::{Block, Borders, Cell, Row, Table},
};

use generator::{Difficulty, generate_puzzle};
use grid::{Grid, PuzzleState};

struct App {
    grid: Grid,
    givens: [[bool; 9]; 9],
    running: bool,
}

impl App {
    fn new() -> Self {
        let mut rng = StdRng::from_os_rng();
        let PuzzleState { grid, givens } = generate_puzzle(Difficulty::Easy, &mut rng);
        Self {
            grid,
            givens,
            running: true,
        }
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
                        let mut cell = Cell::from(Text::from(text).centered());
                        // Add subtle separator coloring for 3x3 boxes
                        if (r / 3 + c / 3) % 2 == 0 {
                            cell = cell.bg(Color::DarkGray);
                        }
                        if self.givens[r][c] {
                            cell = cell.style(Style::default().bold());
                        }
                        cell
                    })
                    .collect();

                Row::new(cells).height(2)
            })
            .collect();

        let widths = [Constraint::Length(4); 9];

        let table = Table::new(rows, widths)
            .block(
                Block::default()
                    .title(" Sudoku ")
                    .borders(Borders::ALL)
                    .title_bottom(" q/Esc: quit "),
            )
            .column_spacing(0);

        // Center the table in the terminal
        let table_width = 4 * 9 + 2; // 9 columns * 4 wide + 2 for border
        let table_height = 2 * 9 + 2; // 9 rows * 2 high + 2 for border

        let [vert] = Layout::vertical([Constraint::Length(table_height)])
            .flex(Flex::Center)
            .areas(area);
        let [centered] = Layout::horizontal([Constraint::Length(table_width)])
            .flex(Flex::Center)
            .areas(vert);

        frame.render_widget(table, centered);
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.running = false,
                _ => {}
            }
        }
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = App::new().run(&mut terminal);
    ratatui::restore();
    result
}
