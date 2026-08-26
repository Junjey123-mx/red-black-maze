use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};

use super::level::Level;
use super::level_manager::LevelTheme;
use super::pathfinding::DistanceField;
use super::rng::Rng;

/// Cantidad de celdas transitables medida en `levels/level_03.txt`
/// (House of Cards, 17×13), usada como referencia real de tamaño —
/// nunca asumida. Ver `tests/level_loading.rs`/`world::level` para
/// la medición original; se re-verifica aquí en una prueba dedicada
/// contra el archivo real para que esta constante nunca quede
/// desincronizada si el nivel 3 cambiara.
const HOUSE_OF_CARDS_WALKABLE_CELLS: usize = 111;

/// Rango objetivo de área jugable de `The Dealer's True Maze`,
/// como múltiplo de `HOUSE_OF_CARDS_WALKABLE_CELLS` (sección 4:
/// "cerca del doble de superficie efectiva, no cuatro veces" —
/// deliberadamente NO se logra duplicando ancho y alto).
const MIN_AREA_MULTIPLIER: f32 = 1.8;
const MAX_AREA_MULTIPLIER: f32 = 2.2;

/// Cuadrícula LÓGICA (una celda lógica = una celda de laberinto real
/// del recursive backtracker; el carácter final es
/// `(2*ancho+1) x (2*alto+1)`, con muros intercalados). Elegida para
/// que el laberinto "perfecto" de partida (árbol de expansión, sin
/// loops todavía) más el refinamiento de loops aterrice cerca del
/// centro del rango objetivo (~2.0×): ver
/// `dimensions_hit_the_target_area_multiplier` para la verificación
/// exacta.
const LOGICAL_WIDTH: usize = 11;
const LOGICAL_HEIGHT: usize = 9;

/// Distancia mínima navegable (en pasos de `DistanceField`, no
/// euclidiana) entre el spawn del jugador y cualquier Dealer
/// (sección 11). `DEALER_ALERT_DISTANCE_CELLS` (`world::entity`) es
/// 4.0 celdas en línea recta; 6 pasos navegables es una cota
/// deliberadamente mayor (una ruta real casi nunca es más corta que
/// la línea recta, así que 6 garantiza margen incluso en un pasillo
/// recto) para que ningún Dealer pueda estar ya en `Alert` en el
/// instante exacto en que la partida comienza.
const SAFE_SPAWN_DISTANCE_CELLS: u32 = 6;

/// Fracción mínima/máxima de la distancia navegable máxima
/// alcanzable desde el spawn a la que debe colocarse la meta
/// (sección 14).
const GOAL_DISTANCE_MIN_FRACTION: f32 = 0.75;
const GOAL_DISTANCE_MAX_FRACTION: f32 = 0.90;

/// Celdas transitables por Dealer, calibrado para que el nivel se
/// sienta claramente como una "plaga" frente a los 4 Dealers de
/// House of Cards (111 celdas / 4 ≈ 1 cada 28); aquí se apunta a
/// ~1 cada 9-10 celdas.
const WALKABLE_CELLS_PER_DEALER: f32 = 9.5;
const MIN_DEALERS: usize = 18;
const MAX_DEALERS: usize = 30;

/// Daño por disparo aceptado y vida máxima del Dealer
/// (`game::session::DEALER_DAMAGE_PER_HIT` / `world::entity::
/// DEALER_MAX_HEALTH`), duplicados aquí SOLO como literales de
/// cálculo de presupuesto de munición — nunca como una segunda
/// fuente de verdad de esas reglas de combate (el nivel no las lee
/// ni las modifica).
const SHOTS_TO_KILL_ONE_DEALER: u32 = 2;

/// Margen por errores de puntería del jugador (sección 13): se pide
/// munición para el 150% de los disparos teóricamente necesarios.
const MISS_MARGIN_MULTIPLIER: f32 = 1.5;

/// Munición con la que el jugador siempre arranca
/// (`Weapon::MAGAZINE_CAPACITY` + `Weapon::INITIAL_RESERVE_AMMO` =
/// 6 + 18), y munición de reserva que otorga cada pickup
/// (`game::session::AMMO_PICKUP_AMOUNT`). Mismo motivo que los
/// literales de combate: solo para el cálculo de presupuesto.
const STARTING_TOTAL_AMMO: u32 = 24;
const AMMO_PER_PICKUP: u32 = 6;
const MIN_AMMO_PICKUPS: usize = 8;
const MAX_AMMO_PICKUPS: usize = 20;

/// Intentos máximos de generación antes de recurrir al mapa de
/// emergencia (sección 15). Las garantías "por construcción" del
/// algoritmo (ver comentarios en `generate`) hacen que el intento 1
/// case prácticamente siempre; este límite es una red de seguridad
/// defensiva, no el camino esperado.
const MAX_GENERATION_ATTEMPTS: u32 = 25;

const WALL_CHARS: [char; 4] = ['+', '-', '|', '#'];

