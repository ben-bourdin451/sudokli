use rand::Rng;
use rand::seq::SliceRandom;

use crate::grid::{Grid, PuzzleState};
use crate::solver::count_solutions;

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

    PuzzleState { grid, givens }
}

/// Fill the grid with a complete valid solution using backtracking with
/// randomized digit ordering.
fn fill_grid(grid: &mut Grid, rng: &mut impl Rng) -> bool {
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
