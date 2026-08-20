use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};

use super::tile::Tile;

/// Error al cargar o validar un nivel.
#[derive(Debug, Clone)]
pub enum LevelError {
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
pub struct Level {
    cells: Vec<Vec<char>>,
    width: usize,
    height: usize,
    player_spawn: (usize, usize),
    goal: (usize, usize),
    torch_spawns: Vec<(usize, usize)>,
    enemy_spawns: Vec<(usize, usize)>,
    ammo_spawns: Vec<(usize, usize)>,
}

impl Level {
    /// Carga y valida un nivel desde un archivo de texto.
    pub fn load(filename: &str) -> Result<Self, LevelError> {
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

        /*
         * Las antorchas son marcadores de aparición de sprite
         * opcionales: cero, una o varias son válidas, sin
         * validación adicional.
         */
        let torch_spawns = Self::find_cells(&cells, 't');

        /*
         * Los marcadores de aparición enemiga son opcionales: cero,
         * uno o varios son válidos. La cantidad de Dealers no forma
         * parte de la validación estructural del nivel.
         */
        let enemy_spawns = Self::find_cells(&cells, 'e');

        /*
         * Tarea 44: los marcadores de aparición de munición son
         * opcionales, igual que `t`/`e` — cero, uno o varios son
         * válidos, sin validación estructural adicional. Solo
         * aportan POSICIONES; el estado de partida (activo/
         * recogido) vive en `world::AmmoPickup`, construido a partir
         * de estas posiciones por `GameSession`, nunca aquí.
         */
        let ammo_spawns = Self::find_cells(&cells, 'a');

        let height = cells.len();

        Ok(Self {
            width: expected_width,
            height,
            player_spawn: player_spawn_cells[0],
            goal: goal_cells[0],
            torch_spawns,
            enemy_spawns,
            ammo_spawns,
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
    pub fn width(&self) -> usize {
        self.width
    }

    /// Alto del nivel en celdas.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Acceso seguro al carácter crudo de una celda.
    pub fn cell_at(&self, row: usize, column: usize) -> Option<char> {
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
    pub fn is_walkable(&self, row: usize, column: usize) -> bool {
        self.tile_at(row, column).is_some_and(Tile::is_walkable)
    }

    /// Posición (fila, columna) de la celda de aparición del jugador.
    pub fn player_spawn(&self) -> (usize, usize) {
        self.player_spawn
    }

    /// Posición (fila, columna) de la meta.
    pub fn goal(&self) -> (usize, usize) {
        self.goal
    }

    /// Posiciones (fila, columna) de todas las celdas de aparición
    /// de antorcha. Puede estar vacío.
    pub(crate) fn torch_spawns(&self) -> &[(usize, usize)] {
        &self.torch_spawns
    }

    /// Posiciones (fila, columna) de todas las celdas de aparición
    /// enemiga. Puede estar vacío.
    pub(crate) fn enemy_spawns(&self) -> &[(usize, usize)] {
        &self.enemy_spawns
    }

    /// Posiciones (fila, columna) de todas las celdas de aparición
    /// de munición. Puede estar vacío.
    pub(crate) fn ammo_spawns(&self) -> &[(usize, usize)] {
        &self.ammo_spawns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Mismo patrón RAII std-only ya establecido en
    /// `tests/level_loading.rs`/`world::pathfinding`: los tests de
    /// aquí viven en `src/world/level.rs` (no en la integración
    /// externa) porque `ammo_spawns`/`enemy_spawns`/`torch_spawns`
    /// son `pub(crate)`, inalcanzables desde un crate externo.
    struct TempLevelFile {
        path: PathBuf,
    }

    impl TempLevelFile {
        fn write(contents: &str) -> Self {
            let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

            let file_name = format!(
                "red_black_maze_level_test_{}_{counter}.txt",
                std::process::id()
            );

            let path = std::env::temp_dir().join(file_name);

            let mut file =
                fs::File::create(&path).expect("no se pudo crear el archivo temporal de nivel");

            file.write_all(contents.as_bytes())
                .expect("no se pudo escribir el archivo temporal de nivel");

            Self { path }
        }

        fn path_str(&self) -> &str {
            self.path
                .to_str()
                .expect("la ruta temporal debe ser UTF-8 válida")
        }
    }

    impl Drop for TempLevelFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }

    #[test]
    fn ammo_marker_is_recognized_as_an_ammo_spawn() {
        let map = "\
#####
#p a#
#  g#
#####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa con munición debe cargar");

        assert_eq!(level.ammo_spawns(), &[(1, 3)]);
    }

    #[test]
    fn ammo_spawn_cell_remains_walkable() {
        let map = "\
#####
#p a#
#  g#
#####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa con munición debe cargar");

        assert!(level.is_walkable(1, 3));
    }

    #[test]
    fn ammo_spawn_tile_classifies_as_ammo_spawn() {
        let map = "\
#####
#p a#
#  g#
#####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa con munición debe cargar");

        assert_eq!(level.tile_at(1, 3), Some(Tile::AmmoSpawn));
    }

    #[test]
    fn multiple_ammo_spawns_are_all_recorded_without_validation_limit() {
        let map = "\
#######
#p a  #
# a  a#
#    g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa con varias 'a' debe cargar");

        assert_eq!(level.ammo_spawns().len(), 3);
    }

    #[test]
    fn level_with_no_ammo_markers_has_an_empty_spawn_list() {
        let map = "\
#####
#p g#
#####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa sin munición debe cargar");

        assert!(level.ammo_spawns().is_empty());
    }

    /// Distribución real de pickups de munición (Tarea 44) sobre los
    /// tres niveles finales del proyecto. Vive aquí (no en
    /// `tests/level_loading.rs`) porque `ammo_spawns` es
    /// `pub(crate)`, inalcanzable desde ese crate de integración
    /// externo — mismo motivo que ya aplicaba a `torch_spawns`/
    /// `enemy_spawns`, ninguno de los cuales tampoco se prueba
    /// allí.
    #[test]
    fn real_levels_have_the_exact_expected_ammo_spawn_counts() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        let crimson = Level::load(&format!("{manifest_dir}/levels/level_01.txt"))
            .expect("Crimson Entrance debe cargar");
        assert_eq!(crimson.ammo_spawns().len(), 2);

        let black_club = Level::load(&format!("{manifest_dir}/levels/level_02.txt"))
            .expect("Black Club debe cargar");
        assert_eq!(black_club.ammo_spawns().len(), 3);

        let house_of_cards = Level::load(&format!("{manifest_dir}/levels/level_03.txt"))
            .expect("House of Cards debe cargar");
        assert_eq!(house_of_cards.ammo_spawns().len(), 4);
    }

    /// Ningún pickup de munición real coincide con la posición del
    /// spawn del jugador, la meta, un Dealer o una antorcha — cada
    /// uno posee su propia celda.
    #[test]
    fn real_levels_ammo_spawns_do_not_overlap_other_semantic_markers() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        for path in ["level_01.txt", "level_02.txt", "level_03.txt"] {
            let level = Level::load(&format!("{manifest_dir}/levels/{path}"))
                .unwrap_or_else(|_| panic!("{path} debe cargar"));

            let mut occupied: Vec<(usize, usize)> = vec![level.player_spawn(), level.goal()];
            occupied.extend(level.torch_spawns());
            occupied.extend(level.enemy_spawns());

            for &ammo_position in level.ammo_spawns() {
                assert!(
                    !occupied.contains(&ammo_position),
                    "{path}: la posición de munición {ammo_position:?} colisiona con otro marcador"
                );

                // Cada 'a' debe ser, en efecto, su propia celda
                // caminable independiente.
                assert!(level.is_walkable(ammo_position.0, ammo_position.1));
            }
        }
    }

    /// Los nueve pickups reales son alcanzables desde el spawn del
    /// jugador vía la geometría normal del nivel (BFS de 4
    /// direcciones sobre celdas caminables) — ninguna 'a' quedó
    /// encerrada detrás de paredes inaccesibles.
    #[test]
    fn real_levels_ammo_spawns_are_reachable_from_the_player_spawn() {
        use std::collections::{HashSet, VecDeque};

        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        for path in ["level_01.txt", "level_02.txt", "level_03.txt"] {
            let level = Level::load(&format!("{manifest_dir}/levels/{path}"))
                .unwrap_or_else(|_| panic!("{path} debe cargar"));

            let start = level.player_spawn();

            let mut visited: HashSet<(usize, usize)> = HashSet::from([start]);

            let mut queue: VecDeque<(usize, usize)> = VecDeque::from([start]);

            while let Some((row, column)) = queue.pop_front() {
                let neighbors = [
                    row.checked_sub(1).map(|r| (r, column)),
                    Some((row + 1, column)),
                    column.checked_sub(1).map(|c| (row, c)),
                    Some((row, column + 1)),
                ];

                for neighbor in neighbors.into_iter().flatten() {
                    if visited.contains(&neighbor) {
                        continue;
                    }

                    if level.is_walkable(neighbor.0, neighbor.1) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }

            for &ammo_position in level.ammo_spawns() {
                assert!(
                    visited.contains(&ammo_position),
                    "{path}: la munición en {ammo_position:?} es inalcanzable desde el spawn del jugador"
                );
            }
        }
    }
}