/// Resultado completo y reproducible de una generación procedural:
/// todo lo que `LevelManager` necesita cachear para que `Retry`
/// (sección 7) reconstruya EXACTAMENTE el mismo nivel sin volver a
/// tirar el dado.
pub(crate) struct GeneratedLevel {
    pub(crate) cells: Vec<Vec<char>>,
    pub(crate) theme: LevelTheme,
    pub(crate) seed: u64,
    pub(crate) dealer_count: usize,
    pub(crate) ammo_pickup_count: usize,
}

/// Semilla nueva y (para fines prácticos) única por partida, para
/// "New Game" (sección 6/21). Fuente de entropía: reloj del sistema
/// + PID del proceso — sin crates externos. Nunca se usa DENTRO de
/// `generate`, solo para producir el `seed: u64` que luego SÍ
/// determina toda la generación de forma reproducible.
pub(crate) fn fresh_seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);

    let pid = std::process::id() as u64;

    nanos ^ pid.wrapping_mul(0x9E3779B97F4A7C15)
}

/// Celda lógica del laberinto (antes de expandir a caracteres).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct LogicalCell {
    row: usize,
    column: usize,
}

impl LogicalCell {
    fn index(self, width: usize) -> usize {
        self.row * width + self.column
    }
}

/// Vecinos lógicos 4-direccionales dentro de `width x height`.
fn logical_neighbors(cell: LogicalCell, width: usize, height: usize) -> Vec<LogicalCell> {
    let mut result = Vec::with_capacity(4);

    if cell.row > 0 {
        result.push(LogicalCell {
            row: cell.row - 1,
            column: cell.column,
        });
    }

    if cell.row + 1 < height {
        result.push(LogicalCell {
            row: cell.row + 1,
            column: cell.column,
        });
    }

    if cell.column > 0 {
        result.push(LogicalCell {
            row: cell.row,
            column: cell.column - 1,
        });
    }

    if cell.column + 1 < width {
        result.push(LogicalCell {
            row: cell.row,
            column: cell.column + 1,
        });
    }

    result
}

/// Randomized DFS / Recursive Backtracker (sección 5) sobre la
/// cuadrícula lógica: retorna el conjunto de aristas (pares de
/// celdas lógicas adyacentes) que forman el árbol de expansión —
/// exactamente un camino entre cualquier par de celdas, sin loops
/// todavía.
fn carve_spanning_tree(
    width: usize,
    height: usize,
    rng: &mut Rng,
) -> HashSet<(LogicalCell, LogicalCell)> {
    let mut visited = vec![false; width * height];

    let mut edges = HashSet::new();

    let start = LogicalCell {
        row: rng.gen_range(height),
        column: rng.gen_range(width),
    };

    let mut stack = vec![start];

    visited[start.index(width)] = true;

    while let Some(&current) = stack.last() {
        let mut neighbors = logical_neighbors(current, width, height);

        rng.shuffle(&mut neighbors);

        let unvisited = neighbors
            .into_iter()
            .find(|neighbor| !visited[neighbor.index(width)]);

        match unvisited {
            Some(next) => {
                visited[next.index(width)] = true;

                edges.insert(edge_key(current, next));

                stack.push(next);
            }

            None => {
                stack.pop();
            }
        }
    }

    edges
}

/// Clave canónica (no ordenada) de una arista, para que `(a, b)` y
/// `(b, a)` se traten como la misma conexión en el `HashSet`.
fn edge_key(a: LogicalCell, b: LogicalCell) -> (LogicalCell, LogicalCell) {
    if (a.row, a.column) <= (b.row, b.column) {
        (a, b)
    } else {
        (b, a)
    }
}

