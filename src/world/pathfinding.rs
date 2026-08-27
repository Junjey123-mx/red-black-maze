use std::collections::VecDeque;

use super::level::Level;

/// Campo de distancias (en pasos de cuadrícula) desde una celda de
/// origen, calculado mediante BFS de 4 direcciones sobre las celdas
/// transitables de `Level`.
///
/// Estructura de dominio pura: no conoce `Framebuffer`, `Player`,
/// `Entity`, `RaylibHandle` ni ningún tipo de audio/rendering, por lo
/// que es comprobable directamente sobre un `Level` sin abrir una
/// ventana. Reutiliza `Level::is_walkable` como ÚNICA autoridad de
/// qué celda es transitable; no reimplementa ni duplica esa regla.
pub(crate) struct DistanceField {
    distances: Vec<Option<u32>>,
    width: usize,
    height: usize,
}

impl DistanceField {
    /// Calcula el campo de distancias desde `origin` (fila, columna)
    /// sobre `level`, mediante BFS estándar mediante `VecDeque`.
    ///
    /// Si `origin` está fuera de rango o no es transitable, retorna
    /// un campo completamente inalcanzable (todas las celdas en
    /// `None`) en vez de entrar en pánico: no existe ninguna celda
    /// válida desde la cual perseguir.
    pub(crate) fn from_level(level: &Level, origin: (usize, usize)) -> Self {
        let width = level.width();

        let height = level.height();

        let mut distances = vec![None; width * height];

        let (origin_row, origin_column) = origin;

        if origin_row >= height
            || origin_column >= width
            || !level.is_walkable(origin_row, origin_column)
        {
            return Self {
                distances,
                width,
                height,
            };
        }

        let mut queue = VecDeque::new();

        distances[origin_row * width + origin_column] = Some(0);

        queue.push_back(origin);

        while let Some((row, column)) = queue.pop_front() {
            let current_distance =
                distances[row * width + column].expect("la celda en cola ya tiene distancia");

            for (next_row, next_column) in
                neighbors(row, column, width, height).into_iter().flatten()
            {
                if !level.is_walkable(next_row, next_column) {
                    continue;
                }

                let index = next_row * width + next_column;

                if distances[index].is_some() {
                    continue;
                }

                distances[index] = Some(current_distance + 1);

                queue.push_back((next_row, next_column));
            }
        }

        Self {
            distances,
            width,
            height,
        }
    }

    /// Distancia en pasos de cuadrícula desde el origen hasta
    /// `(row, column)`, o `None` si esa celda es inalcanzable o cae
    /// fuera de los límites del nivel. Nunca entra en pánico.
    pub(crate) fn distance_at(&self, row: usize, column: usize) -> Option<u32> {
        if row >= self.height || column >= self.width {
            return None;
        }

        self.distances[row * self.width + column]
    }

    /// Elige, entre los vecinos 4-direccionales TRANSITABLES de
    /// `from`, aquel cuya distancia al origen sea estrictamente
    /// menor que la de `from` (nunca retrocede ni permanece en el
    /// lugar); si hay varios, el de menor distancia.
    ///
    /// Retorna `None` si `from` es inalcanzable/fuera de rango, o si
    /// `from` ya es el propio origen (distancia 0): no hay paso
    /// siguiente que dar.
    pub(crate) fn step_toward_origin(&self, from: (usize, usize)) -> Option<(usize, usize)> {
        let current_distance = self.distance_at(from.0, from.1)?;

        if current_distance == 0 {
            return None;
        }

        neighbors(from.0, from.1, self.width, self.height)
            .into_iter()
            .flatten()
            .filter_map(|cell| {
                self.distance_at(cell.0, cell.1)
                    .map(|distance| (cell, distance))
            })
            .filter(|&(_, distance)| distance < current_distance)
            .min_by_key(|&(_, distance)| distance)
            .map(|(cell, _)| cell)
    }

    /// Simétrico de `step_toward_origin` para una entidad que HUYE del
    /// origen (Bloque 4, Commit 46): elige, entre los vecinos
    /// 4-direccionales alcanzables de `from`, aquel cuya distancia al
    /// origen sea ESTRICTAMENTE MAYOR que la de `from` (nunca se
    /// acerca ni se queda igual); si hay varios, el de mayor
    /// distancia.
    ///
    /// Retorna `None` si `from` es inalcanzable/fuera de rango o si
    /// ningún vecino alejable existe (callejón sin salida respecto al
    /// origen): la entidad se detiene en vez de oscilar. Reutiliza la
    /// MISMA cuadrícula, `Level::is_walkable` y topología 4-direccional
    /// que el resto del campo — no hay un segundo grafo de navegación.
    pub(crate) fn step_away_from_origin(&self, from: (usize, usize)) -> Option<(usize, usize)> {
        let current_distance = self.distance_at(from.0, from.1)?;

        neighbors(from.0, from.1, self.width, self.height)
            .into_iter()
            .flatten()
            .filter_map(|cell| {
                self.distance_at(cell.0, cell.1)
                    .map(|distance| (cell, distance))
            })
            .filter(|&(_, distance)| distance > current_distance)
            .max_by_key(|&(_, distance)| distance)
            .map(|(cell, _)| cell)
    }
}

