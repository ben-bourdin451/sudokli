#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    cells: [[u8; 9]; 9],
}

impl Grid {
    pub fn empty() -> Self {
        Self { cells: [[0; 9]; 9] }
    }

    pub fn get(&self, row: usize, col: usize) -> u8 {
        self.cells[row][col]
    }

    pub fn set(&mut self, row: usize, col: usize, val: u8) {
        self.cells[row][col] = val;
    }

    /// Check if placing `val` at (row, col) conflicts with any existing value
    /// in the same row, column, or 3×3 box. Does not check the cell itself.
    pub fn is_valid_placement(&self, row: usize, col: usize, val: u8) -> bool {
        // Check row
        for c in 0..9 {
            if c != col && self.cells[row][c] == val {
                return false;
            }
        }
        // Check column
        for r in 0..9 {
            if r != row && self.cells[r][col] == val {
                return false;
            }
        }
        // Check 3x3 box
        let box_r = (row / 3) * 3;
        let box_c = (col / 3) * 3;
        for r in box_r..box_r + 3 {
            for c in box_c..box_c + 3 {
                if (r, c) != (row, col) && self.cells[r][c] == val {
                    return false;
                }
            }
        }
        true
    }

    /// Returns the set of valid candidate values for an empty cell.
    /// Returns empty vec if the cell is already filled.
    pub fn candidates(&self, row: usize, col: usize) -> Vec<u8> {
        if self.cells[row][col] != 0 {
            return vec![];
        }
        (1..=9)
            .filter(|&v| self.is_valid_placement(row, col, v))
            .collect()
    }

    /// Check if the grid is completely filled with a valid solution.
    #[allow(dead_code)]
    pub fn is_complete_and_valid(&self) -> bool {
        for r in 0..9 {
            let mut seen = [false; 10];
            for c in 0..9 {
                let v = self.cells[r][c] as usize;
                if v == 0 || v > 9 || seen[v] {
                    return false;
                }
                seen[v] = true;
            }
        }
        for c in 0..9 {
            let mut seen = [false; 10];
            for r in 0..9 {
                let v = self.cells[r][c] as usize;
                if seen[v] {
                    return false;
                }
                seen[v] = true;
            }
        }
        for box_r in (0..9).step_by(3) {
            for box_c in (0..9).step_by(3) {
                let mut seen = [false; 10];
                for r in box_r..box_r + 3 {
                    for c in box_c..box_c + 3 {
                        let v = self.cells[r][c] as usize;
                        if seen[v] {
                            return false;
                        }
                        seen[v] = true;
                    }
                }
            }
        }
        true
    }
}

pub struct Cage {
    pub cells: Vec<(usize, usize)>,
    pub sum: u8,
}

pub struct CageRenderInfo {
    pub cage_map: [[usize; 9]; 9],
    pub cage_colors: Vec<u8>,
    pub label_cells: Vec<(usize, usize)>,
}

pub fn compute_cage_render_info(cages: &[Cage]) -> CageRenderInfo {
    let num_cages = cages.len();

    // Build cage_map: cell -> cage index
    let mut cage_map = [[0usize; 9]; 9];
    for (i, cage) in cages.iter().enumerate() {
        for &(r, c) in &cage.cells {
            cage_map[r][c] = i;
        }
    }

    // Build adjacency list (two cages are adjacent if any cells are orthogonal neighbors)
    let mut adj: Vec<Vec<usize>> = vec![vec![]; num_cages];
    for r in 0..9 {
        for c in 0..9 {
            let ci = cage_map[r][c];
            for (dr, dc) in [(0, 1), (1, 0)] {
                let nr = r + dr;
                let nc = c + dc;
                if nr < 9 && nc < 9 {
                    let ni = cage_map[nr][nc];
                    if ci != ni {
                        if !adj[ci].contains(&ni) {
                            adj[ci].push(ni);
                        }
                        if !adj[ni].contains(&ci) {
                            adj[ni].push(ci);
                        }
                    }
                }
            }
        }
    }

    // Greedy graph coloring
    let mut cage_colors: Vec<u8> = vec![0; num_cages];
    let mut colored = vec![false; num_cages];
    for i in 0..num_cages {
        let mut used = [false; 6];
        for &neighbor in &adj[i] {
            if colored[neighbor] {
                used[cage_colors[neighbor] as usize] = true;
            }
        }
        cage_colors[i] = used.iter().position(|&u| !u).unwrap_or(0) as u8;
        colored[i] = true;
    }

    // Find label cell per cage: top-left (min by row then col)
    let label_cells: Vec<(usize, usize)> = cages
        .iter()
        .map(|cage| {
            *cage
                .cells
                .iter()
                .min_by_key(|&&(r, c)| (r, c))
                .unwrap()
        })
        .collect();

    CageRenderInfo {
        cage_map,
        cage_colors,
        label_cells,
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum GameMode {
    #[default]
    Classic,
    Killer,
}

impl GameMode {
    pub fn next(self) -> Self {
        match self {
            GameMode::Classic => GameMode::Killer,
            GameMode::Killer => GameMode::Classic,
        }
    }
}

impl std::fmt::Display for GameMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameMode::Classic => write!(f, "Classic"),
            GameMode::Killer => write!(f, "Killer"),
        }
    }
}