/// Refinamiento post-árbol-de-expansión (sección 5): agrega aristas
/// EXTRA (loops, bifurcaciones, rutas alternativas) hasta acercar el
/// conteo final de celdas transitables al centro del rango objetivo,
/// y carva un puñado de "salas" pequeñas (celdas 2×2 totalmente
/// abiertas) para variar el ritmo del recorrido.
fn refine_maze(
    width: usize,
    height: usize,
    edges: &mut HashSet<(LogicalCell, LogicalCell)>,
    rng: &mut Rng,
) {
    // Un pequeño puñado de "salas" 2x2: conecta todas las aristas
    // internas de un bloque de 4 celdas lógicas contiguas, si aún no
    // lo estaban.
    let room_attempts = 4;

    for _ in 0..room_attempts {
        if width < 2 || height < 2 {
            break;
        }

        let room_row = rng.gen_range(height - 1);
        let room_column = rng.gen_range(width - 1);

        let block = [
            LogicalCell {
                row: room_row,
                column: room_column,
            },
            LogicalCell {
                row: room_row,
                column: room_column + 1,
            },
            LogicalCell {
                row: room_row + 1,
                column: room_column,
            },
            LogicalCell {
                row: room_row + 1,
                column: room_column + 1,
            },
        ];

        for i in 0..block.len() {
            for j in (i + 1)..block.len() {
                let a = block[i];
                let b = block[j];

                let adjacent = (a.row == b.row && a.column.abs_diff(b.column) == 1)
                    || (a.column == b.column && a.row.abs_diff(b.row) == 1);

                if adjacent {
                    edges.insert(edge_key(a, b));
                }
            }
        }
    }

    // Aristas extra aleatorias (loops / rutas falsas / cruces): se
    // agregan hasta alcanzar el número calculado por
    // `additional_edges_for_target_area`, o hasta agotar candidatos.
    let target_extra = additional_edges_for_target_area(width, height, edges.len());

    let mut candidates = Vec::new();

    for row in 0..height {
        for column in 0..width {
            let cell = LogicalCell { row, column };

            for neighbor in logical_neighbors(cell, width, height) {
                let key = edge_key(cell, neighbor);

                if !edges.contains(&key) {
                    candidates.push(key);
                }
            }
        }
    }

    // `candidates` contiene cada arista faltante dos veces (una por
    // cada extremo que la generó). Deliberadamente NO se deduplica
    // vía `HashSet` (su orden de iteración es aleatorio POR PROCESO
    // en Rust, incluso para el mismo contenido — eso rompería el
    // determinismo de "misma seed -> mismo laberinto" sin que la
    // semilla tuviera nada que ver). En su lugar: orden canónico
    // determinista (`sort_by_key`) seguido de `dedup`, y solo
    // DESPUÉS se baraja con el RNG sembrado.
    let mut unique_candidates = candidates;

    unique_candidates.sort_by_key(|&(a, b)| (a.row, a.column, b.row, b.column));
    unique_candidates.dedup();

    rng.shuffle(&mut unique_candidates);

    for key in unique_candidates.into_iter().take(target_extra) {
        edges.insert(key);
    }
}

/// Cuántas aristas EXTRA (más allá del árbol de expansión) se
/// necesitan para que el conteo final de celdas transitables
/// aterrice en el centro del rango objetivo `[MIN_AREA_MULTIPLIER,
/// MAX_AREA_MULTIPLIER] × HOUSE_OF_CARDS_WALKABLE_CELLS`.
///
/// Cada celda lógica y cada arista carvada se traduce en EXACTAMENTE
/// una celda de caracter transitable (ver `render_to_chars`): el
/// árbol de expansión ya aporta `width*height` (celdas) +
/// `current_edges` (paredes internas ya abiertas); cada arista extra
/// suma una celda transitable más.
fn additional_edges_for_target_area(width: usize, height: usize, current_edges: usize) -> usize {
    let target_multiplier = (MIN_AREA_MULTIPLIER + MAX_AREA_MULTIPLIER) / 2.0;

    let target_walkable =
        (HOUSE_OF_CARDS_WALKABLE_CELLS as f32 * target_multiplier).round() as usize;

    let base_walkable = width * height + current_edges;

    target_walkable.saturating_sub(base_walkable)
}

/// Expande la cuadrícula lógica (celdas + aristas carvadas) a la
/// representación de caracteres final: `(2*width+1) x (2*height+1)`,
/// con muros (elegidos al azar entre los cuatro caracteres
/// decorativos ya usados por los niveles estáticos) en todo lo que
/// NO sea una celda lógica ni una arista abierta. El borde exterior
/// nunca se abre (ninguna celda lógica ni arista vive en fila/
/// columna 0 o máxima), así que queda cerrado por construcción.
fn render_to_chars(
    width: usize,
    height: usize,
    edges: &HashSet<(LogicalCell, LogicalCell)>,
    rng: &mut Rng,
) -> Vec<Vec<char>> {
    let char_width = 2 * width + 1;
    let char_height = 2 * height + 1;

    let mut cells = vec![vec![' '; char_width]; char_height];

    for row in 0..char_height {
        for column in 0..char_width {
            let is_cell_row = row % 2 == 1;
            let is_cell_column = column % 2 == 1;

            if is_cell_row && is_cell_column {
                // Celda lógica: siempre transitable.
                cells[row][column] = ' ';
            } else {
                // Posición de muro/junta: pared, salvo que sea el
                // punto medio de una arista carvada.
                cells[row][column] = rng.choice(&WALL_CHARS);
            }
        }
    }

    for &(a, b) in edges {
        let mid_row = a.row + b.row + 1;
        let mid_column = a.column + b.column + 1;

        cells[mid_row][mid_column] = ' ';
    }

    cells
}

/// Todas las posiciones (fila, columna) de caracteres cuya celda es
/// transitable (`' '`) — es decir, todas las celdas lógicas Y todas
/// las aristas abiertas, en coordenadas de caracteres.
fn passage_positions(cells: &[Vec<char>]) -> Vec<(usize, usize)> {
    let mut positions = Vec::new();

    for (row, line) in cells.iter().enumerate() {
        for (column, &character) in line.iter().enumerate() {
            if character == ' ' {
                positions.push((row, column));
            }
        }
    }

    positions
}

