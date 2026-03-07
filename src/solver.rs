use crate::grid::Grid;

/// Count solutions for the given grid, stopping early once `limit` is reached.
/// Returns a value in `0..=limit`.
pub fn count_solutions(grid: &Grid, limit: usize) -> usize {
    let Some(state) = CpState::from_grid(grid) else {
        return 0;
    };
    let mut count = 0;
    solve_recursive_cp(&state, &mut count, limit);
    count
}

#[derive(Clone)]
struct CpState {
    cells: [[u8; 9]; 9],
    candidates: [[u16; 9]; 9],
    empty_count: u8,
}

impl CpState {
    fn from_grid(grid: &Grid) -> Option<Self> {
        let mut state = CpState {
            cells: [[0u8; 9]; 9],
            candidates: [[0x3FEu16; 9]; 9], // bits 1-9 set
            empty_count: 0,
        };

        // Copy cells, count empties, zero candidates for filled cells
        for r in 0..9 {
            for c in 0..9 {
                let v = grid.get(r, c);
                if v == 0 {
                    state.empty_count += 1;
                } else {
                    state.cells[r][c] = v;
                    state.candidates[r][c] = 0;
                }
            }
        }

        // Propagate constraints from filled cells
        for r in 0..9 {
            for c in 0..9 {
                let v = grid.get(r, c);
                if v != 0 {
                    // Eliminate this digit from all peers
                    if !state.eliminate_from_peers(r, c, v) {
                        return None;
                    }
                }
            }
        }

        Some(state)
    }

    /// Eliminate digit `d` from candidates of all peers of (row, col).
    fn eliminate_from_peers(&mut self, row: usize, col: usize, d: u8) -> bool {
        let bit = 1u16 << d;

        // Row peers
        for c in 0..9 {
            if c != col && !self.eliminate(row, c, d, bit) {
                return false;
            }
        }
        // Column peers
        for r in 0..9 {
            if r != row && !self.eliminate(r, col, d, bit) {
                return false;
            }
        }
        // Box peers
        let br = (row / 3) * 3;
        let bc = (col / 3) * 3;
        for r in br..br + 3 {
            for c in bc..bc + 3 {
                if (r != row || c != col) && !self.eliminate(r, c, d, bit) {
                    return false;
                }
            }
        }

        true
    }

    /// Eliminate digit `d` (with precomputed `bit`) from cell (row, col).
    /// Returns false on contradiction.
    fn eliminate(&mut self, row: usize, col: usize, d: u8, bit: u16) -> bool {
        let cands = &mut self.candidates[row][col];
        if *cands & bit == 0 {
            return true; // already eliminated
        }
        *cands &= !bit;
        let remaining = *cands;

        if remaining == 0 {
            return false; // contradiction: no candidates left
        }

        // Naked single: exactly one candidate remains
        if remaining & (remaining - 1) == 0 {
            let sole = remaining.trailing_zeros() as u8;
            if !self.place(row, col, sole) {
                return false;
            }
        }

        // Hidden single: check each unit containing (row, col) for digit d
        // Row unit
        if !self.check_hidden_single_row(row, d, bit) {
            return false;
        }
        // Column unit
        if !self.check_hidden_single_col(col, d, bit) {
            return false;
        }
        // Box unit
        if !self.check_hidden_single_box(row, col, d, bit) {
            return false;
        }

        true
    }

    fn check_hidden_single_row(&mut self, row: usize, d: u8, bit: u16) -> bool {
        // If digit is already placed in this row, nothing to do
        for c in 0..9 {
            if self.cells[row][c] == d {
                return true;
            }
        }
        let mut count = 0u8;
        let mut last_col = 0;
        for c in 0..9 {
            if self.candidates[row][c] & bit != 0 {
                count += 1;
                last_col = c;
                if count > 1 {
                    return true; // more than one place, no hidden single
                }
            }
        }
        if count == 0 {
            return false; // contradiction: digit has no place
        }
        // count == 1: hidden single
        self.place(row, last_col, d)
    }

