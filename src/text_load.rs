use std::fs::File;
use std::io::{BufRead, BufReader};

pub type Maze = Vec<Vec<char>>;

pub fn load_maze(filename: &str) -> Maze {
    let file = File::open(filename).expect("No se pudo abrir el archivo del laberinto");

    let reader = BufReader::new(file);

    reader
        .lines()
        .map(|line| {
            line.expect("No se pudo leer una línea del laberinto")
                .chars()
                .collect()
        })
        .collect()
}

pub fn validate_maze(maze: &Maze) -> Result<(), String> {
    if maze.is_empty() {
        return Err("El laberinto está vacío".to_string());
    }

    let expected_width = maze[0].len();

    if expected_width == 0 {
        return Err("La primera fila está vacía".to_string());
    }

    for (row_index, row) in maze.iter().enumerate() {
        if row.len() != expected_width {
            return Err(format!(
                "La fila {} tiene {} caracteres, pero debería tener {}",
                row_index + 1,
                row.len(),
                expected_width,
            ));
        }
    }

    let player_count = maze.iter().flatten().filter(|&&cell| cell == 'p').count();

    let goal_count = maze.iter().flatten().filter(|&&cell| cell == 'g').count();

    if player_count != 1 {
        return Err(format!(
            "Debe existir exactamente una 'p', pero se encontraron {player_count}",
        ));
    }

    if goal_count != 1 {
        return Err(format!(
            "Debe existir exactamente una 'g', pero se encontraron {goal_count}",
        ));
    }

    Ok(())
}