/// Grado (cantidad de vecinos 4-direccionales transitables) de una
/// posición de caracteres — usado para identificar cruces
/// (grado >= 3, buenos para grupos/plagas) frente a corredores
/// (grado <= 2, buenos para Dealers individuales) y callejones sin
/// salida (grado == 1, buenos para pickups secundarios).
fn passage_degree(cells: &[Vec<char>], row: usize, column: usize) -> usize {
    let height = cells.len();
    let width = if height > 0 { cells[0].len() } else { 0 };

    let mut degree = 0;

    let neighbors: [Option<(usize, usize)>; 4] = [
        row.checked_sub(1).map(|r| (r, column)),
        (row + 1 < height).then_some((row + 1, column)),
        column.checked_sub(1).map(|c| (row, c)),
        (column + 1 < width).then_some((row, column + 1)),
    ];

    for neighbor in neighbors.into_iter().flatten() {
        if cells[neighbor.0][neighbor.1] == ' ' {
            degree += 1;
        }
    }

    degree
}

/// Cuántos Dealers debe tener el nivel, calibrado por tamaño real
/// (celdas transitables), acotado a un rango razonable.
fn dealer_budget(walkable_count: usize) -> usize {
    let raw = (walkable_count as f32 / WALKABLE_CELLS_PER_DEALER).round() as usize;

    raw.clamp(MIN_DEALERS, MAX_DEALERS)
}

/// Cuántos `AmmoPickup` debe tener el nivel (sección 13), calculado
/// a partir del presupuesto de disparos necesario para el número de
/// Dealers ya decidido, con margen de error de puntería, menos la
/// munición inicial con la que el jugador ya arranca.
fn ammo_pickup_budget(dealer_count: usize) -> usize {
    let shots_needed = dealer_count as u32 * SHOTS_TO_KILL_ONE_DEALER;

    let shots_with_margin = (shots_needed as f32 * MISS_MARGIN_MULTIPLIER).ceil() as u32;

    let deficit = shots_with_margin.saturating_sub(STARTING_TOTAL_AMMO);

    let pickups = deficit.div_ceil(AMMO_PER_PICKUP) as usize;

    pickups.clamp(MIN_AMMO_PICKUPS, MAX_AMMO_PICKUPS)
}

/// Intenta UNA generación completa determinista a partir de
/// `attempt_seed`. Retorna `None` si esta generación en particular
/// no logró satisfacer alguna invariante (tamaño fuera de rango,
/// presupuesto de entidades no alcanzable, etc.) — el llamador
/// decide si reintentar con otra semilla derivada.
/// Coloca hasta `size` Dealers formando un pequeño racimo compacto
/// (BFS local desde `origin` sobre celdas transitables no ocupadas):
/// usado tanto para las zonas de plaga (4-6) como para los grupos
/// pequeños (2-3) de la sección 10. Nunca coloca sobre una celda ya
/// en `occupied` (spawn/meta/otro Dealer/pickup); si el vecindario
/// se agota antes de alcanzar `size`, simplemente coloca menos —
/// nunca entra en pánico ni bloquea la única ruta (los Dealers nunca
/// son parte de la geometría de paredes, ver `Tile::is_walkable`).
fn place_cluster(
    cells: &[Vec<char>],
    occupied: &mut HashSet<(usize, usize)>,
    dealer_positions: &mut Vec<(usize, usize)>,
    origin: (usize, usize),
    size: usize,
) -> usize {
    let mut placed = 0;

    let mut frontier = vec![origin];

    while placed < size {
        let Some(candidate) = frontier.pop() else {
            break;
        };

        if occupied.contains(&candidate) {
            continue;
        }

        occupied.insert(candidate);
        dealer_positions.push(candidate);
        placed += 1;

        let neighbors: [Option<(usize, usize)>; 4] = [
            candidate.0.checked_sub(1).map(|r| (r, candidate.1)),
            Some((candidate.0 + 1, candidate.1)),
            candidate.1.checked_sub(1).map(|c| (candidate.0, c)),
            Some((candidate.0, candidate.1 + 1)),
        ];

        for neighbor in neighbors.into_iter().flatten() {
            if cells
                .get(neighbor.0)
                .and_then(|line| line.get(neighbor.1))
                .copied()
                == Some(' ')
                && !occupied.contains(&neighbor)
            {
                frontier.push(neighbor);
            }
        }
    }

    placed
}

