use crate::grid::{Cage, Grid};
use crate::solver::CAGE_COMBOS;

pub struct PuzzleRating {
    pub score: u8, // 1-10
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Technique {
    NakedSingle,    // base: 1
    HiddenSingle,   // base: 2
    CageCombination, // base: 3 (killer only)
    NakedPair,      // base: 4
    HiddenPair,     // base: 5
    PointingPair,   // base: 5
    NakedTriple,    // base: 6
    HiddenTriple,   // base: 6
    XWing,          // base: 8
    Swordfish,      // base: 9
    Backtracking,   // base: 10
}

impl Technique {
    fn base_score(self) -> u8 {
        match self {
            Technique::NakedSingle => 1,
            Technique::HiddenSingle => 2,
            Technique::CageCombination => 3,
            Technique::NakedPair => 4,
            Technique::HiddenPair => 5,
            Technique::PointingPair => 5,
            Technique::NakedTriple => 6,
            Technique::HiddenTriple => 6,
            Technique::XWing => 8,
            Technique::Swordfish => 9,
            Technique::Backtracking => 10,
        }
    }
}

/// Rate a classic sudoku puzzle (grid with givens).
pub fn rate_puzzle(grid: &Grid) -> PuzzleRating {
    let mut state = RaterState::from_grid(grid);
    solve_and_rate(&mut state, None)
}

/// Rate a killer sudoku puzzle (empty grid + cage constraints).
pub fn rate_killer_puzzle(grid: &Grid, cages: &[Cage]) -> PuzzleRating {
    let cage_info = RaterCageInfo::from_cages(cages);
    let mut state = RaterState::from_grid_with_cages(grid, &cage_info);
    solve_and_rate(&mut state, Some(&cage_info))
}

// ---------------------------------------------------------------------------
// Rater state — separate from solver, no auto-cascading
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct RaterState {
    cells: [[u8; 9]; 9],
    candidates: [[u16; 9]; 9],
    empty_count: u8,
    cage_states: Option<Vec<RaterCageState>>,
}

#[derive(Clone, Copy)]
struct RaterCageState {
    placed_mask: u16,
    placed_sum: u8,
    remaining: u8,
}

struct RaterCageInfo {
    cells: Vec<Vec<(usize, usize)>>,
    sums: Vec<u8>,
    cell_cage: [[usize; 9]; 9],
}

impl RaterCageInfo {
    fn from_cages(cages: &[Cage]) -> Self {
        let mut cell_cage = [[0usize; 9]; 9];
        let mut cells = Vec::with_capacity(cages.len());
        let mut sums = Vec::with_capacity(cages.len());

        for (i, cage) in cages.iter().enumerate() {
            for &(r, c) in &cage.cells {
                cell_cage[r][c] = i;
            }
            cells.push(cage.cells.clone());
            sums.push(cage.sum);
        }

        RaterCageInfo {
            cells,
            sums,
            cell_cage,
        }
    }

    fn num_cages(&self) -> usize {
        self.cells.len()
    }
}

impl RaterState {
    fn from_grid(grid: &Grid) -> Self {
        let mut state = RaterState {
            cells: [[0u8; 9]; 9],
            candidates: [[0x3FEu16; 9]; 9],
            empty_count: 0,
            cage_states: None,
        };

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

        // Strip candidates from peers of filled cells (no cascading)
        for r in 0..9 {
            for c in 0..9 {
                let v = grid.get(r, c);
                if v != 0 {
                    state.strip_from_peers(r, c, v);
                }
            }
        }

        state
    }

