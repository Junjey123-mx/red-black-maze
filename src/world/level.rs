use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

use super::tile::Tile;

/// Error al cargar o validar un nivel.
#[derive(Debug, Clone)]
pub(crate) enum LevelError {
    Empty,
    EmptyFirstRow,
    InconsistentRowWidth {
        row: usize,
        found: usize,
        expected: usize,
    },
    PlayerSpawnCount(usize),
    GoalCount(usize),
}

impl fmt::Display for LevelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LevelError::Empty => write!(formatter, "El laberinto está vacío"),

            LevelError::EmptyFirstRow => write!(formatter, "La primera fila está vacía"),

            LevelError::InconsistentRowWidth {
                row,
                found,
                expected,
            } => write!(
                formatter,
                "La fila {row} tiene {found} caracteres, pero debería tener {expected}",
            ),

            LevelError::PlayerSpawnCount(count) => write!(
                formatter,
                "Debe existir exactamente una 'p', pero se encontraron {count}",
            ),

            LevelError::GoalCount(count) => write!(
                formatter,
                "Debe existir exactamente una 'g', pero se encontraron {count}",
            ),
        }
    }
}

/// Representación cargada y validada del mundo actual.
///
/// `Level` es la fuente de verdad en tiempo de ejecución del mapa:
/// descubre la celda de aparición del jugador y la meta para que
/// los demás sistemas no tengan que buscarlas por su cuenta.
pub(crate) struct Level {
    cells: Vec<Vec<char>>,
    width: usize,
    height: usize,
    player_spawn: (usize, usize),
    goal: (usize, usize),
}

impl Level {
    /// Carga y valida un nivel desde un archivo de texto.
    pub(crate) fn load(filename: &str) -> Result<Self, LevelError> {
        let file = File::open(filename).expect("No se pudo abrir el archivo del laberinto");

        let reader = BufReader::new(file);

        let cells: Vec<Vec<char>> = reader
            .lines()
            .map(|line| {
                line.expect("No se pudo leer una línea del laberinto")
                    .chars()
                    .collect()
            })
            .collect();

        Self::from_cells(cells)
    }

    fn from_cells(cells: Vec<Vec<char>>) -> Result<Self, LevelError> {
        if cells.is_empty() {
            return Err(LevelError::Empty);
        }

        let expected_width = cells[0].len();

        if expected_width == 0 {
            return Err(LevelError::EmptyFirstRow);
        }

        for (row_index, row) in cells.iter().enumerate() {
            if row.len() != expected_width {
                return Err(LevelError::InconsistentRowWidth {
                    row: row_index + 1,
                    found: row.len(),
                    expected: expected_width,
                });
            }
        }

        let player_spawn_cells = Self::find_cells(&cells, 'p');

        if player_spawn_cells.len() != 1 {
            return Err(LevelError::PlayerSpawnCount(player_spawn_cells.len()));
        }

        let goal_cells = Self::find_cells(&cells, 'g');

        if goal_cells.len() != 1 {
            return Err(LevelError::GoalCount(goal_cells.len()));
        }

        let height = cells.len();

        Ok(Self {
            width: expected_width,
            height,
            player_spawn: player_spawn_cells[0],
            goal: goal_cells[0],
            cells,
        })
    }

    /// Encuentra todas las posiciones (fila, columna) que
    /// contienen el carácter indicado.
    fn find_cells(cells: &[Vec<char>], target: char) -> Vec<(usize, usize)> {
        cells
            .iter()
            .enumerate()
            .flat_map(|(row_index, row)| {
                row.iter()
                    .enumerate()
                    .filter(move |&(_, &cell)| cell == target)
                    .map(move |(column_index, _)| (row_index, column_index))
            })
            .collect()
    }

    /// Ancho del nivel en celdas.
    pub(crate) fn width(&self) -> usize {
        self.width
    }

    /// Alto del nivel en celdas.
    pub(crate) fn height(&self) -> usize {
        self.height
    }

    /// Acceso seguro al carácter crudo de una celda.
    pub(crate) fn cell_at(&self, row: usize, column: usize) -> Option<char> {
        self.cells.get(row).and_then(|row| row.get(column)).copied()
    }

    /// Acceso seguro a la clasificación semántica de una celda.
    pub(crate) fn tile_at(&self, row: usize, column: usize) -> Option<Tile> {
        self.cell_at(row, column).and_then(Tile::from_char)
    }

    /// Indica si la celda puede ser atravesada por el jugador
    /// y por los rayos.
    ///
    /// Retorna `false` para posiciones fuera de los límites del
    /// nivel y para cualquier carácter no reconocido.
    pub(crate) fn is_walkable(&self, row: usize, column: usize) -> bool {
        self.tile_at(row, column).is_some_and(Tile::is_walkable)
    }

    /// Posición (fila, columna) de la celda de aparición del jugador.
    pub(crate) fn player_spawn(&self) -> (usize, usize) {
        self.player_spawn
    }

    /// Posición (fila, columna) de la meta.
    pub(crate) fn goal(&self) -> (usize, usize) {
        self.goal
    }
}
