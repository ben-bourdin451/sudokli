use rand::Rng;
use rand::seq::{IndexedRandom, SliceRandom};

use crate::grid::{Cage, Grid, PuzzleState};
use crate::solver::{count_solutions, count_solutions_killer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Difficulty {
    #[default]
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    fn clue_range(self) -> (usize, usize) {
        match self {
            Difficulty::Easy => (36, 45),
            Difficulty::Medium => (27, 35),
            Difficulty::Hard => (22, 26),
        }
    }

    pub fn next(self) -> Self {
        match self {
            Difficulty::Easy => Difficulty::Medium,
            Difficulty::Medium => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Easy,
        }
    }
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "Easy"),
            Difficulty::Medium => write!(f, "Medium"),
            Difficulty::Hard => write!(f, "Hard"),
        }
    }
}

/// Generate a puzzle with a unique solution at the given difficulty.
pub fn generate_puzzle(difficulty: Difficulty, rng: &mut impl Rng) -> PuzzleState {
    let mut grid = Grid::empty();
    fill_grid(&mut grid, rng);

    let (min_clues, max_clues) = difficulty.clue_range();
    let target_clues = rng.random_range(min_clues..=max_clues);

    remove_cells(&mut grid, target_clues, rng);

    let mut givens = [[false; 9]; 9];
    for (r, row) in givens.iter_mut().enumerate() {
        for (c, given) in row.iter_mut().enumerate() {
            *given = grid.get(r, c) != 0;
        }
    }

    PuzzleState {
        grid,
        givens,
        cages: None,
    }
}

