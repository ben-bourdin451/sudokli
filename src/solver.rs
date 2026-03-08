use std::collections::HashMap;
use std::sync::LazyLock;

use crate::grid::{Cage, Grid};

/// Lookup table: (cage_size, target_sum) -> list of valid digit bitmasks.
/// Each bitmask has bits set for the digits used (bit i = digit i, 1-indexed).
static CAGE_COMBOS: LazyLock<HashMap<(usize, u8), Vec<u16>>> = LazyLock::new(|| {
    let mut map: HashMap<(usize, u8), Vec<u16>> = HashMap::new();
    // Enumerate all non-empty subsets of digits 1-9 (bitmask 0x002..0x3FE)
    for mask in 1u16..512 {
        let bitmask = mask << 1; // shift so bit i represents digit i
        let size = bitmask.count_ones() as usize;
        let sum: u8 = (1..=9u8).filter(|&d| bitmask & (1 << d) != 0).sum();
        map.entry((size, sum)).or_default().push(bitmask);
    }
    map
});

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

// ---------------------------------------------------------------------------
// Killer-aware solver
// ---------------------------------------------------------------------------

/// Count solutions for a killer sudoku (empty grid + cage constraints).
/// Stops early once `limit` is reached. Returns a value in `0..=limit`.
pub fn count_solutions_killer(grid: &Grid, cages: &[Cage], limit: usize) -> usize {
    let Some(state) = KillerCpState::from_grid_with_cages(grid, cages) else {
        return 0;
    };
    let mut count = 0;
    solve_recursive_killer(&state, &mut count, limit);
    count
}

#[derive(Clone)]
struct KillerCageState {
    cells: Vec<(usize, usize)>,
    sum: u8,
    placed_mask: u16,
    placed_sum: u8,
    remaining: u8,
}

#[derive(Clone)]
struct KillerCpState {
    cells: [[u8; 9]; 9],
    candidates: [[u16; 9]; 9],
    empty_count: u8,
    cage_states: Vec<KillerCageState>,
    cell_cage: [[usize; 9]; 9],
}

impl KillerCpState {
    fn from_grid_with_cages(grid: &Grid, cages: &[Cage]) -> Option<Self> {
        let mut cell_cage = [[0usize; 9]; 9];
        let mut cage_states = Vec::with_capacity(cages.len());

        for (i, cage) in cages.iter().enumerate() {
            for &(r, c) in &cage.cells {
                cell_cage[r][c] = i;
            }
            cage_states.push(KillerCageState {
                cells: cage.cells.clone(),
                sum: cage.sum,
                placed_mask: 0,
                placed_sum: 0,
                remaining: cage.cells.len() as u8,
            });
        }

        let mut state = KillerCpState {
            cells: [[0u8; 9]; 9],
            candidates: [[0x3FEu16; 9]; 9], // bits 1-9
            empty_count: 0,
            cage_states,
            cell_cage,
        };

        // Count empties and copy filled cells
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

        // Initial cage candidate pruning: restrict candidates to digits that
        // appear in at least one valid combination for each cage.
        for ci in 0..state.cage_states.len() {
            let cs = &state.cage_states[ci];
            let key = (cs.cells.len(), cs.sum);
            let combos = CAGE_COMBOS.get(&key)?;
            // Union of all valid combo digits
            let valid_digits: u16 = combos.iter().copied().fold(0u16, |acc, m| acc | m);
            let cells = cs.cells.clone();
            for &(r, c) in &cells {
                if state.cells[r][c] == 0 {
                    state.candidates[r][c] &= valid_digits;
                    if state.candidates[r][c] == 0 {
                        return None;
                    }
                }
            }
        }

        // Propagate constraints from filled cells
        for r in 0..9 {
            for c in 0..9 {
                let v = grid.get(r, c);
                if v != 0 && !state.place(r, c, v) {
                    return None;
                }
            }
        }

        Some(state)
    }