    fn check_hidden_single_col(&mut self, col: usize, d: u8, bit: u16) -> bool {
        for r in 0..9 {
            if self.cells[r][col] == d {
                return true;
            }
        }
        let mut count = 0u8;
        let mut last_row = 0;
        for r in 0..9 {
            if self.candidates[r][col] & bit != 0 {
                count += 1;
                last_row = r;
                if count > 1 {
                    return true;
                }
            }
        }
        if count == 0 {
            return false;
        }
        self.place(last_row, col, d)
    }

    fn check_hidden_single_box(&mut self, row: usize, col: usize, d: u8, bit: u16) -> bool {
        let br = (row / 3) * 3;
        let bc = (col / 3) * 3;
        for r in br..br + 3 {
            for c in bc..bc + 3 {
                if self.cells[r][c] == d {
                    return true;
                }
            }
        }
        let mut count = 0u8;
        let mut last_r = 0;
        let mut last_c = 0;
        for r in br..br + 3 {
            for c in bc..bc + 3 {
                if self.candidates[r][c] & bit != 0 {
                    count += 1;
                    last_r = r;
                    last_c = c;
                    if count > 1 {
                        return true;
                    }
                }
            }
        }
        if count == 0 {
            return false;
        }
        self.place(last_r, last_c, d)
    }

    /// Place digit `d` in cell (row, col) and propagate. Returns false on contradiction.
    fn place(&mut self, row: usize, col: usize, d: u8) -> bool {
        self.cells[row][col] = d;
        self.empty_count -= 1;
        self.candidates[row][col] = 0;
        self.eliminate_from_peers(row, col, d)
    }
}

/// Find empty cell with fewest candidates (MRV heuristic).
fn find_mrv_cell(state: &CpState) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    let mut best_count = 10u32;
    for r in 0..9 {
        for c in 0..9 {
            let cands = state.candidates[r][c];
            if state.cells[r][c] == 0 && cands != 0 {
                let n = cands.count_ones();
                if n < best_count {
                    best_count = n;
                    best = Some((r, c));
                    if n == 2 {
                        return best; // can't do better than 2
                    }
                }
            }
        }
    }
    best
}

fn solve_recursive_cp(state: &CpState, count: &mut usize, limit: usize) {
    if *count >= limit {
        return;
    }

    if state.empty_count == 0 {
        *count += 1;
        return;
    }

    let Some((row, col)) = find_mrv_cell(state) else {
        return; // no empty cell with candidates — contradiction
    };

    let mut cands = state.candidates[row][col];
    while cands != 0 {
        let bit = cands & cands.wrapping_neg(); // lowest set bit
        let d = bit.trailing_zeros() as u8;
        cands &= !bit;

        let mut branch = state.clone();
        if branch.place(row, col, d) {
            solve_recursive_cp(&branch, count, limit);
            if *count >= limit {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solved_grid_has_one_solution() {
        let mut g = Grid::empty();
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
        for r in 0..9 {
            for c in 0..9 {
                g.set(r, c, cells[r][c]);
            }
        }
        assert_eq!(count_solutions(&g, 2), 1);
    }

    #[test]
    fn empty_grid_has_multiple_solutions() {
        let g = Grid::empty();
        assert_eq!(count_solutions(&g, 2), 2);
    }

    #[test]
    fn no_solution_for_invalid_grid() {
        // Start from a solved grid, blank out one cell, and set a conflict
        // so that cell can't be filled.
        let mut g = Grid::empty();
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
        for r in 0..9 {
            for c in 0..9 {
                g.set(r, c, cells[r][c]);
            }
        }
        // Blank (0,0) which was 5 and change (0,1) to 5 — now nothing fits (0,0)
        g.set(0, 0, 0);
        g.set(0, 1, 5);
        assert_eq!(count_solutions(&g, 2), 0);
    }
}