fn try_generate(attempt_seed: u64) -> Option<GeneratedLevel> {
    let mut rng = Rng::new(attempt_seed);

    let mut edges = carve_spanning_tree(LOGICAL_WIDTH, LOGICAL_HEIGHT, &mut rng);

    refine_maze(LOGICAL_WIDTH, LOGICAL_HEIGHT, &mut edges, &mut rng);

    let mut cells = render_to_chars(LOGICAL_WIDTH, LOGICAL_HEIGHT, &edges, &mut rng);

    let passages = passage_positions(&cells);

    let walkable_count = passages.len();

    let min_walkable = (HOUSE_OF_CARDS_WALKABLE_CELLS as f32 * MIN_AREA_MULTIPLIER) as usize;
    let max_walkable = (HOUSE_OF_CARDS_WALKABLE_CELLS as f32 * MAX_AREA_MULTIPLIER) as usize;

    if walkable_count < min_walkable || walkable_count > max_walkable {
        return None;
    }

    // Spawn provisional: cualquier celda transitable. Un `g`
    // provisional (la última celda de la lista, garantizada distinta
    // de la primera porque el laberinto tiene más de una celda) es
    // SOLO para poder construir un `Level` válido y correr
    // `DistanceField` — la meta REAL se decide después con la
    // distancia ya conocida (la transitabilidad es idéntica sea cual
    // sea el marcador puesto encima, ver `Tile::is_walkable`).
    if passages.len() < 2 {
        return None;
    }

    let spawn = passages[rng.gen_range(passages.len())];

    let provisional_goal = *passages.iter().find(|&&p| p != spawn)?;

    cells[spawn.0][spawn.1] = 'p';
    cells[provisional_goal.0][provisional_goal.1] = 'g';

    let provisional_level = Level::from_cells(cells.clone()).ok()?;

    let distances = DistanceField::from_level(&provisional_level, spawn);

    let max_distance = passages
        .iter()
        .filter_map(|&(r, c)| distances.distance_at(r, c))
        .max()?;

    let min_goal_distance = (max_distance as f32 * GOAL_DISTANCE_MIN_FRACTION).ceil() as u32;
    let max_goal_distance = (max_distance as f32 * GOAL_DISTANCE_MAX_FRACTION).floor() as u32;

    let mut goal_candidates: Vec<(usize, usize)> = passages
        .iter()
        .copied()
        .filter(|&(r, c)| {
            distances
                .distance_at(r, c)
                .is_some_and(|d| d >= min_goal_distance && d <= max_goal_distance)
        })
        .collect();

    if goal_candidates.is_empty() {
        // Laberinto excepcionalmente compacto: usar la(s) celda(s)
        // alcanzable(s) más lejana(s) en vez de fallar la generación
        // entera por un rango vacío.
        goal_candidates = passages
            .iter()
            .copied()
            .filter(|&(r, c)| distances.distance_at(r, c) == Some(max_distance))
            .collect();
    }

    rng.shuffle(&mut goal_candidates);

    let goal = goal_candidates[0];

    // Limpiar los marcadores provisionales y escribir los reales.
    cells[spawn.0][spawn.1] = ' ';
    cells[provisional_goal.0][provisional_goal.1] = ' ';

    cells[spawn.0][spawn.1] = 'p';
    cells[goal.0][goal.1] = 'g';

    // --- Colocación de Dealers ("plagas", sección 10) ---

    let dealer_count = dealer_budget(walkable_count);

    let mut occupied: HashSet<(usize, usize)> = HashSet::from([spawn, goal]);

    let mut eligible: Vec<(usize, usize)> = passages
        .iter()
        .copied()
        .filter(|&pos| pos != spawn && pos != goal)
        .filter(|&(r, c)| {
            distances
                .distance_at(r, c)
                .is_some_and(|d| d >= SAFE_SPAWN_DISTANCE_CELLS)
        })
        .collect();

    rng.shuffle(&mut eligible);

    // Candidatos ordenados por grado descendente (cruces primero,
    // buenos para grupos/plagas); el resto de corredores queda al
    // final, bueno para Dealers individuales dispersos.
    let mut by_degree = eligible.clone();

    by_degree.sort_by_key(|&(r, c)| std::cmp::Reverse(passage_degree(&cells, r, c)));

    let mut dealer_positions: Vec<(usize, usize)> = Vec::with_capacity(dealer_count);

    // 1-2 zonas de plaga (4-6 Dealers) en los cruces más abiertos.
    let plague_zone_count = if dealer_count >= 22 { 2 } else { 1 };

    for zone in 0..plague_zone_count {
        if dealer_positions.len() >= dealer_count {
            break;
        }

        if let Some(&origin) = by_degree.get(zone * 3) {
            if !occupied.contains(&origin) {
                let remaining_budget = dealer_count - dealer_positions.len();

                let size = 4 + rng.gen_range(3); // 4..=6

                place_cluster(
                    &cells,
                    &mut occupied,
                    &mut dealer_positions,
                    origin,
                    size.min(remaining_budget),
                );
            }
        }
    }

    // Un puñado de grupos pequeños (2-3 Dealers) en cruces
    // restantes.
    let small_group_target = (dealer_count / 6).max(2);

    for group_index in 0..small_group_target {
        if dealer_positions.len() >= dealer_count {
            break;
        }

        let candidate_index = plague_zone_count * 3 + group_index * 2;

        if let Some(&origin) = by_degree.get(candidate_index) {
            if !occupied.contains(&origin) {
                let remaining_budget = dealer_count - dealer_positions.len();

                let size = 2 + rng.gen_range(2); // 2..=3

                place_cluster(
                    &cells,
                    &mut occupied,
                    &mut dealer_positions,
                    origin,
                    size.min(remaining_budget),
                );
            }
        }
    }

    // Rellenar el resto con Dealers individuales dispersos por
    // corredores/rutas secundarias.
    for &candidate in &eligible {
        if dealer_positions.len() >= dealer_count {
            break;
        }

        if occupied.contains(&candidate) {
            continue;
        }

        occupied.insert(candidate);
        dealer_positions.push(candidate);
    }

    if dealer_positions.len() < MIN_DEALERS.min(dealer_count) {
        return None;
    }

    for &(r, c) in &dealer_positions {
        cells[r][c] = 'e';
    }

    // --- Colocación de munición (sección 13) ---

    let ammo_target = ammo_pickup_budget(dealer_positions.len());

    // Callejones sin salida (grado == 1) primero: rutas secundarias
    // que incentivan exploración, tal como pide la sección 13.
    let mut dead_ends: Vec<(usize, usize)> = passages
        .iter()
        .copied()
        .filter(|&pos| !occupied.contains(&pos))
        .filter(|&(r, c)| passage_degree(&cells, r, c) == 1)
        .collect();

    rng.shuffle(&mut dead_ends);

    let mut remaining_eligible: Vec<(usize, usize)> = passages
        .iter()
        .copied()
        .filter(|&pos| !occupied.contains(&pos) && !dead_ends.contains(&pos))
        .collect();

    rng.shuffle(&mut remaining_eligible);

    let mut ammo_positions = Vec::with_capacity(ammo_target);

    for &pos in dead_ends.iter().chain(remaining_eligible.iter()) {
        if ammo_positions.len() >= ammo_target {
            break;
        }

        if occupied.contains(&pos) {
            continue;
        }

        occupied.insert(pos);
        ammo_positions.push(pos);
    }

    for &(r, c) in &ammo_positions {
        cells[r][c] = 'a';
    }

    let theme = rng.choice(&[
        LevelTheme::CrimsonEntrance,
        LevelTheme::BlackClub,
        LevelTheme::HouseOfCards,
    ]);

    // --- Validación final (sección 15) ---
    //
    // La mayoría de invariantes ya se cumplen POR CONSTRUCCIÓN
    // (spawn/meta transitables y únicos, ruta spawn->meta existente,
    // distancia mínima, Dealers/pickups sobre celdas válidas y sin
    // solapamiento, bordes cerrados, dimensiones). `Level::from_cells`
    // vuelve a ejercer, sobre la cuadrícula FINAL, exactamente la
    // misma validación estructural que ya usan los tres niveles
    // estáticos — la autoridad única, nunca duplicada.
    let final_level = Level::from_cells(cells.clone()).ok()?;

    let final_distances = DistanceField::from_level(&final_level, spawn);

    if final_distances.distance_at(goal.0, goal.1).is_none() {
        return None;
    }

    for &(r, c) in &dealer_positions {
        if final_distances
            .distance_at(r, c)
            .is_none_or(|d| d < SAFE_SPAWN_DISTANCE_CELLS)
        {
            return None;
        }
    }

    for &(r, c) in &ammo_positions {
        if final_distances.distance_at(r, c).is_none() {
            return None;
        }
    }

    let all_positions: Vec<(usize, usize)> = std::iter::once(spawn)
        .chain(std::iter::once(goal))
        .chain(dealer_positions.iter().copied())
        .chain(ammo_positions.iter().copied())
        .collect();

    let unique_positions: HashSet<(usize, usize)> = all_positions.iter().copied().collect();

    if unique_positions.len() != all_positions.len() {
        return None;
    }

    Some(GeneratedLevel {
        cells,
        theme,
        seed: attempt_seed,
        dealer_count: dealer_positions.len(),
        ammo_pickup_count: ammo_positions.len(),
    })
}