    fn from_grid_with_cages(grid: &Grid, info: &RaterCageInfo) -> Self {
        let mut cage_states = Vec::with_capacity(info.num_cages());
        for i in 0..info.num_cages() {
            cage_states.push(RaterCageState {
                placed_mask: 0,
                placed_sum: 0,
                remaining: info.cells[i].len() as u8,
            });
        }

        let mut state = RaterState {
            cells: [[0u8; 9]; 9],
            candidates: [[0x3FEu16; 9]; 9],
            empty_count: 0,
            cage_states: Some(cage_states),
        };

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

        // Initial cage candidate pruning
        for ci in 0..info.num_cages() {
            let key = (info.cells[ci].len(), info.sums[ci]);
            if let Some(combos) = CAGE_COMBOS.get(&key) {
                let valid_digits: u16 = combos.iter().copied().fold(0u16, |acc, m| acc | m);
                for &(r, c) in &info.cells[ci] {
                    if state.cells[r][c] == 0 {
                        state.candidates[r][c] &= valid_digits;
                    }
                }
            }
        }

        // Strip candidates from peers of filled cells
        for r in 0..9 {
            for c in 0..9 {
                let v = grid.get(r, c);
                if v != 0 {
                    state.strip_from_peers(r, c, v);
                    // Update cage state
                    if let Some(ref mut cs) = state.cage_states {
                        let ci = info.cell_cage[r][c];
                        cs[ci].placed_mask |= 1u16 << v;
                        cs[ci].placed_sum += v;
                        cs[ci].remaining -= 1;
                    }
                }
            }
        }

        state
    }

    /// Strip candidate bits from peers — NO cascading (no auto-placing singles).
    fn strip_from_peers(&mut self, row: usize, col: usize, d: u8) {
        let bit = 1u16 << d;
        for c in 0..9 {
            if c != col {
                self.candidates[row][c] &= !bit;
            }
        }
        for r in 0..9 {
            if r != row {
                self.candidates[r][col] &= !bit;
            }
        }
        let br = (row / 3) * 3;
        let bc = (col / 3) * 3;
        for r in br..br + 3 {
            for c in bc..bc + 3 {
                if r != row || c != col {
                    self.candidates[r][c] &= !bit;
                }
            }
        }
    }