pub struct PuzzleState {
    pub grid: Grid,
    pub givens: [[bool; 9]; 9],
    pub cages: Option<Vec<Cage>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solved_grid() -> Grid {
        let cells = [
            [5, 3, 4, 6, 7, 8, 9, 1, 2],
            [6, 7, 2, 1, 9, 5, 3, 4, 8],
            [1, 9, 8, 3, 4, 2, 5, 6, 7],
            [8, 5, 9, 7, 6, 1, 4, 2, 3],
            [4, 2, 6, 8, 5, 3, 7, 9, 1],
            [7, 1, 3, 9, 2, 4, 8, 5, 6],
            [9, 6, 1, 5, 3, 7, 2, 8, 4],
            [2, 8, 7, 4, 1, 9, 6, 3, 5],
            [3, 4, 5, 2, 8, 6, 1, 7, 9],
        ];
        Grid { cells }
    }

    #[test]
    fn complete_valid_grid() {
        assert!(solved_grid().is_complete_and_valid());
    }

    #[test]
    fn incomplete_grid_is_not_valid() {
        let mut g = solved_grid();
        g.set(0, 0, 0);
        assert!(!g.is_complete_and_valid());
    }

    #[test]
    fn duplicate_in_row_is_invalid() {
        let mut g = solved_grid();
        g.set(0, 0, g.get(0, 1)); // duplicate in row
        assert!(!g.is_complete_and_valid());
    }

    #[test]
    fn valid_placement_on_empty() {
        let g = Grid::empty();
        assert!(g.is_valid_placement(0, 0, 5));
    }

    #[test]
    fn invalid_placement_row_conflict() {
        let mut g = Grid::empty();
        g.set(0, 3, 5);
        assert!(!g.is_valid_placement(0, 0, 5));
    }

    #[test]
    fn invalid_placement_col_conflict() {
        let mut g = Grid::empty();
        g.set(3, 0, 5);
        assert!(!g.is_valid_placement(0, 0, 5));
    }

    #[test]
    fn invalid_placement_box_conflict() {
        let mut g = Grid::empty();
        g.set(1, 1, 5);
        assert!(!g.is_valid_placement(0, 0, 5));
    }

    #[test]
    fn candidates_empty_cell() {
        let mut g = Grid::empty();
        // Place 1-8 in row 0, cols 1-8
        for v in 1..=8 {
            g.set(0, v as usize, v);
        }
        // Only 9 is valid at (0,0)
        assert_eq!(g.candidates(0, 0), vec![9]);
    }

    #[test]
    fn candidates_filled_cell_returns_empty() {
        let g = solved_grid();
        assert_eq!(g.candidates(0, 0), vec![]);
    }

    fn sample_cages() -> Vec<Cage> {
        // Simple cages covering all 81 cells (9 row-based cages for simplicity)
        (0..9)
            .map(|r| Cage {
                cells: (0..9).map(|c| (r, c)).collect(),
                sum: 45,
            })
            .collect()
    }

    #[test]
    fn cage_render_info_covers_all_cells() {
        let cages = sample_cages();
        let info = compute_cage_render_info(&cages);
        for r in 0..9 {
            for c in 0..9 {
                assert!(info.cage_map[r][c] < cages.len());
            }
        }
    }

    #[test]
    fn cage_coloring_no_adjacent_same_color() {
        let cages = sample_cages();
        let info = compute_cage_render_info(&cages);
        for r in 0..9 {
            for c in 0..9 {
                let ci = info.cage_map[r][c];
                let color = info.cage_colors[ci];
                for (dr, dc) in [(0, 1), (1, 0)] {
                    let nr = r + dr;
                    let nc = c + dc;
                    if nr < 9 && nc < 9 {
                        let ni = info.cage_map[nr][nc];
                        if ci != ni {
                            assert_ne!(
                                color,
                                info.cage_colors[ni],
                                "Adjacent cages {ci} and {ni} share color {color}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn label_cells_belong_to_cage() {
        let cages = sample_cages();
        let info = compute_cage_render_info(&cages);
        for (i, cage) in cages.iter().enumerate() {
            let label = info.label_cells[i];
            assert!(
                cage.cells.contains(&label),
                "Label cell {label:?} not in cage {i}"
            );
        }
    }
}