/// Mapa de emergencia, pequeño y trivialmente válido, embebido como
/// último recurso documentado (sección 15) si
/// `MAX_GENERATION_ATTEMPTS` intentos deterministas consecutivos
/// (misma semilla base, derivada de forma distinta en cada intento)
/// no lograran satisfacer todas las invariantes — algo que las
/// garantías por construcción de `try_generate` hacen, en la
/// práctica, virtualmente inalcanzable. Nunca se genera desde
/// `seed`: es un layout FIJO, siempre válido por inspección directa,
/// para que el juego jamás arranque con un mapa roto.
const FALLBACK_MAZE: &str = "\
#############
#p   e     g#
# ### ### # #
# #e#   #e# #
# # ##### # #
# #   a   # #
# # ##### # #
#e#       #e#
# ######### #
#     a     #
#############
";

fn fallback_generated_level(seed: u64) -> GeneratedLevel {
    let cells: Vec<Vec<char>> = FALLBACK_MAZE
        .lines()
        .map(|line| line.chars().collect())
        .collect();

    GeneratedLevel {
        cells,
        theme: LevelTheme::HouseOfCards,
        seed,
        dealer_count: 4,
        ammo_pickup_count: 2,
    }
}

/// Genera `The Dealer's True Maze` de forma determinista a partir de
/// `seed`: misma semilla -> mismo laberinto, mismo spawn, misma
/// meta, mismos Dealers, mismos pickups, mismo tema (sección 6).
///
/// Internamente reintenta con semillas derivadas de `seed` (nunca
/// con una fuente de aleatoriedad nueva) hasta
/// `MAX_GENERATION_ATTEMPTS` veces si una generación en particular
/// no satisface alguna invariante; si ninguna lo logra, retorna el
/// mapa de emergencia fijo documentado en `FALLBACK_MAZE`. El
/// resultado sigue siendo una función pura de `seed`.
pub(crate) fn generate(seed: u64) -> GeneratedLevel {
    for attempt in 0..MAX_GENERATION_ATTEMPTS {
        let attempt_seed = seed ^ (attempt as u64).wrapping_mul(0xD1B54A32D192ED03);

        if let Some(generated) = try_generate(attempt_seed) {
            log_generation(&generated);

            return generated;
        }
    }

    eprintln!(
        "The Dealer's True Maze: {MAX_GENERATION_ATTEMPTS} intentos de generación fallaron para la semilla {seed}; usando el mapa de emergencia."
    );

    let fallback = fallback_generated_level(seed);

    log_generation(&fallback);

    fallback
}