    /// Place a digit and strip from peers (no cascading).
    fn place(&mut self, row: usize, col: usize, d: u8, cage_info: Option<&RaterCageInfo>) {
        self.cells[row][col] = d;
        self.candidates[row][col] = 0;
        self.empty_count -= 1;
        self.strip_from_peers(row, col, d);

        // Update cage state and strip placed digit from cage peers
        if let (Some(cs), Some(info)) = (&mut self.cage_states, cage_info) {
            let ci = info.cell_cage[row][col];
            cs[ci].placed_mask |= 1u16 << d;
            cs[ci].placed_sum += d;
            cs[ci].remaining -= 1;

            // Also strip this digit from other unfilled cells in the same cage
            let bit = 1u16 << d;
            for &(cr, cc) in &info.cells[ci] {
                if self.cells[cr][cc] == 0 {
                    self.candidates[cr][cc] &= !bit;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Technique implementations
// ---------------------------------------------------------------------------

/// Try to find and apply naked singles. Returns number of placements made.
fn try_naked_singles(state: &mut RaterState, cage_info: Option<&RaterCageInfo>) -> u32 {
    let mut count = 0u32;
    let mut progress = true;
    while progress {
        progress = false;
        for r in 0..9 {
            for c in 0..9 {
                let cands = state.candidates[r][c];
                if state.cells[r][c] == 0 && cands != 0 && cands & (cands - 1) == 0 {
                    let d = cands.trailing_zeros() as u8;
                    state.place(r, c, d, cage_info);
                    count += 1;
                    progress = true;
                }
            }
        }
    }
    count
}

/// Try to find and apply hidden singles. Returns number of placements made.
fn try_hidden_singles(state: &mut RaterState, cage_info: Option<&RaterCageInfo>) -> u32 {
    let mut count = 0u32;

    // Check rows
    for r in 0..9 {
        for d in 1..=9u8 {
            let bit = 1u16 << d;
            // Skip if already placed in row
            if (0..9).any(|c| state.cells[r][c] == d) {
                continue;
            }
            let positions: Vec<usize> = (0..9)
                .filter(|&c| state.candidates[r][c] & bit != 0)
                .collect();
            if positions.len() == 1 {
                let c = positions[0];
                state.place(r, c, d, cage_info);
                count += 1;
            }
        }
    }

    // Check columns
    for c in 0..9 {
        for d in 1..=9u8 {
            let bit = 1u16 << d;
            if (0..9).any(|r| state.cells[r][c] == d) {
                continue;
            }
            let positions: Vec<usize> = (0..9)
                .filter(|&r| state.candidates[r][c] & bit != 0)
                .collect();
            if positions.len() == 1 {
                let r = positions[0];
                state.place(r, c, d, cage_info);
                count += 1;
            }
        }
    }

    // Check boxes
    for br in (0..9).step_by(3) {
        for bc in (0..9).step_by(3) {
            for d in 1..=9u8 {
                let bit = 1u16 << d;
                let mut already_placed = false;
                let mut positions = Vec::new();
                for r in br..br + 3 {
                    for c in bc..bc + 3 {
                        if state.cells[r][c] == d {
                            already_placed = true;
                            break;
                        }
                        if state.candidates[r][c] & bit != 0 {
                            positions.push((r, c));
                        }
                    }
                    if already_placed {
                        break;
                    }
                }
                if !already_placed && positions.len() == 1 {
                    let (r, c) = positions[0];
                    if state.cells[r][c] == 0 {
                        state.place(r, c, d, cage_info);
                        count += 1;
                    }
                }
            }
        }
    }

    count
}

/// Try cage combination pruning (killer only). Returns number of candidates eliminated.
fn try_cage_combination_pruning(state: &mut RaterState, cage_info: &RaterCageInfo) -> u32 {
    let cage_states = match state.cage_states {
        Some(ref cs) => cs.clone(),
        None => return 0,
    };
    let mut eliminated = 0u32;

    for (ci, cs) in cage_states.iter().enumerate().take(cage_info.num_cages()) {
        if cs.remaining == 0 {
            continue;
        }

        let key = (cage_info.cells[ci].len(), cage_info.sums[ci]);
        let Some(combos) = CAGE_COMBOS.get(&key) else {
            continue;
        };

        let placed = cs.placed_mask;
        let remaining_count = cs.remaining as u32;

        let mut valid_remaining: u16 = 0;
        for &combo in combos {
            if combo & placed != placed {
                continue;
            }
            let rest = combo & !placed;
            if rest.count_ones() != remaining_count {
                continue;
            }
            valid_remaining |= rest;
        }

        for &(r, c) in &cage_info.cells[ci] {
            if state.cells[r][c] == 0 {
                let old = state.candidates[r][c];
                let restricted = old & valid_remaining;
                if restricted != old {
                    state.candidates[r][c] = restricted;
                    eliminated += (old ^ restricted).count_ones();
                }
            }
        }
    }

    eliminated
}

/// Try naked pairs in all units. Returns number of candidates eliminated.
fn try_naked_pairs(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;
    eliminated += naked_pairs_in_units(state, unit_rows());
    eliminated += naked_pairs_in_units(state, unit_cols());
    eliminated += naked_pairs_in_units(state, unit_boxes());
    eliminated
}

fn naked_pairs_in_units(state: &mut RaterState, units: Vec<Vec<(usize, usize)>>) -> u32 {
    let mut eliminated = 0u32;
    for unit in &units {
        // Find cells with exactly 2 candidates
        let pairs: Vec<(usize, usize, u16)> = unit
            .iter()
            .filter(|&&(r, c)| state.cells[r][c] == 0 && state.candidates[r][c].count_ones() == 2)
            .map(|&(r, c)| (r, c, state.candidates[r][c]))
            .collect();

        for i in 0..pairs.len() {
            for j in (i + 1)..pairs.len() {
                if pairs[i].2 == pairs[j].2 {
                    let mask = pairs[i].2;
                    // Remove these candidates from all other cells in the unit
                    for &(r, c) in unit {
                        if (r, c) != (pairs[i].0, pairs[i].1)
                            && (r, c) != (pairs[j].0, pairs[j].1)
                            && state.cells[r][c] == 0
                        {
                            let old = state.candidates[r][c];
                            let new = old & !mask;
                            if new != old {
                                state.candidates[r][c] = new;
                                eliminated += (old ^ new).count_ones();
                            }
                        }
                    }
                }
            }
        }
    }
    eliminated
}

/// Try hidden pairs in all units. Returns number of candidates eliminated.
fn try_hidden_pairs(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;
    eliminated += hidden_pairs_in_units(state, unit_rows());
    eliminated += hidden_pairs_in_units(state, unit_cols());
    eliminated += hidden_pairs_in_units(state, unit_boxes());
    eliminated
}

fn hidden_pairs_in_units(state: &mut RaterState, units: Vec<Vec<(usize, usize)>>) -> u32 {
    let mut eliminated = 0u32;
    for unit in &units {
        // For each pair of digits, check if they appear in exactly 2 cells
        for d1 in 1..=8u8 {
            let bit1 = 1u16 << d1;
            // Skip if already placed
            if unit.iter().any(|&(r, c)| state.cells[r][c] == d1) {
                continue;
            }
            let cells1: Vec<(usize, usize)> = unit
                .iter()
                .filter(|&&(r, c)| state.cells[r][c] == 0 && state.candidates[r][c] & bit1 != 0)
                .copied()
                .collect();
            if cells1.len() != 2 {
                continue;
            }

            for d2 in (d1 + 1)..=9u8 {
                let bit2 = 1u16 << d2;
                if unit.iter().any(|&(r, c)| state.cells[r][c] == d2) {
                    continue;
                }
                let cells2: Vec<(usize, usize)> = unit
                    .iter()
                    .filter(|&&(r, c)| {
                        state.cells[r][c] == 0 && state.candidates[r][c] & bit2 != 0
                    })
                    .copied()
                    .collect();
                if cells2 == cells1 {
                    // Hidden pair found: strip all other candidates from these 2 cells
                    let pair_mask = bit1 | bit2;
                    for &(r, c) in &cells1 {
                        let old = state.candidates[r][c];
                        let new = old & pair_mask;
                        if new != old {
                            state.candidates[r][c] = new;
                            eliminated += (old ^ new).count_ones();
                        }
                    }
                }
            }
        }
    }
    eliminated
}

/// Try pointing pairs/triples. Returns number of candidates eliminated.
fn try_pointing_pairs(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;

    for br in (0..9).step_by(3) {
        for bc in (0..9).step_by(3) {
            for d in 1..=9u8 {
                let bit = 1u16 << d;

                // Skip if already placed in box
                let mut placed = false;
                let mut positions = Vec::new();
                for r in br..br + 3 {
                    for c in bc..bc + 3 {
                        if state.cells[r][c] == d {
                            placed = true;
                            break;
                        }
                        if state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0 {
                            positions.push((r, c));
                        }
                    }
                    if placed {
                        break;
                    }
                }
                if placed || positions.len() < 2 {
                    continue;
                }

                // Check if all positions are in the same row
                if positions.iter().all(|&(r, _)| r == positions[0].0) {
                    let row = positions[0].0;
                    for c in 0..9 {
                        if c / 3 != bc / 3
                            && state.cells[row][c] == 0
                            && state.candidates[row][c] & bit != 0
                        {
                            state.candidates[row][c] &= !bit;
                            eliminated += 1;
                        }
                    }
                }

                // Check if all positions are in the same col
                if positions.iter().all(|&(_, c)| c == positions[0].1) {
                    let col = positions[0].1;
                    for r in 0..9 {
                        if r / 3 != br / 3
                            && state.cells[r][col] == 0
                            && state.candidates[r][col] & bit != 0
                        {
                            state.candidates[r][col] &= !bit;
                            eliminated += 1;
                        }
                    }
                }
            }
        }
    }

    eliminated
}

/// Try naked triples in all units. Returns number of candidates eliminated.
fn try_naked_triples(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;
    eliminated += naked_triples_in_units(state, unit_rows());
    eliminated += naked_triples_in_units(state, unit_cols());
    eliminated += naked_triples_in_units(state, unit_boxes());
    eliminated
}

fn naked_triples_in_units(state: &mut RaterState, units: Vec<Vec<(usize, usize)>>) -> u32 {
    let mut eliminated = 0u32;
    for unit in &units {
        let unfilled: Vec<(usize, usize, u16)> = unit
            .iter()
            .filter(|&&(r, c)| {
                state.cells[r][c] == 0 && {
                    let n = state.candidates[r][c].count_ones();
                    (2..=3).contains(&n)
                }
            })
            .map(|&(r, c)| (r, c, state.candidates[r][c]))
            .collect();

        for i in 0..unfilled.len() {
            for j in (i + 1)..unfilled.len() {
                for k in (j + 1)..unfilled.len() {
                    let union = unfilled[i].2 | unfilled[j].2 | unfilled[k].2;
                    if union.count_ones() == 3 {
                        let triple_cells = [
                            (unfilled[i].0, unfilled[i].1),
                            (unfilled[j].0, unfilled[j].1),
                            (unfilled[k].0, unfilled[k].1),
                        ];
                        for &(r, c) in unit {
                            if state.cells[r][c] == 0
                                && !triple_cells.contains(&(r, c))
                            {
                                let old = state.candidates[r][c];
                                let new = old & !union;
                                if new != old {
                                    state.candidates[r][c] = new;
                                    eliminated += (old ^ new).count_ones();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eliminated
}

/// Try hidden triples in all units. Returns number of candidates eliminated.
fn try_hidden_triples(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;
    eliminated += hidden_triples_in_units(state, unit_rows());
    eliminated += hidden_triples_in_units(state, unit_cols());
    eliminated += hidden_triples_in_units(state, unit_boxes());
    eliminated
}

fn hidden_triples_in_units(state: &mut RaterState, units: Vec<Vec<(usize, usize)>>) -> u32 {
    let mut eliminated = 0u32;
    for unit in &units {
        // Collect unplaced digits and their candidate cells
        let mut digit_cells: Vec<(u8, Vec<(usize, usize)>)> = Vec::new();
        for d in 1..=9u8 {
            if unit.iter().any(|&(r, c)| state.cells[r][c] == d) {
                continue;
            }
            let bit = 1u16 << d;
            let cells: Vec<(usize, usize)> = unit
                .iter()
                .filter(|&&(r, c)| state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0)
                .copied()
                .collect();
            if cells.len() >= 2 && cells.len() <= 3 {
                digit_cells.push((d, cells));
            }
        }

        for i in 0..digit_cells.len() {
            for j in (i + 1)..digit_cells.len() {
                for k in (j + 1)..digit_cells.len() {
                    // Union of cells for these 3 digits
                    let mut union_cells: Vec<(usize, usize)> = Vec::new();
                    for cell in digit_cells[i]
                        .1
                        .iter()
                        .chain(digit_cells[j].1.iter())
                        .chain(digit_cells[k].1.iter())
                    {
                        if !union_cells.contains(cell) {
                            union_cells.push(*cell);
                        }
                    }
                    if union_cells.len() == 3 {
                        let triple_mask = (1u16 << digit_cells[i].0)
                            | (1u16 << digit_cells[j].0)
                            | (1u16 << digit_cells[k].0);
                        for &(r, c) in &union_cells {
                            let old = state.candidates[r][c];
                            let new = old & triple_mask;
                            if new != old {
                                state.candidates[r][c] = new;
                                eliminated += (old ^ new).count_ones();
                            }
                        }
                    }
                }
            }
        }
    }
    eliminated
}

/// Try X-Wing technique. Returns number of candidates eliminated.
fn try_x_wing(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;

    for d in 1..=9u8 {
        let bit = 1u16 << d;

        // X-Wing in rows: find 2 rows where digit appears in exactly 2 columns, same columns
        let mut row_positions: Vec<(usize, Vec<usize>)> = Vec::new();
        for r in 0..9 {
            if (0..9).any(|c| state.cells[r][c] == d) {
                continue;
            }
            let cols: Vec<usize> = (0..9)
                .filter(|&c| state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0)
                .collect();
            if cols.len() == 2 {
                row_positions.push((r, cols));
            }
        }

        for i in 0..row_positions.len() {
            for j in (i + 1)..row_positions.len() {
                if row_positions[i].1 == row_positions[j].1 {
                    let r1 = row_positions[i].0;
                    let r2 = row_positions[j].0;
                    let c1 = row_positions[i].1[0];
                    let c2 = row_positions[i].1[1];
                    // Eliminate digit from these columns in all other rows
                    for r in 0..9 {
                        if r != r1 && r != r2 {
                            for &c in &[c1, c2] {
                                if state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0 {
                                    state.candidates[r][c] &= !bit;
                                    eliminated += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // X-Wing in columns: find 2 cols where digit appears in exactly 2 rows, same rows
        let mut col_positions: Vec<(usize, Vec<usize>)> = Vec::new();
        for c in 0..9 {
            if (0..9).any(|r| state.cells[r][c] == d) {
                continue;
            }
            let rows: Vec<usize> = (0..9)
                .filter(|&r| state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0)
                .collect();
            if rows.len() == 2 {
                col_positions.push((c, rows));
            }
        }

        for i in 0..col_positions.len() {
            for j in (i + 1)..col_positions.len() {
                if col_positions[i].1 == col_positions[j].1 {
                    let c1 = col_positions[i].0;
                    let c2 = col_positions[j].0;
                    let r1 = col_positions[i].1[0];
                    let r2 = col_positions[i].1[1];
                    for c in 0..9 {
                        if c != c1 && c != c2 {
                            for &r in &[r1, r2] {
                                if state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0 {
                                    state.candidates[r][c] &= !bit;
                                    eliminated += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    eliminated
}

/// Try Swordfish technique. Returns number of candidates eliminated.
fn try_swordfish(state: &mut RaterState) -> u32 {
    let mut eliminated = 0u32;

    for d in 1..=9u8 {
        let bit = 1u16 << d;

        // Swordfish in rows
        let mut row_positions: Vec<(usize, Vec<usize>)> = Vec::new();
        for r in 0..9 {
            if (0..9).any(|c| state.cells[r][c] == d) {
                continue;
            }
            let cols: Vec<usize> = (0..9)
                .filter(|&c| state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0)
                .collect();
            if cols.len() >= 2 && cols.len() <= 3 {
                row_positions.push((r, cols));
            }
        }

        for i in 0..row_positions.len() {
            for j in (i + 1)..row_positions.len() {
                for k in (j + 1)..row_positions.len() {
                    let mut col_set = 0u16;
                    for &c in row_positions[i]
                        .1
                        .iter()
                        .chain(row_positions[j].1.iter())
                        .chain(row_positions[k].1.iter())
                    {
                        col_set |= 1u16 << c;
                    }
                    if col_set.count_ones() == 3 {
                        let r1 = row_positions[i].0;
                        let r2 = row_positions[j].0;
                        let r3 = row_positions[k].0;
                        for c in 0..9 {
                            if col_set & (1 << c) != 0 {
                                for r in 0..9 {
                                    if r != r1
                                        && r != r2
                                        && r != r3
                                        && state.cells[r][c] == 0
                                        && state.candidates[r][c] & bit != 0
                                    {
                                        state.candidates[r][c] &= !bit;
                                        eliminated += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Swordfish in columns
        let mut col_positions: Vec<(usize, Vec<usize>)> = Vec::new();
        for c in 0..9 {
            if (0..9).any(|r| state.cells[r][c] == d) {
                continue;
            }
            let rows: Vec<usize> = (0..9)
                .filter(|&r| state.cells[r][c] == 0 && state.candidates[r][c] & bit != 0)
                .collect();
            if rows.len() >= 2 && rows.len() <= 3 {
                col_positions.push((c, rows));
            }
        }

        for i in 0..col_positions.len() {
            for j in (i + 1)..col_positions.len() {
                for k in (j + 1)..col_positions.len() {
                    let mut row_set = 0u16;
                    for &r in col_positions[i]
                        .1
                        .iter()
                        .chain(col_positions[j].1.iter())
                        .chain(col_positions[k].1.iter())
                    {
                        row_set |= 1u16 << r;
                    }
                    if row_set.count_ones() == 3 {
                        let c1 = col_positions[i].0;
                        let c2 = col_positions[j].0;
                        let c3 = col_positions[k].0;
                        for r in 0..9 {
                            if row_set & (1 << r) != 0 {
                                for c in 0..9 {
                                    if c != c1
                                        && c != c2
                                        && c != c3
                                        && state.cells[r][c] == 0
                                        && state.candidates[r][c] & bit != 0
                                    {
                                        state.candidates[r][c] &= !bit;
                                        eliminated += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    eliminated
}

// ---------------------------------------------------------------------------
// Unit helpers
// ---------------------------------------------------------------------------

fn unit_rows() -> Vec<Vec<(usize, usize)>> {
    (0..9).map(|r| (0..9).map(|c| (r, c)).collect()).collect()
}

fn unit_cols() -> Vec<Vec<(usize, usize)>> {
    (0..9).map(|c| (0..9).map(|r| (r, c)).collect()).collect()
}

fn unit_boxes() -> Vec<Vec<(usize, usize)>> {
    (0..3)
        .flat_map(|br| {
            (0..3).map(move |bc| {
                let r0 = br * 3;
                let c0 = bc * 3;
                (r0..r0 + 3)
                    .flat_map(|r| (c0..c0 + 3).map(move |c| (r, c)))
                    .collect()
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Main rating loop
// ---------------------------------------------------------------------------

fn solve_and_rate(state: &mut RaterState, cage_info: Option<&RaterCageInfo>) -> PuzzleRating {
    let mut max_technique = Technique::NakedSingle;
    let mut advanced_uses = 0u32;

    loop {
        if state.empty_count == 0 {
            break;
        }

        // Try techniques from easiest to hardest
        if try_naked_singles(state, cage_info) > 0 {
            // NakedSingle is already the minimum — no update needed
            continue;
        }

        if try_hidden_singles(state, cage_info) > 0 {
            if max_technique < Technique::HiddenSingle {
                max_technique = Technique::HiddenSingle;
            }
            continue;
        }

        if let Some(ci) = cage_info
            && try_cage_combination_pruning(state, ci) > 0
        {
            if max_technique < Technique::CageCombination {
                max_technique = Technique::CageCombination;
            }
            continue;
        }

        if try_naked_pairs(state) > 0 {
            if max_technique < Technique::NakedPair {
                max_technique = Technique::NakedPair;
            }
            advanced_uses += 1;
            continue;
        }

        if try_hidden_pairs(state) > 0 {
            if max_technique < Technique::HiddenPair {
                max_technique = Technique::HiddenPair;
            }
            advanced_uses += 1;
            continue;
        }

        if try_pointing_pairs(state) > 0 {
            if max_technique < Technique::PointingPair {
                max_technique = Technique::PointingPair;
            }
            advanced_uses += 1;
            continue;
        }

        if try_naked_triples(state) > 0 {
            if max_technique < Technique::NakedTriple {
                max_technique = Technique::NakedTriple;
            }
            advanced_uses += 1;
            continue;
        }

        if try_hidden_triples(state) > 0 {
            if max_technique < Technique::HiddenTriple {
                max_technique = Technique::HiddenTriple;
            }
            advanced_uses += 1;
            continue;
        }

        if try_x_wing(state) > 0 {
            if max_technique < Technique::XWing {
                max_technique = Technique::XWing;
            }
            advanced_uses += 1;
            continue;
        }

        if try_swordfish(state) > 0 {
            if max_technique < Technique::Swordfish {
                max_technique = Technique::Swordfish;
            }
            advanced_uses += 1;
            continue;
        }

        // No technique worked — need to backtrack (guess)
        max_technique = Technique::Backtracking;
        advanced_uses += 1;

        // Find MRV cell and guess
        if let Some((r, c)) = find_mrv_cell(state) {
            let cands = state.candidates[r][c];
            let d = cands.trailing_zeros() as u8; // pick lowest candidate
            state.place(r, c, d, cage_info);
        } else {
            break; // contradiction — shouldn't happen on valid puzzles
        }
    }

    let score = compute_score(max_technique, advanced_uses);
    PuzzleRating { score }
}

fn find_mrv_cell(state: &RaterState) -> Option<(usize, usize)> {
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

fn compute_score(max_technique: Technique, advanced_uses: u32) -> u8 {
    let base = max_technique.base_score();
    let freq_bonus = if advanced_uses > 5 { 1 } else { 0 };
    (base + freq_bonus).min(10)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{Difficulty, fill_grid, generate_cages, generate_puzzle};
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn easy_puzzle_rates_low() {
        let mut rng = StdRng::seed_from_u64(42);
        let state = generate_puzzle(Difficulty::Easy, &mut rng);
        let rating = rate_puzzle(&state.grid);
        assert!(
            rating.score <= 3,
            "Easy puzzle should rate <= 3, got {}",
            rating.score
        );
    }

    #[test]
    fn hard_puzzle_rates_higher_than_easy() {
        let mut rng = StdRng::seed_from_u64(42);
        let easy = generate_puzzle(Difficulty::Easy, &mut rng);
        let easy_rating = rate_puzzle(&easy.grid);

        let mut rng = StdRng::seed_from_u64(42);
        let hard = generate_puzzle(Difficulty::Hard, &mut rng);
        let hard_rating = rate_puzzle(&hard.grid);

        assert!(
            hard_rating.score >= easy_rating.score,
            "Hard ({}) should rate >= Easy ({})",
            hard_rating.score,
            easy_rating.score
        );
    }

    #[test]
    fn rating_is_deterministic() {
        let mut rng = StdRng::seed_from_u64(99);
        let state = generate_puzzle(Difficulty::Medium, &mut rng);
        let r1 = rate_puzzle(&state.grid);
        let r2 = rate_puzzle(&state.grid);
        assert_eq!(r1.score, r2.score, "Same puzzle must produce same rating");
    }

    #[test]
    fn rating_in_valid_range() {
        let mut rng = StdRng::seed_from_u64(7);
        for d in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard] {
            let state = generate_puzzle(d, &mut rng);
            let rating = rate_puzzle(&state.grid);
            assert!(
                (1..=10).contains(&rating.score),
                "Rating {} out of 1-10 range for {d:?}",
                rating.score
            );
        }
    }

    #[test]
    fn killer_puzzle_gets_rated() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut solution = Grid::empty();
        fill_grid(&mut solution, &mut rng);
        let cages = generate_cages(&solution, Difficulty::Easy, &mut rng);
        let rating = rate_killer_puzzle(&Grid::empty(), &cages);
        assert!(
            (1..=10).contains(&rating.score),
            "Killer rating {} out of range",
            rating.score
        );
    }

    #[test]
    fn solved_grid_rates_one() {
        // A fully solved grid has 0 empty cells — should rate 1 (minimum)
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
        let rating = rate_puzzle(&g);
        assert_eq!(rating.score, 1);
    }
}
