use crate::grid::Grid;

/// Count solutions for the given grid, stopping early once `limit` is reached.
/// Returns a value in `0..=limit`.
pub fn count_solutions(grid: &Grid, limit: usize) -> usize {
    let mut grid = grid.clone();
    let mut count = 0;
    solve_recursive(&mut grid, &mut count, limit);
    count
}

fn solve_recursive(grid: &mut Grid, count: &mut usize, limit: usize) {
    if *count >= limit {
        return;
    }

    // Find next empty cell
    let Some((row, col)) = find_empty(grid) else {
        *count += 1;
        return;
    };

    for val in 1..=9 {
        if grid.is_valid_placement(row, col, val) {
            grid.set(row, col, val);
            solve_recursive(grid, count, limit);
            if *count >= limit {
                return;
            }
            grid.set(row, col, 0);
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