/// Vecinos 4-direccionales (arriba/abajo/izquierda/derecha) de
/// `(row, column)` dentro de una cuadrícula de `width x height`,
/// recortados contra los límites. Arreglo fijo de 4 posiciones (sin
/// asignación de heap): cada posición es `None` si el vecino
/// correspondiente cae fuera de rango. Deliberadamente sin vecinos
/// diagonales: la topología del laberinto es 4-direccional, así que
/// ninguna ruta puede "cortar" una esquina bloqueada.
fn neighbors(
    row: usize,
    column: usize,
    width: usize,
    height: usize,
) -> [Option<(usize, usize)>; 4] {
    [
        if row > 0 {
            Some((row - 1, column))
        } else {
            None
        },
        if row + 1 < height {
            Some((row + 1, column))
        } else {
            None
        },
        if column > 0 {
            Some((row, column - 1))
        } else {
            None
        },
        if column + 1 < width {
            Some((row, column + 1))
        } else {
            None
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Guardia RAII mínima para un archivo de nivel temporal, mismo
    /// patrón std-only ya establecido en las pruebas de integración
    /// (Tarea 36/37): nombre único vía PID + contador, limpieza
    /// automática al salir de alcance.
    struct TempLevelFile {
        path: PathBuf,
    }

    impl TempLevelFile {
        fn write(contents: &str) -> Self {
            let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

            let file_name = format!(
                "red_black_maze_pathfinding_test_{}_{counter}.txt",
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
    fn straight_corridor_next_step_moves_toward_the_origin() {
        let map = "\
#######
#p   g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el corredor debe cargar");

        // Origen en g = (1, 5).
        let field = DistanceField::from_level(&level, (1, 5));

        assert_eq!(field.distance_at(1, 5), Some(0));
        assert_eq!(field.distance_at(1, 3), Some(2));

        // Desde (1, 3), el único paso que reduce la distancia es
        // moverse a la derecha, hacia el origen.
        assert_eq!(field.step_toward_origin((1, 3)), Some((1, 4)));
    }

    #[test]
    fn wall_obstruction_routes_around_instead_of_through() {
        let map = "\
#######
#p    #
##### #
#g    #
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa con pared debe cargar");

        // Origen en g = (3, 1). La única abertura entre las filas 1 y
        // 3 está en la columna 5.
        let field = DistanceField::from_level(&level, (3, 1));

        // Distancia real (con desvío) es mucho mayor que la
        // distancia Manhattan directa (2), confirmando que la ruta
        // rodeó la pared en vez de atravesarla.
        assert_eq!(field.distance_at(1, 1), Some(10));

        // El primer paso desde (1,1) debe ser hacia la derecha (la
        // única abertura), nunca hacia abajo (pared).
        assert_eq!(field.step_toward_origin((1, 1)), Some((1, 2)));
    }

    #[test]
    fn unreachable_cell_returns_none_safely() {
        let map = "\
#######
#p # g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa dividido debe cargar");

        // Origen en g = (1, 5); p = (1, 1) queda en un bolsillo
        // separado por la pared de la columna 3.
        let field = DistanceField::from_level(&level, (1, 5));

        assert_eq!(field.distance_at(1, 1), None);
        assert_eq!(field.step_toward_origin((1, 1)), None);
    }

    #[test]
    fn start_equal_to_origin_needs_no_further_step() {
        let map = "\
#####
#p g#
#####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa debe cargar");

        let field = DistanceField::from_level(&level, (1, 3));

        assert_eq!(field.step_toward_origin((1, 3)), None);
    }

    #[test]
    fn out_of_range_queries_are_safe_and_never_panic() {
        let map = "\
#####
#p g#
#####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa debe cargar");

        let field = DistanceField::from_level(&level, (1, 3));

        assert_eq!(field.distance_at(1_000, 1_000), None);
        assert_eq!(field.step_toward_origin((1_000, 1_000)), None);

        // Origen fuera de rango: campo completamente inalcanzable,
        // sin pánico.
        let out_of_range_field = DistanceField::from_level(&level, (1_000, 1_000));

        assert_eq!(out_of_range_field.distance_at(1, 3), None);
    }

    #[test]
    fn step_away_from_origin_increases_path_distance_and_respects_walls() {
        let map = "\
#######
#p    #
##### #
#g    #
#######
";
        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el mapa debe cargar");

        // Origen en g = (3, 1); la única abertura entre filas está en
        // la columna 5.
        let field = DistanceField::from_level(&level, (3, 1));

        // Desde (3, 3) el paso que ALEJA del origen es hacia la
        // derecha (col 4), nunca hacia la pared ni de vuelta.
        let away = field.step_away_from_origin((3, 3)).expect("hay salida");
        assert_eq!(away, (3, 4));
        assert!(field.distance_at(away.0, away.1).unwrap() > field.distance_at(3, 3).unwrap());
    }

    #[test]
    fn step_away_from_origin_stops_instead_of_oscillating_at_a_dead_end() {
        let map = "\
#####
#pg #
#####
";
        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el mapa debe cargar");

        // Origen en g = (1, 2). Desde (1, 1) (celda de p) el único
        // vecino transitable es el propio origen -> no hay a dónde
        // alejarse.
        let field = DistanceField::from_level(&level, (1, 2));
        assert_eq!(field.step_away_from_origin((1, 1)), None);

        // Celda inalcanzable / fuera de rango: sin pánico.
        assert_eq!(field.step_away_from_origin((100, 100)), None);
    }

    #[test]
    fn diagonally_adjacent_cell_is_not_treated_as_connected() {
        let map = "\
####
#p##
##g#
####
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el mapa diagonal debe cargar");

        // g = (2, 2) queda completamente aislado: sus cuatro vecinos
        // cardinales (1,2)/(3,2)/(2,1)/(2,3) son todos pared. Aunque
        // p = (1,1) es diagonalmente adyacente a g, la topología
        // 4-direccional NUNCA los conecta.
        let field = DistanceField::from_level(&level, (2, 2));

        assert_eq!(field.distance_at(2, 2), Some(0));
        assert_eq!(field.distance_at(1, 1), None);
    }
}
