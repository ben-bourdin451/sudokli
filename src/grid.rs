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

pub struct PuzzleState {
    pub grid: Grid,
    pub givens: [[bool; 9]; 9],
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
}