/// Registro de diagnóstico de UNA generación (sección 6: "obligatorio
/// para debugging y reproducibilidad"). Deliberadamente `eprintln!`
/// en vez de una nueva pantalla/HUD — no hay requisito de UI para
/// mostrar la semilla en pantalla, solo de poder reproducirla.
fn log_generation(generated: &GeneratedLevel) {
    eprintln!(
        "The Dealer's True Maze — Seed: {} | tema: {:?} | Dealers: {} | pickups de munición: {}",
        generated.seed, generated.theme, generated.dealer_count, generated.ammo_pickup_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Tile;

    fn count_walkable(level_txt: &str) -> usize {
        level_txt
            .lines()
            .flat_map(|line| line.chars())
            .filter(|&c| Tile::from_char(c).is_some_and(Tile::is_walkable))
            .count()
    }

    #[test]
    fn house_of_cards_walkable_constant_matches_the_real_file() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        let contents = std::fs::read_to_string(format!("{manifest_dir}/levels/level_03.txt"))
            .expect("level_03.txt debe existir");

        assert_eq!(count_walkable(&contents), HOUSE_OF_CARDS_WALKABLE_CELLS);
    }

    #[test]
    fn dimensions_hit_the_target_area_multiplier() {
        let mut edges = carve_spanning_tree(LOGICAL_WIDTH, LOGICAL_HEIGHT, &mut Rng::new(1));

        refine_maze(LOGICAL_WIDTH, LOGICAL_HEIGHT, &mut edges, &mut Rng::new(2));

        let cells = render_to_chars(LOGICAL_WIDTH, LOGICAL_HEIGHT, &edges, &mut Rng::new(3));

        let walkable = passage_positions(&cells).len();

        let multiplier = walkable as f32 / HOUSE_OF_CARDS_WALKABLE_CELLS as f32;

        assert!(
            (MIN_AREA_MULTIPLIER..=MAX_AREA_MULTIPLIER).contains(&multiplier),
            "multiplicador de área {multiplier} fuera de [{MIN_AREA_MULTIPLIER}, {MAX_AREA_MULTIPLIER}]"
        );
    }

    #[test]
    fn same_seed_produces_the_same_maze() {
        let a = generate(837_492);
        let b = generate(837_492);

        assert_eq!(a.cells, b.cells);
        assert_eq!(a.theme, b.theme);
        assert_eq!(a.dealer_count, b.dealer_count);
        assert_eq!(a.ammo_pickup_count, b.ammo_pickup_count);
    }

    #[test]
    fn different_seeds_can_produce_different_mazes() {
        let a = generate(1);
        let b = generate(2);

        assert_ne!(a.cells, b.cells);
    }

    #[test]
    fn generated_level_always_has_a_path_from_spawn_to_goal() {
        for seed in [1u64, 2, 3, 837_492, u64::MAX, 0] {
            let generated = generate(seed);

            let level = Level::from_cells(generated.cells.clone())
                .expect("el nivel generado debe ser válido");

            let distances = DistanceField::from_level(&level, level.player_spawn());

            let goal = level.goal();

            assert!(
                distances.distance_at(goal.0, goal.1).is_some(),
                "seed {seed}: la meta debe ser alcanzable desde el spawn"
            );
        }
    }

    #[test]
    fn generated_level_goal_respects_the_minimum_distance_fraction() {
        for seed in [1u64, 2, 3, 837_492] {
            let generated = generate(seed);

            let level = Level::from_cells(generated.cells.clone())
                .expect("el nivel generado debe ser válido");

            let distances = DistanceField::from_level(&level, level.player_spawn());

            let max_distance = (0..level.height())
                .flat_map(|r| (0..level.width()).map(move |c| (r, c)))
                .filter_map(|(r, c)| distances.distance_at(r, c))
                .max()
                .unwrap();

            let goal = level.goal();

            let goal_distance = distances.distance_at(goal.0, goal.1).unwrap();

            let fraction = goal_distance as f32 / max_distance as f32;

            assert!(
                fraction >= GOAL_DISTANCE_MIN_FRACTION - 0.01,
                "seed {seed}: fracción de distancia de meta {fraction} demasiado baja"
            );
        }
    }

    #[test]
    fn no_dealer_is_within_the_unsafe_spawn_radius() {
        for seed in [1u64, 2, 3, 837_492] {
            let generated = generate(seed);

            let level = Level::from_cells(generated.cells.clone())
                .expect("el nivel generado debe ser válido");

            let distances = DistanceField::from_level(&level, level.player_spawn());

            for &(r, c) in level.enemy_spawns() {
                let distance = distances
                    .distance_at(r, c)
                    .expect("todo Dealer generado debe ser alcanzable");

                assert!(
                    distance >= SAFE_SPAWN_DISTANCE_CELLS,
                    "seed {seed}: Dealer en ({r},{c}) a distancia {distance} < {SAFE_SPAWN_DISTANCE_CELLS}"
                );
            }
        }
    }

    #[test]
    fn dealers_and_pickups_never_overlap_spawn_or_goal_or_each_other() {
        for seed in [1u64, 2, 3, 837_492] {
            let generated = generate(seed);

            let level = Level::from_cells(generated.cells.clone())
                .expect("el nivel generado debe ser válido");

            let mut all = vec![level.player_spawn(), level.goal()];
            all.extend(level.enemy_spawns());
            all.extend(level.ammo_spawns());

            let unique: HashSet<(usize, usize)> = all.iter().copied().collect();

            assert_eq!(
                unique.len(),
                all.len(),
                "seed {seed}: posiciones duplicadas"
            );
        }
    }

    #[test]
    fn generated_level_has_noticeably_more_dealers_than_any_static_level() {
        for seed in [1u64, 2, 3, 837_492] {
            let generated = generate(seed);

            // House of Cards, el nivel estático con más Dealers, tiene 4.
            assert!(
                generated.dealer_count > 4,
                "seed {seed}: solo {} Dealers, se esperaban notablemente más que 4",
                generated.dealer_count
            );
        }
    }

    #[test]
    fn generated_level_is_considerably_larger_than_house_of_cards() {
        for seed in [1u64, 2, 3, 837_492] {
            let generated = generate(seed);

            let level = Level::from_cells(generated.cells.clone())
                .expect("el nivel generado debe ser válido");

            let walkable = (0..level.height())
                .flat_map(|r| (0..level.width()).map(move |c| (r, c)))
                .filter(|&(r, c)| level.is_walkable(r, c))
                .count();

            let multiplier = walkable as f32 / HOUSE_OF_CARDS_WALKABLE_CELLS as f32;

            assert!(
                (MIN_AREA_MULTIPLIER..=MAX_AREA_MULTIPLIER).contains(&multiplier),
                "seed {seed}: multiplicador de área {multiplier} fuera de rango"
            );
        }
    }

    #[test]
    fn theme_is_always_one_of_the_three_existing_identities() {
        for seed in [1u64, 2, 3, 837_492, 999_999] {
            let generated = generate(seed);

            assert!(matches!(
                generated.theme,
                LevelTheme::CrimsonEntrance | LevelTheme::BlackClub | LevelTheme::HouseOfCards
            ));
        }
    }

    #[test]
    fn fresh_seed_values_are_extremely_unlikely_to_collide() {
        let a = fresh_seed();
        let b = fresh_seed();

        // No es una garantía matemática de unicidad (el reloj puede
        // repetirse en sistemas muy rápidos), pero el PID + reloj de
        // nanosegundos hace una colisión real virtualmente imposible
        // en dos llamadas consecutivas del mismo proceso.
        assert_ne!(a, b);
    }

    #[test]
    fn fallback_maze_is_structurally_valid() {
        let generated = fallback_generated_level(0);

        let level =
            Level::from_cells(generated.cells).expect("el mapa de emergencia debe ser válido");

        let distances = DistanceField::from_level(&level, level.player_spawn());

        let goal = level.goal();

        assert!(distances.distance_at(goal.0, goal.1).is_some());
    }
}