/// Fill the grid with a complete valid solution using backtracking with
/// randomized digit ordering.
pub(crate) fn fill_grid(grid: &mut Grid, rng: &mut impl Rng) -> bool {
    let Some((row, col)) = find_empty(grid) else {
        return true; // all cells filled
    };

    let mut digits: [u8; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    digits.shuffle(rng);

    for val in digits {
        if grid.is_valid_placement(row, col, val) {
            grid.set(row, col, val);
            if fill_grid(grid, rng) {
                return true;
            }
            grid.set(row, col, 0);
        }
    }
    false
}

/// Remove cells from a complete grid while maintaining a unique solution.
fn remove_cells(grid: &mut Grid, target_clues: usize, rng: &mut impl Rng) {
    let mut positions: Vec<(usize, usize)> =
        (0..9).flat_map(|r| (0..9).map(move |c| (r, c))).collect();
    positions.shuffle(rng);

    let mut clues = 81;

    for (r, c) in positions {
        if clues <= target_clues {
            break;
        }

        let val = grid.get(r, c);
        grid.set(r, c, 0);

        if count_solutions(grid, 2) != 1 {
            // Removal breaks uniqueness — restore
            grid.set(r, c, val);
        } else {
            clues -= 1;
        }
    }
}

/// Generate a killer sudoku puzzle: empty grid with cage constraints.
/// Loops until a cage partition with a unique solution is found.
pub fn generate_killer_puzzle(difficulty: Difficulty, rng: &mut impl Rng) -> PuzzleState {
    let _ = difficulty; // reserved for future difficulty tuning
    loop {
        let mut solution = Grid::empty();
        fill_grid(&mut solution, rng);

        // Try multiple cage partitions per solution before regenerating
        for _ in 0..10 {
            let cages = generate_cages(&solution, rng);
            if count_solutions_killer(&Grid::empty(), &cages, 2) == 1 {
                return PuzzleState {
                    grid: Grid::empty(),
                    givens: [[false; 9]; 9],
                    cages: Some(cages),
                };
            }
        }
    }
}

pub(crate) fn generate_cages(solution: &Grid, rng: &mut impl Rng) -> Vec<Cage> {
    let mut assigned = [[false; 9]; 9];
    let mut cages = Vec::new();

    let mut positions: Vec<(usize, usize)> =
        (0..9).flat_map(|r| (0..9).map(move |c| (r, c))).collect();
    positions.shuffle(rng);

    for (r, c) in positions {
        if assigned[r][c] {
            continue;
        }

        let target_size = pick_cage_size(rng);
        let mut cells = vec![(r, c)];
        assigned[r][c] = true;
        // Bitmask tracking digits already in this cage
        let mut digits_used: u16 = 1 << solution.get(r, c);

        while cells.len() < target_size {
            let neighbors = unassigned_neighbors(&cells, &assigned);
            // Filter to neighbors whose digit isn't already in the cage
            let valid: Vec<(usize, usize)> = neighbors
                .into_iter()
                .filter(|&(nr, nc)| {
                    let d = solution.get(nr, nc);
                    digits_used & (1 << d) == 0
                })
                .collect();

            if valid.is_empty() {
                break;
            }

            let &(nr, nc) = valid.choose(rng).unwrap();
            assigned[nr][nc] = true;
            digits_used |= 1 << solution.get(nr, nc);
            cells.push((nr, nc));
        }

        let sum: u8 = cells.iter().map(|&(cr, cc)| solution.get(cr, cc)).sum();
        cages.push(Cage { cells, sum });
    }

    cages
}

fn unassigned_neighbors(
    cells: &[(usize, usize)],
    assigned: &[[bool; 9]; 9],
) -> Vec<(usize, usize)> {
    let mut result = Vec::new();
    let mut seen = [[false; 9]; 9];
    for &(r, c) in cells {
        for (dr, dc) in [(-1i32, 0), (1, 0), (0, -1i32), (0, 1)] {
            let nr = r as i32 + dr;
            let nc = c as i32 + dc;
            if (0..9).contains(&nr) && (0..9).contains(&nc) {
                let (nr, nc) = (nr as usize, nc as usize);
                if !assigned[nr][nc] && !seen[nr][nc] {
                    seen[nr][nc] = true;
                    result.push((nr, nc));
                }
            }
        }
    }
    result
}

fn pick_cage_size(rng: &mut impl Rng) -> usize {
    let roll: u8 = rng.random_range(0..100);
    match roll {
        0..5 => 1,
        5..35 => 2,
        35..70 => 3,
        70..95 => 4,
        _ => 5,
    }
}

fn find_empty(grid: &Grid) -> Option<(usize, usize)> {
    for r in 0..9 {
        for c in 0..9 {
            if grid.get(r, c) == 0 {
                return Some((r, c));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn fill_grid_produces_valid_grid() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut grid = Grid::empty();
        assert!(fill_grid(&mut grid, &mut rng));
        assert!(grid.is_complete_and_valid());
    }

    #[test]
    fn generated_puzzle_has_unique_solution() {
        let mut rng = StdRng::seed_from_u64(123);
        let puzzle = generate_puzzle(Difficulty::Easy, &mut rng);
        assert_eq!(count_solutions(&puzzle.grid, 2), 1);
    }

    #[test]
    fn cages_partition_all_cells() {
        let mut rng = StdRng::seed_from_u64(100);
        let mut solution = Grid::empty();
        fill_grid(&mut solution, &mut rng);
        let cages = generate_cages(&solution, &mut rng);

        let mut covered = [[false; 9]; 9];
        for cage in &cages {
            for &(r, c) in &cage.cells {
                assert!(!covered[r][c], "cell ({r},{c}) covered twice");
                covered[r][c] = true;
            }
        }
        for r in 0..9 {
            for c in 0..9 {
                assert!(covered[r][c], "cell ({r},{c}) not covered");
            }
        }
    }

    #[test]
    fn cages_no_duplicate_digits() {
        let mut rng = StdRng::seed_from_u64(200);
        let mut solution = Grid::empty();
        fill_grid(&mut solution, &mut rng);
        let cages = generate_cages(&solution, &mut rng);

        for (i, cage) in cages.iter().enumerate() {
            let mut seen = 0u16;
            for &(r, c) in &cage.cells {
                let d = solution.get(r, c);
                assert!(
                    seen & (1 << d) == 0,
                    "cage {i} has duplicate digit {d}"
                );
                seen |= 1 << d;
            }
        }
    }

    #[test]
    fn cage_sums_match_solution() {
        let mut rng = StdRng::seed_from_u64(300);
        let mut solution = Grid::empty();
        fill_grid(&mut solution, &mut rng);
        let cages = generate_cages(&solution, &mut rng);

        for (i, cage) in cages.iter().enumerate() {
            let actual_sum: u8 = cage.cells.iter().map(|&(r, c)| solution.get(r, c)).sum();
            assert_eq!(
                cage.sum, actual_sum,
                "cage {i} sum mismatch: stored {} vs actual {}",
                cage.sum, actual_sum
            );
        }
    }

    #[test]
    fn cages_are_contiguous() {
        let mut rng = StdRng::seed_from_u64(400);
        let mut solution = Grid::empty();
        fill_grid(&mut solution, &mut rng);
        let cages = generate_cages(&solution, &mut rng);

        for (i, cage) in cages.iter().enumerate() {
            if cage.cells.len() <= 1 {
                continue;
            }
            // BFS from first cell, ensure all cells reachable
            let cell_set: std::collections::HashSet<(usize, usize)> =
                cage.cells.iter().copied().collect();
            let mut visited = std::collections::HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(cage.cells[0]);
            visited.insert(cage.cells[0]);
            while let Some((r, c)) = queue.pop_front() {
                for (dr, dc) in [(-1i32, 0), (1, 0), (0, -1i32), (0, 1)] {
                    let nr = r as i32 + dr;
                    let nc = c as i32 + dc;
                    if (0..9).contains(&nr) && (0..9).contains(&nc) {
                        let pos = (nr as usize, nc as usize);
                        if cell_set.contains(&pos) && !visited.contains(&pos) {
                            visited.insert(pos);
                            queue.push_back(pos);
                        }
                    }
                }
            }
            assert_eq!(
                visited.len(),
                cage.cells.len(),
                "cage {i} is not contiguous"
            );
        }
    }

    #[test]
    fn generated_killer_puzzle_has_unique_solution() {
        let mut rng = StdRng::seed_from_u64(999);
        let puzzle = generate_killer_puzzle(Difficulty::Easy, &mut rng);
        let cages = puzzle.cages.as_ref().unwrap();
        assert_eq!(
            crate::solver::count_solutions_killer(&Grid::empty(), cages, 2),
            1
        );
    }

    #[test]
    fn clue_counts_match_difficulty() {
        let mut rng = StdRng::seed_from_u64(456);
        for difficulty in [Difficulty::Easy, Difficulty::Medium, Difficulty::Hard] {
            let puzzle = generate_puzzle(difficulty, &mut rng);
            let clues: usize = puzzle
                .givens
                .iter()
                .flat_map(|row| row.iter())
                .filter(|&&g| g)
                .count();
            let (min, max) = difficulty.clue_range();
            assert!(
                clues >= min && clues <= max,
                "{difficulty:?}: expected {min}-{max} clues, got {clues}"
            );
        }
    }
}