    fn eliminate_from_peers(&mut self, row: usize, col: usize, d: u8) -> bool {
        let bit = 1u16 << d;
        for c in 0..9 {
            if c != col && !self.eliminate(row, c, d, bit) {
                return false;
            }
        }
        for r in 0..9 {
            if r != row && !self.eliminate(r, col, d, bit) {
                return false;
            }
        }
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

    fn eliminate(&mut self, row: usize, col: usize, d: u8, bit: u16) -> bool {
        let cands = &mut self.candidates[row][col];
        if *cands & bit == 0 {
            return true;
        }
        *cands &= !bit;
        let remaining = *cands;
        if remaining == 0 {
            return false;
        }
        if remaining & (remaining - 1) == 0 {
            let sole = remaining.trailing_zeros() as u8;
            if !self.place(row, col, sole) {
                return false;
            }
        }
        // Hidden singles in row
        if !self.check_hidden_single_row(row, d, bit) {
            return false;
        }
        if !self.check_hidden_single_col(col, d, bit) {
            return false;
        }
        if !self.check_hidden_single_box(row, col, d, bit) {
            return false;
        }
        true
    }

    fn check_hidden_single_row(&mut self, row: usize, d: u8, bit: u16) -> bool {
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
                    return true;
                }
            }
        }
        if count == 0 {
            return false;
        }
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

    fn place(&mut self, row: usize, col: usize, d: u8) -> bool {
        self.cells[row][col] = d;
        self.empty_count -= 1;
        self.candidates[row][col] = 0;

        // Standard row/col/box propagation
        if !self.eliminate_from_peers(row, col, d) {
            return false;
        }

        // Cage propagation
        let ci = self.cell_cage[row][col];
        let cs = &mut self.cage_states[ci];
        cs.placed_mask |= 1u16 << d;
        cs.placed_sum += d;
        cs.remaining -= 1;

        if cs.remaining == 0 {
            // All cells filled — verify sum
            if cs.placed_sum != cs.sum {
                return false;
            }
        } else {
            // Eliminate placed digit from other unfilled cage cells
            let cage_cells = cs.cells.clone();
            let bit = 1u16 << d;
            for &(cr, cc) in &cage_cells {
                if self.cells[cr][cc] == 0
                    && self.candidates[cr][cc] & bit != 0
                    && !self.eliminate(cr, cc, d, bit)
                {
                    return false;
                }
            }

            // Combination pruning: filter valid combos, intersect with candidates
            if !self.prune_cage_combos(ci) {
                return false;
            }
        }

        true
    }

    /// Filter cage combinations to those compatible with already-placed digits,
    /// then restrict unfilled cell candidates to the union of remaining digits.
    fn prune_cage_combos(&mut self, ci: usize) -> bool {
        let cs = &self.cage_states[ci];
        let key = (cs.cells.len(), cs.sum);
        let Some(combos) = CAGE_COMBOS.get(&key) else {
            return false;
        };

        let placed = cs.placed_mask;
        let remaining_count = cs.remaining as u32;

        // A combo is valid if it contains all placed digits and the remaining
        // digits (combo & !placed) have the right count.
        let mut valid_remaining: u16 = 0;
        for &combo in combos {
            if combo & placed != placed {
                continue; // combo missing a placed digit
            }
            let rest = combo & !placed;
            if rest.count_ones() != remaining_count {
                continue;
            }
            valid_remaining |= rest;
        }

        if valid_remaining == 0 && remaining_count > 0 {
            return false;
        }

        let cage_cells = cs.cells.clone();
        for &(cr, cc) in &cage_cells {
            if self.cells[cr][cc] == 0 {
                let old = self.candidates[cr][cc];
                let restricted = old & valid_remaining;
                if restricted == 0 {
                    return false;
                }
                // Eliminate bits that are no longer valid
                let removed = old & !restricted;
                let mut bits = removed;
                while bits != 0 {
                    let bit = bits & bits.wrapping_neg();
                    let digit = bit.trailing_zeros() as u8;
                    bits &= !bit;
                    if !self.eliminate(cr, cc, digit, bit) {
                        return false;
                    }
                }
            }
        }

        true
    }
}

fn find_mrv_cell_killer(state: &KillerCpState) -> Option<(usize, usize)> {
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
                        return best;
                    }
                }
            }
        }
    }
    best
}

fn solve_recursive_killer(state: &KillerCpState, count: &mut usize, limit: usize) {
    if *count >= limit {
        return;
    }

    if state.empty_count == 0 {
        *count += 1;
        return;
    }

    let Some((row, col)) = find_mrv_cell_killer(state) else {
        return;
    };

    let mut cands = state.candidates[row][col];
    while cands != 0 {
        let bit = cands & cands.wrapping_neg();
        let d = bit.trailing_zeros() as u8;
        cands &= !bit;

        let mut branch = state.clone();
        if branch.place(row, col, d) {
            solve_recursive_killer(&branch, count, limit);
            if *count >= limit {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{fill_grid, generate_cages};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn cage_combo_table_basic() {
        // size=2, sum=3 → only {1,2} which is bitmask 0b110 = 0x006
        let combos = CAGE_COMBOS.get(&(2, 3)).unwrap();
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0], (1 << 1) | (1 << 2));

        // size=1, sum=5 → only {5}
        let combos = CAGE_COMBOS.get(&(1, 5)).unwrap();
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0], 1 << 5);

        // size=2, sum=17 → only {8,9}
        let combos = CAGE_COMBOS.get(&(2, 17)).unwrap();
        assert_eq!(combos.len(), 1);
        assert_eq!(combos[0], (1 << 8) | (1 << 9));

        // No valid combo for impossible sum
        assert!(CAGE_COMBOS.get(&(1, 10)).is_none());
    }

    #[test]
    fn killer_trivial_one_cell_cages() {
        // 81 single-cell cages from a solved grid → exactly 1 solution
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
        let cages: Vec<Cage> = (0..9)
            .flat_map(|r| {
                (0..9).map(move |c| Cage {
                    cells: vec![(r, c)],
                    sum: cells[r][c],
                })
            })
            .collect();
        assert_eq!(count_solutions_killer(&Grid::empty(), &cages, 2), 1);
    }

    #[test]
    fn killer_solved_cage_puzzle_has_solution() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut solution = Grid::empty();
        fill_grid(&mut solution, &mut rng);
        let cages = generate_cages(&solution, &mut rng);
        let count = count_solutions_killer(&Grid::empty(), &cages, 2);
        assert!(count >= 1, "solver should find at least 1 solution");
    }

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
