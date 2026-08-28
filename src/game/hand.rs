use std::collections::HashSet;

use raylib::prelude::Vector2;

use crate::world::{DistanceField, Level, Rng};

/// Límite superior ABSOLUTO de Dealers vivos simultáneos, válido para
/// los cuatro niveles: ningún `dealer_cap` por nivel puede superarlo
/// (verificado en `world::level_manager::tests`).
pub(crate) const GLOBAL_HARD_DEALER_CAP: usize = 52;

/// Duración total de la fase "The House is reloading" (mensaje +
/// cuenta regresiva 3-2-1), en segundos de tiempo de PARTIDA.
const RELOADING_PHASE_DURATION: f32 = 4.0;

/// Fin del tramo "THE HOUSE IS RELOADING..." dentro de la fase
/// (0.0-1.0s transcurridos).
const RELOADING_MESSAGE_END: f32 = 1.0;
/// Fin del tramo "NEXT HAND IN 3..." (1.0-2.0s transcurridos).
const COUNTDOWN_3_END: f32 = 2.0;
/// Fin del tramo "NEXT HAND IN 2..." (2.0-3.0s transcurridos).
const COUNTDOWN_2_END: f32 = 3.0;
/// El resto (3.0-4.0s transcurridos) es "NEXT HAND IN 1...".

/// Duración del banner "HAND N" mostrado brevemente tras spawnear una
/// nueva Hand (0.8-1.2s pedido; se usa el punto medio).
const HAND_BANNER_DURATION: f32 = 1.0;

/// Distancia mínima navegable (en pasos de `DistanceField`, no
/// euclidiana) entre el spawn de un Dealer de una Hand nueva y la
/// posición actual del jugador — misma filosofía y mismo valor que
/// `world::level_generator::SAFE_SPAWN_DISTANCE_CELLS` usa para la
/// generación inicial de "The Dealer's True Maze" (sección 13:
/// "reutiliza esa filosofía").
const SAFE_RESPAWN_DISTANCE_CELLS: u32 = 6;

/// Umbrales de distancia de seguridad, probados en orden de mayor a
/// menor: si el más estricto no produce suficientes candidatos (nivel
/// pequeño, mucho ya ocupado), se relaja progresivamente en vez de
/// fallar — nunca se bloquea un respawn completo por falta de
/// celdas perfectamente seguras.
const SAFE_DISTANCE_FALLBACKS: [u32; 4] = [SAFE_RESPAWN_DISTANCE_CELLS, 4, 2, 0];

/// Radio (en celdas) dentro del cual una celda candidata se considera
/// potencialmente "a la vista" del jugador para la heurística barata
/// de la sección 14 — nunca un raycast real contra las paredes, solo
/// una aproximación distancia+cono.
const VISIBILITY_RADIUS_CELLS: f32 = 6.0;

/// Coseno del semi-ángulo del cono de visión aproximado (60°): una
/// celda dentro de `VISIBILITY_RADIUS_CELLS` Y dentro de este cono
/// respecto a hacia dónde mira el jugador se considera "visible" y
/// se evita como spawn mientras existan alternativas.
const VISIBILITY_CONE_COS: f32 = 0.5;

/// Disparos Standard necesarios para eliminar un Dealer
/// (`player::WeaponTier::Standard.damage()` / `world::entity::
/// DEALER_MAX_HEALTH` = 50/100 = 2), duplicado aquí SOLO como
/// literal de cálculo de presupuesto de munición — mismo patrón ya
/// documentado en `world::level_generator`. El presupuesto se
/// dimensiona siempre para el arma Standard; The Royal Flush (Bloque
/// 2) solo REDUCE el consumo al hacer one-shot, nunca lo aumenta.
const SHOTS_TO_KILL_ONE_DEALER: u32 = 2;

/// Margen por errores de puntería (sección 19), igual que en la
/// generación inicial procedural.
const MISS_MARGIN_MULTIPLIER: f32 = 1.5;

/// Munición de reserva otorgada por cada `AmmoPickup`
/// (`game::session::AMMO_PER_PICKUP`), duplicado aquí por el mismo
/// motivo que las constantes anteriores.
const AMMO_PER_PICKUP: u32 = 6;

/// Tope de pickups adicionales inyectados de una sola vez al iniciar
/// una Hand por la fórmula base de presupuesto, para no inundar el
/// nivel de munición aunque el déficit calculado sea enorme. Los
/// lotes CONDICIONALES (`GameSession::spawn_intermission_supplies`)
/// tienen su propio tamaño fijo (4 o 3) y no pasan por este tope.
const MAX_EXTRA_PICKUPS_PER_HAND: usize = 6;

/// Cantidad de `AmmoPickup` que crea el Emergency Ammo Respawn
/// (anti-softlock) cuando el jugador se queda SIN munición y sin
/// recargas con enemigos todavía vivos. Deliberadamente holgado (4):
/// es la última red de seguridad y no debe volver a fallar por
/// escasez.
pub(crate) const EMERGENCY_AMMO_PICKUP_COUNT: usize = 4;

/// Banda de distancia navegable (pasos de `DistanceField`, nunca
/// euclidiana) preferida para un `AmmoPickup` de emergencia respecto
/// a la posición actual del jugador: ni debajo del jugador, ni al
/// otro extremo del mapa mientras está indefenso. Se relaja
/// progresivamente si el mapa no ofrece suficientes candidatos en la
/// banda estricta — nunca se bloquea el respawn por falta de celdas
/// perfectamente ideales.
const EMERGENCY_AMMO_DISTANCE_BANDS: [(u32, u32); 3] = [(1, 3), (1, 6), (1, u32::MAX)];

/// Objetivo mínimo/máximo (inclusive) de Health Pickups activos que
/// debe haber en el mapa al comenzar cada Hand posterior a HAND I
/// (Health Respawn por Hand, sección 13).
const MIN_HAND_HEALTH_PICKUPS: usize = 3;
const MAX_HAND_HEALTH_PICKUPS: usize = 5;

/// Multiplicador dorado (SplitMix64) ya usado por `derive_hand_seed`;
/// reutilizado aquí para derivar semillas de recursos SIN acoplar su
/// cálculo al de spawn de Dealers.
const SEED_MULTIPLIER_A: u64 = 0x9E3779B97F4A7C15;
const SEED_MULTIPLIER_B: u64 = 0xD1B54A32D192ED03;

/// Discriminadores arbitrarios pero FIJOS que separan los distintos
/// propósitos de semilla derivados de la misma `session_seed`
/// (sección 23: "level_seed + hand_number + resource discriminator").
/// Sin estos, dos sistemas distintos (por ejemplo Emergency Ammo y
/// Health Respawn) podrían derivar accidentalmente la MISMA semilla
/// para el mismo `hand_number`/índice y producir el mismo layout por
/// coincidencia, en vez de ser independientes.
const EMERGENCY_AMMO_SEED_DISCRIMINATOR: u64 = 0xE33A_9001;
const HEALTH_TARGET_SEED_DISCRIMINATOR: u64 = 0x4EA1_7002;
const HEALTH_SPAWN_SEED_DISCRIMINATOR: u64 = 0x4EA1_7003;
const ROYAL_FLUSH_SEED_DISCRIMINATOR: u64 = 0x2_0FA7_0005;
const KING_SEED_DISCRIMINATOR: u64 = 0x4_1_16_0006;
const KING_SUMMON_SEED_DISCRIMINATOR: u64 = 0x4_1_16_0007;

/// Deriva una semilla determinista a partir de `session_seed`, un
/// `discriminator` fijo por sistema, y un `index` (número de Hand o
/// contador de invocaciones). Única forma de derivar semillas de
/// recursos dinámicos del proyecto — evita que cada sistema
/// reinvente su propia fórmula de mezcla.
fn derive_resource_seed(session_seed: u64, discriminator: u64, index: u64) -> u64 {
    session_seed
        ^ discriminator.wrapping_mul(SEED_MULTIPLIER_A)
        ^ index.wrapping_mul(SEED_MULTIPLIER_B)
}

/// Fase temporal del sistema de Hands. Deliberadamente NO es un
/// `GameState`: el jugador sigue jugando con normalidad durante
/// `Reloading` (sección 8) — es solo una sub-fase dentro de
/// `Playing`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum HandPhase {
    Active,
    Reloading { countdown: f32 },
}

/// Mensaje HUD que el sistema de Hands quiere mostrar este cuadro, o
/// `None` si no hay nada que mostrar. Dominio puro: no conoce
/// `Framebuffer` ni ninguna fuente/glifo — `rendering::hud` decide
/// CÓMO dibujarlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandHudMessage {
    None,
    HouseIsReloading,
    NextHandIn(u32),
    HandBanner(usize),
}

/// Resultado de `HordeManager::tick` en el cuadro exacto en que una
/// Hand nueva comienza (Bloque 1, Commit 07).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandOutcome {
    /// Hand normal: el llamador debe spawnear `dealer_count` Dealers
    /// nuevos, exactamente como antes de este commit.
    Spawn { dealer_count: usize },

    /// Se alcanzó la Hand reservada como final para este nivel
    /// (`LevelManager::current_horde_hand_config().final_hand_number`).
    /// Todavía no existe The King (Bloque 3), así que esta Hand no
    /// spawnea ningún Dealer — el llamador no debe crear entidades ni
    /// tocar munición/vida para ella.
    FinalHandReached,
}

/// Administra la progresión de Horde ("Dealer Hands") para UNA
/// `GameSession`: qué Hand está activa, si el jugador está en la
/// intermisión de recarga, y el conteo de la última Hand spawneada
/// (base para decidir la siguiente).
///
/// Pertenece a la sesión/nivel, nunca a `AudioManager`/rendering/
/// `App`. `GameSession::new` siempre arranca en `HAND I`
/// (`hand_number: 1`, `phase: Active`) — Retry y cambio de nivel
/// reconstruyen una `GameSession` enteramente nueva (mismo mecanismo
/// ya establecido para vida/arma/pickups), así que este estado nunca
/// sobrevive entre partidas sin que nadie tenga que resetearlo campo
/// por campo.
///
/// `GameSession::update_hand_state` sigue siendo quien decide QUÉ
/// spawnear (posiciones, munición, vida) cuando `tick` reporta una
/// Hand nueva — este tipo solo posee la máquina de estados de
/// progresión en sí, sin tocar `Level`/`Entity`/pickups.
pub(crate) struct HordeManager {
    hand_number: usize,
    previous_spawn_count: usize,
    phase: HandPhase,
    banner_remaining: f32,
}

impl HordeManager {
    /// Construye el estado inicial: HAND I, con `initial_dealer_count`
    /// Dealers ya colocados por el nivel (estático o procedural) —
    /// esta llamada NO spawnea nada, solo registra cuántos había para
    /// que la Hand II sepa a partir de qué número doblar.
    pub(crate) fn new(initial_dealer_count: usize) -> Self {
        Self {
            hand_number: 1,
            previous_spawn_count: initial_dealer_count,
            phase: HandPhase::Active,
            banner_remaining: 0.0,
        }
    }

    pub(crate) fn hand_number(&self) -> usize {
        self.hand_number
    }

    /// Solo usado por pruebas (verificar la fase interna
    /// directamente, sin pasar por `hud_message`): no forma parte de
    /// la API que `GameSession`/`App` consumen en producción, que
    /// solo necesitan `hud_message`/`tick`.
    #[cfg(test)]
    pub(crate) fn phase(&self) -> HandPhase {
        self.phase
    }

    /// Mensaje HUD correspondiente al instante actual.
    pub(crate) fn hud_message(&self) -> HandHudMessage {
        match self.phase {
            HandPhase::Reloading { countdown } => {
                let elapsed = RELOADING_PHASE_DURATION - countdown;

                if elapsed < RELOADING_MESSAGE_END {
                    HandHudMessage::HouseIsReloading
                } else if elapsed < COUNTDOWN_3_END {
                    HandHudMessage::NextHandIn(3)
                } else if elapsed < COUNTDOWN_2_END {
                    HandHudMessage::NextHandIn(2)
                } else {
                    HandHudMessage::NextHandIn(1)
                }
            }

            HandPhase::Active => {
                if self.banner_remaining > 0.0 {
                    HandHudMessage::HandBanner(self.hand_number)
                } else {
                    HandHudMessage::None
                }
            }
        }
    }

    /// Avanza el sistema de Hands un cuadro. Debe llamarse
    /// EXCLUSIVAMENTE desde el update jugable (`App::update_playing`,
    /// vía `GameSession`) para que `Paused`/`Victory`/`Defeat` lo
    /// congelen automáticamente sin ningún caso especial — mismo
    /// patrón ya establecido para el resto de temporizadores de
    /// partida (nunca reloj de pared).
    ///
    /// Retorna `Some(HandOutcome)` EXACTAMENTE en el cuadro en que
    /// corresponde comenzar la siguiente Hand (el llamador es quien
    /// realmente coloca las entidades para `Spawn`); `None` en
    /// cualquier otro cuadro. El countdown/mensaje de recarga avanza
    /// igual sin importar si la Hand que viene resulta `Spawn` o
    /// `FinalHandReached` — la diferencia solo importa para lo que el
    /// llamador hace DESPUÉS de recibir el resultado.
    ///
    /// `alive_dealer_count` es la fuente de verdad de "la Hand
    /// terminó" (sección 6): nunca `entities.len()`/`is_empty()`,
    /// que seguirían contando cadáveres durante sus 15s de despawn.
    ///
    /// `final_hand_number` (Bloque 1, Commit 07) es el número de Hand
    /// reservado para la ronda final de este nivel
    /// (`LevelManager::current_horde_hand_config`): en cuanto la
    /// siguiente Hand alcanza ese número, el doblado se detiene y se
    /// reporta `FinalHandReached` en su lugar — `previous_spawn_count`
    /// queda congelado en su último valor real, sin significado una
    /// vez alcanzada la ronda final.
    pub(crate) fn tick(
        &mut self,
        delta_time: f32,
        alive_dealer_count: usize,
        level_cap: usize,
        final_hand_number: usize,
    ) -> Option<HandOutcome> {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return None;
        }

        // El límite superior ABSOLUTO nunca depende de que el
        // llamador haya pasado un `level_cap` correcto: se aplica
        // aquí también, como última línea de defensa.
        let level_cap = level_cap.min(GLOBAL_HARD_DEALER_CAP);

        match &mut self.phase {
            HandPhase::Active => {
                if self.banner_remaining > 0.0 {
                    self.banner_remaining = (self.banner_remaining - delta_time).max(0.0);
                }

                if alive_dealer_count == 0 {
                    self.phase = HandPhase::Reloading {
                        countdown: RELOADING_PHASE_DURATION,
                    };
                }

                None
            }

            HandPhase::Reloading { countdown } => {
                *countdown -= delta_time;

                if *countdown > 0.0 {
                    return None;
                }

                let next_hand_number = self.hand_number + 1;

                self.hand_number = next_hand_number;
                self.phase = HandPhase::Active;

                if next_hand_number >= final_hand_number {
                    // La ronda final ES el combate contra The King, no
                    // una Hand numerada de Dealers: NO se muestra el
                    // banner "HAND N" (se solaparía con la barra de
                    // vida del jefe, que ocupa la misma franja
                    // superior-centro). The King tiene su propia
                    // etiqueta "THE KING" + barra.
                    self.banner_remaining = 0.0;
                    return Some(HandOutcome::FinalHandReached);
                }

                self.banner_remaining = HAND_BANNER_DURATION;

                // La progresión nunca depende de `previous_spawn_count`
                // en 0 (un nivel estático hipotético sin Dealers
                // iniciales no debe quedar atascado doblando 0 para
                // siempre).
                let next_count = (self.previous_spawn_count.max(1) * 2).min(level_cap);

                self.previous_spawn_count = next_count;

                Some(HandOutcome::Spawn {
                    dealer_count: next_count,
                })
            }
        }
    }
}

/// Elige, de forma determinista, cuántos Dealers trae la HAND I de
/// una sesión de Horde (Bloque 1, Commit 07): un valor uniforme en
/// `first_hand_min..=first_hand_max`. Con `first_hand_min ==
/// first_hand_max` (los tres niveles estáticos: la RNG nunca produce
/// más de un resultado posible) retorna siempre ese mismo valor fijo.
///
/// `GameSession` usa el resultado para completar, mediante
/// `select_spawn_cells` (la MISMA infraestructura que ya usan las
/// Hands II+), los Dealers que el mapa no trae de fábrica —
/// `first_hand_min`/`first_hand_max` nunca modifican el mapa ni el
/// conteo de enemigos de Portal Mode.
pub(crate) fn first_hand_dealer_count(
    session_seed: u64,
    first_hand_min: usize,
    first_hand_max: usize,
) -> usize {
    if first_hand_min >= first_hand_max {
        return first_hand_min;
    }

    const FIRST_HAND_SEED_DISCRIMINATOR: u64 = 0x4E64_7004;

    let seed = derive_resource_seed(session_seed, FIRST_HAND_SEED_DISCRIMINATOR, 0);

    let mut rng = Rng::new(seed);

    first_hand_min + rng.gen_range(first_hand_max - first_hand_min + 1)
}

/// Deriva una semilla determinista para la Hand `hand_number` a
/// partir de la semilla de sesión (sección 17): "misma level seed +
/// mismo hand index -> mismo layout". Mismo patrón exacto que
/// `world::level_generator` ya usa para derivar semillas de reintento
/// a partir de una semilla base.
fn derive_hand_seed(session_seed: u64, hand_number: usize) -> u64 {
    session_seed ^ (hand_number as u64).wrapping_mul(0x9E3779B97F4A7C15)
}

fn world_to_cell(position: Vector2, block_size: usize) -> (usize, usize) {
    if block_size == 0 || !position.x.is_finite() || !position.y.is_finite() {
        return (0, 0);
    }

    let column = (position.x / block_size as f32).floor().max(0.0) as usize;

    let row = (position.y / block_size as f32).floor().max(0.0) as usize;

    (row, column)
}

/// `true` si la celda de caracteres `(row, column)` cae dentro del
/// cono de visión aproximado del jugador — heurística barata
/// (distancia + producto punto), nunca un raycast real (sección 14:
/// "no necesitas implementar un sistema extremadamente costoso").
fn is_immediately_visible(
    row: usize,
    column: usize,
    player_position: Vector2,
    player_facing: f32,
    block_size: usize,
) -> bool {
    let half_block = block_size as f32 / 2.0;

    let cell_x = column as f32 * block_size as f32 + half_block;
    let cell_y = row as f32 * block_size as f32 + half_block;

    let dx = cell_x - player_position.x;
    let dy = cell_y - player_position.y;

    let distance = dx.hypot(dy);

    let visibility_radius = block_size as f32 * VISIBILITY_RADIUS_CELLS;

    if distance > visibility_radius || distance <= f32::EPSILON {
        return false;
    }

    let facing_x = player_facing.cos();
    let facing_y = player_facing.sin();

    let dot = (dx / distance) * facing_x + (dy / distance) * facing_y;

    dot >= VISIBILITY_CONE_COS
}

/// Recolecta hasta `count` celdas válidas para spawnear Dealers de
/// una Hand nueva (secciones 12-15): transitables, no ocupadas
/// (jugador/meta/pickup/otro Dealer/cadáver todavía visible — todo
/// ya viene precomputado en `occupied`), a distancia navegable segura
/// del jugador, priorizando celdas fuera de visión inmediata.
///
/// `use_clusters` reproduce, cuando es `true` (solo "The Dealer's
/// True Maze"), la misma filosofía de individuales/grupos/plagas que
/// la generación inicial procedural (sección 15/16); en `false`
/// (niveles estáticos) simplemente dispersa Dealers individuales por
/// las celdas elegibles ya barajadas.
///
/// Nunca promete más posiciones de las que el mapa realmente permite:
/// si `count` excede las celdas elegibles disponibles, retorna menos.
pub(crate) fn select_spawn_cells(
    level: &Level,
    player_position: Vector2,
    player_facing: f32,
    block_size: usize,
    occupied: &HashSet<(usize, usize)>,
    count: usize,
    use_clusters: bool,
    seed: u64,
) -> Vec<(usize, usize)> {
    if count == 0 {
        return Vec::new();
    }

    let mut rng = Rng::new(seed);

    let player_cell = world_to_cell(player_position, block_size);

    let distances = DistanceField::from_level(level, player_cell);

    let height = level.height();
    let width = level.width();

    let mut ordered_pool = Vec::new();

    for &threshold in &SAFE_DISTANCE_FALLBACKS {
        let mut not_visible = Vec::new();
        let mut visible = Vec::new();

        for row in 0..height {
            for column in 0..width {
                let cell = (row, column);

                if !level.is_walkable(row, column) {
                    continue;
                }

                if cell == player_cell || occupied.contains(&cell) {
                    continue;
                }

                let Some(distance) = distances.distance_at(row, column) else {
                    continue;
                };

                if distance < threshold {
                    continue;
                }

                if is_immediately_visible(row, column, player_position, player_facing, block_size) {
                    visible.push(cell);
                } else {
                    not_visible.push(cell);
                }
            }
        }

        let total_candidates = not_visible.len() + visible.len();

        if total_candidates >= count || threshold == 0 {
            not_visible.sort();
            visible.sort();

            rng.shuffle(&mut not_visible);
            rng.shuffle(&mut visible);

            ordered_pool = not_visible;
            ordered_pool.extend(visible);

            break;
        }
    }

    if use_clusters {
        select_with_clusters(level, &ordered_pool, count, &mut rng)
    } else {
        ordered_pool.into_iter().take(count).collect()
    }
}

/// Bandas de distancia navegable para la oleada de CASTIGO de la fase
/// de huida de The King: a diferencia del resto de invocaciones (que
/// respetan `SAFE_DISTANCE_FALLBACKS`), estos Dealers aparecen CERCA y
/// RODEANDO al jugador — la intención es que, si no rematas al Rey en
/// los 20 s, el ejército te caiga encima. Se relaja si el mapa no
/// ofrece candidatos en la banda estricta.
const ENCIRCLE_DISTANCE_FALLBACKS: [(u32, u32); 3] = [(2, 5), (2, 8), (1, u32::MAX)];

/// Coloca `count` celdas navegables CERCA del jugador y repartidas por
/// octantes a su alrededor (no a distancia segura). Uso: la oleada de
/// castigo de la huida de The King.
pub(crate) fn select_encircling_cells(
    level: &Level,
    player_position: Vector2,
    block_size: usize,
    occupied: &HashSet<(usize, usize)>,
    count: usize,
    seed: u64,
) -> Vec<(usize, usize)> {
    if count == 0 {
        return Vec::new();
    }

    let mut rng = Rng::new(seed);
    let player_cell = world_to_cell(player_position, block_size);
    let distances = DistanceField::from_level(level, player_cell);

    let mut pool: Vec<(usize, usize)> = Vec::new();
    for &(low, high) in &ENCIRCLE_DISTANCE_FALLBACKS {
        pool.clear();
        for row in 0..level.height() {
            for column in 0..level.width() {
                let cell = (row, column);

                if !level.is_walkable(row, column)
                    || cell == player_cell
                    || occupied.contains(&cell)
                {
                    continue;
                }

                let Some(distance) = distances.distance_at(row, column) else {
                    continue;
                };

                if distance >= low && distance <= high {
                    pool.push(cell);
                }
            }
        }

        if pool.len() >= count {
            break;
        }
    }

    pool.sort();
    rng.shuffle(&mut pool);

    // Reparto por octantes: la oleada rodea al jugador en vez de
    // amontonarse en una sola dirección.
    let mut buckets: [Vec<(usize, usize)>; 8] = std::array::from_fn(|_| Vec::new());
    for &cell in &pool {
        let dy = cell.0 as f32 - player_cell.0 as f32;
        let dx = cell.1 as f32 - player_cell.1 as f32;
        let mut octant = (dy.atan2(dx) / std::f32::consts::FRAC_PI_4).round() as i32 % 8;
        if octant < 0 {
            octant += 8;
        }
        buckets[octant as usize].push(cell);
    }

    let mut chosen: Vec<(usize, usize)> = Vec::with_capacity(count);
    let mut progress = true;
    while chosen.len() < count && progress {
        progress = false;
        for bucket in buckets.iter_mut() {
            if chosen.len() >= count {
                break;
            }
            if let Some(cell) = bucket.pop() {
                chosen.push(cell);
                progress = true;
            }
        }
    }

    chosen
}

/// Banda de distancia navegable de la mitad "accesible pero fuera de
/// vista" de la munición de Hand: lo bastante cerca para recogerla
/// con un rodeo corto y sin mucho peligro, pero NO delante del
/// jugador — tiene que girar o avanzar un poco para encontrarla. Se
/// relaja si el mapa no ofrece candidatos en la banda estricta.
const ACCESSIBLE_AMMO_DISTANCE_BANDS: [(u32, u32); 3] = [(2, 5), (1, 9), (1, u32::MAX)];

/// Separación navegable (distancia Chebyshev en celdas) mínima
/// DESEADA entre dos `AmmoPickup` de la misma tanda, para que no
/// aparezcan amontonados. Se relaja progresivamente hasta `0` si el
/// mapa no da para tanto.
const MIN_AMMO_SEPARATION_CELLS: i32 = 3;

/// Distancia Chebyshev (en celdas de cuadrícula) entre dos celdas.
fn cell_chebyshev(a: (usize, usize), b: (usize, usize)) -> i32 {
    let dr = (a.0 as i32 - b.0 as i32).abs();
    let dc = (a.1 as i32 - b.1 as i32).abs();
    dr.max(dc)
}

/// Añade a `chosen` hasta `want` celdas de `pool` (ya barajado),
/// prefiriendo mantener al menos `MIN_AMMO_SEPARATION_CELLS` de
/// separación con TODAS las ya elegidas; si no llega, afloja la
/// separación exigida hasta `0`. Nunca repite una celda ya elegida.
fn take_spaced(pool: &[(usize, usize)], want: usize, chosen: &mut Vec<(usize, usize)>) {
    if want == 0 {
        return;
    }

    let target = chosen.len() + want;

    for min_sep in (0..=MIN_AMMO_SEPARATION_CELLS).rev() {
        for &cell in pool {
            if chosen.len() >= target {
                return;
            }

            if chosen.contains(&cell) {
                continue;
            }

            if chosen
                .iter()
                .all(|&other| cell_chebyshev(other, cell) >= min_sep)
            {
                chosen.push(cell);
            }
        }

        if chosen.len() >= target {
            return;
        }
    }
}

/// Distancia navegable MÍNIMA de la parte de "difícil acceso" de un
/// lote de munición: el jugador tiene que hacer un trayecto real para
/// recogerla (nunca a su lado). Se relaja si el mapa no ofrece
/// candidatos tan lejanos.
const HARD_AMMO_MIN_DISTANCE_FALLBACKS: [u32; 3] = [6, 4, 1];

/// Selecciona celdas para un lote de munición de Hand repartido en
/// DOS grupos, para que no se amontone y para que no todo sea
/// trivial de recoger:
///
/// - `easy_count` en celdas de FÁCIL ACCESO: cercanas (banda
///   `ACCESSIBLE_AMMO_DISTANCE_BANDS`) y fuera del cono de visión
///   inmediato — accesibles con un rodeo corto y de bajo riesgo;
/// - `hard_count` en celdas de DIFÍCIL ACCESO: a partir de
///   `HARD_AMMO_MIN_DISTANCE_FALLBACKS` pasos navegables, en cualquier
///   punto del mapa — hay que ir a por ellas.
///
/// Dentro del lote se mantiene separación (`MIN_AMMO_SEPARATION_CELLS`)
/// entre pickups, relajándola solo si el mapa es demasiado pequeño; si
/// un grupo no puede llenarse, el otro absorbe lo que falte. Nunca la
/// celda del jugador, ni una pared, ni una celda ya ocupada.
/// Determinista para una misma `seed`.
pub(crate) fn select_split_ammo_cells(
    level: &Level,
    player_position: Vector2,
    player_facing: f32,
    block_size: usize,
    occupied: &HashSet<(usize, usize)>,
    easy_count: usize,
    hard_count: usize,
    seed: u64,
) -> Vec<(usize, usize)> {
    let total = easy_count + hard_count;

    if total == 0 {
        return Vec::new();
    }

    let mut rng = Rng::new(seed);

    let player_cell = world_to_cell(player_position, block_size);

    let distances = DistanceField::from_level(level, player_cell);

    let height = level.height();
    let width = level.width();

    let is_eligible = |cell: (usize, usize)| {
        level.is_walkable(cell.0, cell.1)
            && cell != player_cell
            && !occupied.contains(&cell)
            && distances.distance_at(cell.0, cell.1).is_some()
    };

    // --- Grupo FÁCIL: cercano, alcanzable, fuera de vista inmediata. ---
    let mut easy_pool: Vec<(usize, usize)> = Vec::new();

    for &(low, high) in &ACCESSIBLE_AMMO_DISTANCE_BANDS {
        easy_pool.clear();

        for row in 0..height {
            for column in 0..width {
                let cell = (row, column);

                if !is_eligible(cell) {
                    continue;
                }

                let distance = distances.distance_at(row, column).unwrap_or(0);

                if distance < low.max(1) || distance > high {
                    continue;
                }

                if is_immediately_visible(row, column, player_position, player_facing, block_size) {
                    continue;
                }

                easy_pool.push(cell);
            }
        }

        if easy_pool.len() >= easy_count || high == u32::MAX {
            break;
        }
    }

    easy_pool.sort();
    rng.shuffle(&mut easy_pool);

    // --- Grupo DIFÍCIL: lejos, en cualquier punto del mapa. ---
    let mut hard_pool: Vec<(usize, usize)> = Vec::new();

    for &min_distance in &HARD_AMMO_MIN_DISTANCE_FALLBACKS {
        hard_pool.clear();

        for row in 0..height {
            for column in 0..width {
                let cell = (row, column);

                if !is_eligible(cell) {
                    continue;
                }

                if distances.distance_at(row, column).unwrap_or(0) < min_distance {
                    continue;
                }

                hard_pool.push(cell);
            }
        }

        if hard_pool.len() >= hard_count || min_distance == 1 {
            break;
        }
    }

    hard_pool.sort();
    rng.shuffle(&mut hard_pool);

    // --- Selección espaciada: primero el grupo fácil, luego el
    //     difícil; si un grupo se queda corto el otro lo completa, y
    //     si aún falta (mapa diminuto) se rellena sin exigir
    //     separación. ---
    let mut chosen: Vec<(usize, usize)> = Vec::with_capacity(total);

    take_spaced(&easy_pool, easy_count, &mut chosen);
    take_spaced(&hard_pool, total - chosen.len(), &mut chosen);
    take_spaced(&easy_pool, total - chosen.len(), &mut chosen);

    if chosen.len() < total {
        for &cell in hard_pool.iter().chain(easy_pool.iter()) {
            if chosen.len() >= total {
                break;
            }
            if !chosen.contains(&cell) {
                chosen.push(cell);
            }
        }
    }

    chosen
}

/// Coloca hasta `count` posiciones agrupadas en pequeños racimos (1-2
/// plagas de 4-6, un puñado de grupos de 2-3, el resto individuales),
/// tomando los orígenes de racimo de `ordered_pool` (ya priorizado
/// fuera-de-visión y barajado) — misma filosofía de distribución que
/// `world::level_generator::try_generate` usa para la generación
/// inicial de "The Dealer's True Maze".
fn select_with_clusters(
    level: &Level,
    ordered_pool: &[(usize, usize)],
    count: usize,
    rng: &mut Rng,
) -> Vec<(usize, usize)> {
    let mut chosen: HashSet<(usize, usize)> = HashSet::new();
    let mut result = Vec::with_capacity(count);

    let plague_zone_count = if count >= 22 { 2 } else { 1 };

    let mut pool_cursor = 0usize;

    let place_cluster = |origin: (usize, usize),
                         size: usize,
                         chosen: &mut HashSet<(usize, usize)>,
                         result: &mut Vec<(usize, usize)>| {
        let mut placed = 0;

        let mut frontier = vec![origin];

        while placed < size {
            let Some(candidate) = frontier.pop() else {
                break;
            };

            if chosen.contains(&candidate) {
                continue;
            }

            chosen.insert(candidate);
            result.push(candidate);
            placed += 1;

            let neighbors: [Option<(usize, usize)>; 4] = [
                candidate.0.checked_sub(1).map(|r| (r, candidate.1)),
                Some((candidate.0 + 1, candidate.1)),
                candidate.1.checked_sub(1).map(|c| (candidate.0, c)),
                Some((candidate.0, candidate.1 + 1)),
            ];

            for neighbor in neighbors.into_iter().flatten() {
                if level.is_walkable(neighbor.0, neighbor.1) && !chosen.contains(&neighbor) {
                    frontier.push(neighbor);
                }
            }
        }
    };

    for zone in 0..plague_zone_count {
        if result.len() >= count {
            break;
        }

        while pool_cursor < ordered_pool.len() && chosen.contains(&ordered_pool[pool_cursor]) {
            pool_cursor += 1;
        }

        if pool_cursor >= ordered_pool.len() {
            break;
        }

        let origin = ordered_pool[pool_cursor];
        pool_cursor += 1;

        let remaining_budget = count - result.len();

        let size = 4 + rng.gen_range(3); // 4..=6

        place_cluster(origin, size.min(remaining_budget), &mut chosen, &mut result);

        let _ = zone;
    }

    let small_group_target = (count / 6).max(2);

    for _ in 0..small_group_target {
        if result.len() >= count {
            break;
        }

        while pool_cursor < ordered_pool.len() && chosen.contains(&ordered_pool[pool_cursor]) {
            pool_cursor += 1;
        }

        if pool_cursor >= ordered_pool.len() {
            break;
        }

        let origin = ordered_pool[pool_cursor];
        pool_cursor += 1;

        let remaining_budget = count - result.len();

        let size = 2 + rng.gen_range(2); // 2..=3

        place_cluster(origin, size.min(remaining_budget), &mut chosen, &mut result);
    }

    while result.len() < count && pool_cursor < ordered_pool.len() {
        let candidate = ordered_pool[pool_cursor];
        pool_cursor += 1;

        if chosen.contains(&candidate) {
            continue;
        }

        chosen.insert(candidate);
        result.push(candidate);
    }

    result
}

/// Semilla determinista de spawn para la Hand `hand_number` de una
/// sesión con semilla base `session_seed` (sección 17). Expuesta para
/// que `GameSession` (que orquesta cuándo llamar a `select_spawn_cells`)
/// no tenga que reimplementar la derivación.
pub(crate) fn spawn_seed_for_hand(session_seed: u64, hand_number: usize) -> u64 {
    derive_hand_seed(session_seed, hand_number)
}

/// Cuántos `AmmoPickup` adicionales conviene inyectar al comenzar una
/// Hand con `new_hand_dealer_count` Dealers, dado que el jugador ya
/// tiene `accessible_ammo` balas alcanzables (cargador + reserva +
/// pickups activos × munición por pickup) — sección 19. Misma fórmula
/// (sin doblar) que `world::level_generator::ammo_pickup_budget`, pero
/// relativa a la munición YA disponible en vez de partir de cero.
///
/// Es la ruta BASE: `GameSession::spawn_intermission_supplies` la usa
/// solo cuando el jugador NO cae en ninguno de los dos casos
/// especiales de reabastecimiento (escaso, o reservas medias con
/// munición aún en el suelo), que fijan su propio tamaño de lote.
pub(crate) fn extra_ammo_pickups_needed(
    new_hand_dealer_count: usize,
    accessible_ammo: u32,
) -> usize {
    let shots_needed = new_hand_dealer_count as u32 * SHOTS_TO_KILL_ONE_DEALER;

    let shots_with_margin = (shots_needed as f32 * MISS_MARGIN_MULTIPLIER).ceil() as u32;

    let deficit = shots_with_margin.saturating_sub(accessible_ammo);

    let pickups = deficit.div_ceil(AMMO_PER_PICKUP) as usize;

    pickups.min(MAX_EXTRA_PICKUPS_PER_HAND)
}

/// Semilla determinista para la N-ésima activación del Emergency Ammo
/// Respawn de esta sesión (`spawn_index`: `0`, `1`, `2`, ... — cada
/// activación real incrementa el contador, nunca cada cuadro). Misma
/// `session_seed` + mismo `spawn_index` -> exactamente las mismas
/// posiciones, sin depender de reloj de pared ni del orden de
/// iteración de ninguna colección.
pub(crate) fn spawn_seed_for_emergency_ammo(session_seed: u64, spawn_index: u64) -> u64 {
    derive_resource_seed(session_seed, EMERGENCY_AMMO_SEED_DISCRIMINATOR, spawn_index)
}

/// Cuántos Health Pickups deben estar activos como objetivo al
/// comenzar la Hand `hand_number` (Health Respawn por Hand, sección
/// 15): un valor determinista en `3..=5`, derivado de `session_seed`
/// y `hand_number` — misma semilla + mismo número de Hand siempre
/// produce el mismo objetivo.
pub(crate) fn health_pickup_target_for_hand(session_seed: u64, hand_number: usize) -> usize {
    let seed = derive_resource_seed(
        session_seed,
        HEALTH_TARGET_SEED_DISCRIMINATOR,
        hand_number as u64,
    );

    let mut rng = Rng::new(seed);

    MIN_HAND_HEALTH_PICKUPS + rng.gen_range(MAX_HAND_HEALTH_PICKUPS - MIN_HAND_HEALTH_PICKUPS + 1)
}

/// Semilla determinista de la posición de The Royal Flush (Bloque 2,
/// Commit 15). Una sola aparición por run, así que no necesita un
/// `index`: siempre la misma celda para la misma `session_seed`. Su
/// discriminador propio garantiza que nunca coincide por casualidad
/// con el layout de Dealers, munición de emergencia ni Health Pickups
/// de ninguna Hand.
pub(crate) fn spawn_seed_for_royal_flush(session_seed: u64) -> u64 {
    derive_resource_seed(session_seed, ROYAL_FLUSH_SEED_DISCRIMINATOR, 0)
}

/// Semilla determinista de la celda de aparición de The King (Bloque
/// 3, Commit 24). Un solo spawn por run, sin `index`; discriminador
/// propio para que nunca coincida con el layout de Dealers, munición,
/// vida ni The Royal Flush.
pub(crate) fn spawn_seed_for_king(session_seed: u64) -> u64 {
    derive_resource_seed(session_seed, KING_SEED_DISCRIMINATOR, 0)
}

/// Semilla determinista de las celdas donde The King invoca su
/// cohorte de Dealers en el umbral `threshold_index` (0..4 →
/// 800/600/400/200 HP; Bloque 4, Commit 38). Discriminador propio y
/// distinto por umbral, así que las cuatro invocaciones de una misma
/// run reparten sus Dealers por sitios diferentes y nunca coinciden
/// por casualidad con el layout de ninguna Hand, la munición, la
/// vida, The Royal Flush ni la celda del propio King.
pub(crate) fn spawn_seed_for_king_summon(session_seed: u64, threshold_index: usize) -> u64 {
    derive_resource_seed(
        session_seed,
        KING_SUMMON_SEED_DISCRIMINATOR,
        threshold_index as u64,
    )
}

/// Semilla determinista de posiciones para los Health Pickups nuevos
/// de la Hand `hand_number` (sección 23): independiente de
/// `spawn_seed_for_hand`/`spawn_seed_for_emergency_ammo` gracias al
/// discriminador propio, así que nunca coincide con el layout de
/// Dealers ni con el de munición de emergencia por casualidad.
pub(crate) fn spawn_seed_for_health_replenish(session_seed: u64, hand_number: usize) -> u64 {
    derive_resource_seed(
        session_seed,
        HEALTH_SPAWN_SEED_DISCRIMINATOR,
        hand_number as u64,
    )
}

/// Recolecta hasta `count` celdas para el Emergency Ammo Respawn
/// (secciones 8-10): transitables, alcanzables, no ocupadas
/// (jugador/meta/Dealer vivo/cadáver/otro pickup — todo ya
/// precomputado en `occupied`), dentro de una banda de distancia
/// navegable razonable respecto al jugador — ni la misma celda, ni al
/// otro extremo del mapa.
///
/// A diferencia de `select_spawn_cells` (que busca la distancia
/// SEGURA MÁXIMA posible para Dealers), esta función busca una banda
/// deliberadamente CERCANA: el jugador acaba de quedarse sin
/// munición y necesita poder alcanzarla. La banda se relaja
/// progresivamente (`EMERGENCY_AMMO_DISTANCE_BANDS`) solo si el mapa
/// no ofrece suficientes candidatos en la banda estricta.
pub(crate) fn select_emergency_ammo_cells(
    level: &Level,
    player_position: Vector2,
    block_size: usize,
    occupied: &HashSet<(usize, usize)>,
    count: usize,
    seed: u64,
) -> Vec<(usize, usize)> {
    if count == 0 {
        return Vec::new();
    }

    let mut rng = Rng::new(seed);

    let player_cell = world_to_cell(player_position, block_size);

    let distances = DistanceField::from_level(level, player_cell);

    let height = level.height();
    let width = level.width();

    for &(min_distance, max_distance) in &EMERGENCY_AMMO_DISTANCE_BANDS {
        let mut candidates = Vec::new();

        for row in 0..height {
            for column in 0..width {
                let cell = (row, column);

                if !level.is_walkable(row, column) {
                    continue;
                }

                if cell == player_cell || occupied.contains(&cell) {
                    continue;
                }

                let Some(distance) = distances.distance_at(row, column) else {
                    continue;
                };

                if distance < min_distance || distance > max_distance {
                    continue;
                }

                candidates.push(cell);
            }
        }

        if candidates.len() >= count || max_distance == u32::MAX {
            candidates.sort();

            rng.shuffle(&mut candidates);

            // Reparte los pickups de emergencia sin amontonarlos
            // (misma regla que la munición de Hand), aunque sigan
            // todos dentro de la banda cercana.
            let mut chosen = Vec::with_capacity(count);
            take_spaced(&candidates, count, &mut chosen);
            return chosen;
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Doblado y cap ---

    #[test]
    fn starts_at_hand_one_with_the_level_initial_count() {
        let state = HordeManager::new(4);

        assert_eq!(state.hand_number(), 1);
        assert_eq!(state.phase(), HandPhase::Active);
    }

    /// Simula la fase completa de recarga hasta que la siguiente Hand
    /// comienza, y retorna cuántos Dealers trajo — nunca usada con un
    /// `final_hand_number` alcanzable dentro de la ventana de prueba
    /// (`usize::MAX` en todos los llamadores), así que siempre debe
    /// resultar en `HandOutcome::Spawn`.
    fn run_full_reload(state: &mut HordeManager, level_cap: usize) -> usize {
        // Último Dealer muere.
        assert_eq!(state.tick(0.016, 0, level_cap, usize::MAX), None);

        // 4 segundos completos de countdown, en pasos pequeños.
        let mut spawned = None;

        let mut elapsed = 0.0;

        while elapsed < 4.5 {
            if let Some(outcome) = state.tick(0.05, 0, level_cap, usize::MAX) {
                spawned = Some(outcome);
                break;
            }

            elapsed += 0.05;
        }

        match spawned.expect("la Hand debe spawnear dentro de ~4s") {
            HandOutcome::Spawn { dealer_count } => dealer_count,

            HandOutcome::FinalHandReached => {
                panic!("run_full_reload no debe alcanzar la Hand final reservada en este test")
            }
        }
    }

    #[test]
    fn hand_two_doubles_hand_one() {
        let mut state = HordeManager::new(4);

        let hand_two_count = run_full_reload(&mut state, 100);

        assert_eq!(hand_two_count, 8);
        assert_eq!(state.hand_number(), 2);
    }

    #[test]
    fn hand_three_doubles_hand_two() {
        let mut state = HordeManager::new(4);

        run_full_reload(&mut state, 100);
        let hand_three_count = run_full_reload(&mut state, 100);

        assert_eq!(hand_three_count, 16);
        assert_eq!(state.hand_number(), 3);
    }

    #[test]
    fn doubling_never_exceeds_the_level_cap() {
        let mut state = HordeManager::new(23);

        let hand_two = run_full_reload(&mut state, 50);
        assert_eq!(hand_two, 46);

        let hand_three = run_full_reload(&mut state, 50);
        assert_eq!(hand_three, 50);

        let hand_four = run_full_reload(&mut state, 50);
        assert_eq!(hand_four, 50);
    }

    #[test]
    fn cap_is_never_allowed_to_exceed_the_global_hard_cap_in_this_test_matrix() {
        let mut state = HordeManager::new(23);

        for _ in 0..5 {
            let count = run_full_reload(&mut state, GLOBAL_HARD_DEALER_CAP);

            assert!(count <= GLOBAL_HARD_DEALER_CAP);
        }
    }

    #[test]
    fn zero_initial_dealers_does_not_get_stuck_doubling_zero() {
        let mut state = HordeManager::new(0);

        let hand_two = run_full_reload(&mut state, 10);

        assert_eq!(hand_two, 2);
    }

    // --- Detección de fin de Hand y fase Reloading ---

    #[test]
    fn active_with_dealers_alive_never_enters_reloading() {
        let mut state = HordeManager::new(4);

        for _ in 0..120 {
            assert_eq!(state.tick(0.016, 3, 100, usize::MAX), None);
        }

        assert_eq!(state.phase(), HandPhase::Active);
    }

    #[test]
    fn last_dealer_death_immediately_starts_reloading() {
        let mut state = HordeManager::new(4);

        state.tick(0.016, 0, 100, usize::MAX);

        assert!(matches!(state.phase(), HandPhase::Reloading { .. }));
    }

    #[test]
    fn invalid_delta_time_does_not_advance_the_phase() {
        let mut state = HordeManager::new(4);

        state.tick(0.016, 0, 100, usize::MAX);

        let phase_before = state.phase();

        state.tick(0.0, 0, 100, usize::MAX);
        state.tick(-1.0, 0, 100, usize::MAX);
        state.tick(f32::NAN, 0, 100, usize::MAX);

        assert_eq!(state.phase(), phase_before);
    }

    // --- Mensajes HUD / countdown ---

    #[test]
    fn hud_sequence_matches_the_documented_timeline() {
        let mut state = HordeManager::new(4);

        state.tick(0.016, 0, 100, usize::MAX);

        // t=0.5s: mensaje de "reloading".
        state.tick(0.5 - 0.016, 0, 100, usize::MAX);
        assert_eq!(state.hud_message(), HandHudMessage::HouseIsReloading);

        // t=1.5s: NEXT HAND IN 3.
        state.tick(1.0, 0, 100, usize::MAX);
        assert_eq!(state.hud_message(), HandHudMessage::NextHandIn(3));

        // t=2.5s: NEXT HAND IN 2.
        state.tick(1.0, 0, 100, usize::MAX);
        assert_eq!(state.hud_message(), HandHudMessage::NextHandIn(2));

        // t=3.5s: NEXT HAND IN 1.
        state.tick(1.0, 0, 100, usize::MAX);
        assert_eq!(state.hud_message(), HandHudMessage::NextHandIn(1));

        // t=4.5s: ya debería haber spawneado y mostrar el banner.
        let spawned = state.tick(1.0, 0, 100, usize::MAX);
        assert!(spawned.is_some());
        assert_eq!(
            state.hud_message(),
            HandHudMessage::HandBanner(state.hand_number())
        );
    }

    #[test]
    fn banner_disappears_after_its_duration() {
        let mut state = HordeManager::new(4);

        run_full_reload(&mut state, 100);

        assert_eq!(
            state.hud_message(),
            HandHudMessage::HandBanner(state.hand_number())
        );

        state.tick(1.5, 5, 100, usize::MAX);

        assert_eq!(state.hud_message(), HandHudMessage::None);
    }

    #[test]
    fn no_message_while_actively_fighting_without_a_fresh_banner() {
        let state = HordeManager::new(4);

        assert_eq!(state.hud_message(), HandHudMessage::None);
    }

    // --- Bloque 1, Commit 07: Hand final reservada por nivel. ---

    #[test]
    fn reaching_the_configured_final_hand_number_reports_final_hand_reached_instead_of_a_spawn() {
        // Refleja Crimson Entrance/Black Club: HAND 1=4, 2=8, 3=16,
        // 4=Final reservada.
        let mut state = HordeManager::new(4);

        let hand_two = run_full_reload_with_final(&mut state, 100, 4);
        assert_eq!(hand_two, HandOutcome::Spawn { dealer_count: 8 });

        let hand_three = run_full_reload_with_final(&mut state, 100, 4);
        assert_eq!(hand_three, HandOutcome::Spawn { dealer_count: 16 });

        let hand_four = run_full_reload_with_final(&mut state, 100, 4);
        assert_eq!(hand_four, HandOutcome::FinalHandReached);
        assert_eq!(state.hand_number(), 4);
    }

    #[test]
    fn reaching_the_final_hand_shows_no_hand_banner() {
        // La ronda final es The King (barra de vida propia); el banner
        // "HAND N" se solaparía con ella, así que NO se muestra.
        let mut state = HordeManager::new(2);

        assert_eq!(
            run_full_reload_with_final(&mut state, 100, 2),
            HandOutcome::FinalHandReached
        );

        assert_eq!(state.hud_message(), HandHudMessage::None);

        // Y sigue en `None` en los cuadros siguientes (el King mantiene
        // la fase `Active` sin re-disparar el banner).
        for _ in 0..10 {
            assert_eq!(state.tick(0.1, 1, 100, 2), None);
            assert_eq!(state.hud_message(), HandHudMessage::None);
        }
    }

    /// Igual que `run_full_reload`, pero conservando el
    /// `HandOutcome` completo (nunca desenvuelto a `usize`) y con
    /// `final_hand_number` configurable, para poder ejercitar
    /// `HandOutcome::FinalHandReached` explícitamente.
    fn run_full_reload_with_final(
        state: &mut HordeManager,
        level_cap: usize,
        final_hand_number: usize,
    ) -> HandOutcome {
        assert_eq!(state.tick(0.016, 0, level_cap, final_hand_number), None);

        let mut elapsed = 0.0;

        while elapsed < 4.5 {
            if let Some(outcome) = state.tick(0.05, 0, level_cap, final_hand_number) {
                return outcome;
            }

            elapsed += 0.05;
        }

        panic!("la Hand debe spawnear (o reservarse) dentro de ~4s")
    }

    #[test]
    fn the_procedural_level_reserves_hand_two_as_final_with_no_doubling_at_all() {
        // The Dealer's True Maze: HAND 1=40..=50, HAND 2=Final
        // reservada — ni una sola Hand normal de por medio.
        let mut state = HordeManager::new(45);

        let hand_two = run_full_reload_with_final(&mut state, 100, 2);

        assert_eq!(hand_two, HandOutcome::FinalHandReached);
        assert_eq!(state.hand_number(), 2);
    }

    #[test]
    fn house_of_cards_gets_one_extra_normal_hand_before_the_final_hand() {
        // House of Cards: HAND 1=4, 2=8, 3=16, 4=32, 5=Final
        // reservada.
        let mut state = HordeManager::new(4);

        assert_eq!(
            run_full_reload_with_final(&mut state, 100, 5),
            HandOutcome::Spawn { dealer_count: 8 }
        );
        assert_eq!(
            run_full_reload_with_final(&mut state, 100, 5),
            HandOutcome::Spawn { dealer_count: 16 }
        );
        assert_eq!(
            run_full_reload_with_final(&mut state, 100, 5),
            HandOutcome::Spawn { dealer_count: 32 }
        );
        assert_eq!(
            run_full_reload_with_final(&mut state, 100, 5),
            HandOutcome::FinalHandReached
        );
    }

    // --- Bloque 1, Commit 07: elección de la HAND I. ---

    #[test]
    fn fixed_range_first_hand_dealer_count_always_returns_the_same_value() {
        for seed in [0, 1, 12345, u64::MAX] {
            assert_eq!(first_hand_dealer_count(seed, 4, 4), 4);
        }
    }

    #[test]
    fn ranged_first_hand_dealer_count_stays_within_bounds() {
        for seed in 0..50u64 {
            let count = first_hand_dealer_count(seed, 40, 50);

            assert!((40..=50).contains(&count), "seed {seed}: {count}");
        }
    }

    #[test]
    fn ranged_first_hand_dealer_count_is_deterministic_per_seed() {
        let a = first_hand_dealer_count(777, 40, 50);
        let b = first_hand_dealer_count(777, 40, 50);

        assert_eq!(a, b);
    }

    #[test]
    fn ranged_first_hand_dealer_count_can_differ_across_seeds() {
        let counts: HashSet<usize> = (0..50u64)
            .map(|seed| first_hand_dealer_count(seed, 40, 50))
            .collect();

        assert!(counts.len() > 1);
    }

    // --- Determinismo de spawn (sección 17) ---

    #[test]
    fn same_session_seed_and_hand_number_produce_the_same_spawn_layout() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::from([(1, 1)]);

        let seed_a = spawn_seed_for_hand(12345, 2);
        let seed_b = spawn_seed_for_hand(12345, 2);

        assert_eq!(seed_a, seed_b);

        let a = select_spawn_cells(
            &level,
            Vector2::new(72.0, 72.0),
            0.0,
            48,
            &occupied,
            4,
            false,
            seed_a,
        );

        let b = select_spawn_cells(
            &level,
            Vector2::new(72.0, 72.0),
            0.0,
            48,
            &occupied,
            4,
            false,
            seed_b,
        );

        assert_eq!(a, b);
    }

    #[test]
    fn different_hand_numbers_can_produce_different_layouts() {
        let seed_hand_2 = spawn_seed_for_hand(12345, 2);
        let seed_hand_3 = spawn_seed_for_hand(12345, 3);

        assert_ne!(seed_hand_2, seed_hand_3);
    }

    // --- Spawn válido / distancia segura ---

    /// Sala abierta y completamente conectada (sin muros internos):
    /// suficientemente grande para que existan celdas a distancia
    /// navegable >= 6 desde cualquier punto interior razonable.
    fn test_level() -> Level {
        let map = "\
###############
#p            #
#             #
#             #
#             #
#             #
#             #
#            g#
###############
";

        Level::from_cells(map.lines().map(|line| line.chars().collect()).collect())
            .expect("el mapa de prueba debe ser válido")
    }

    #[test]
    fn selected_cells_are_always_walkable_and_never_occupied() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::from([(4, 5), (4, 6), (4, 7)]);

        let player_position = Vector2::new(4.5 * 48.0, 4.5 * 48.0);

        let cells = select_spawn_cells(&level, player_position, 0.0, 48, &occupied, 10, false, 999);

        assert!(!cells.is_empty());

        for &(row, column) in &cells {
            assert!(level.is_walkable(row, column));
            assert!(!occupied.contains(&(row, column)));
        }
    }

    #[test]
    fn selected_cells_respect_the_safe_distance_when_the_map_allows_it() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_cell = (4, 6);

        let player_position = Vector2::new(
            player_cell.1 as f32 * 48.0 + 24.0,
            player_cell.0 as f32 * 48.0 + 24.0,
        );

        let distances = DistanceField::from_level(&level, player_cell);

        let cells = select_spawn_cells(&level, player_position, 0.0, 48, &occupied, 3, false, 1);

        assert!(!cells.is_empty());

        for &(row, column) in &cells {
            let distance = distances
                .distance_at(row, column)
                .expect("celda elegida debe ser alcanzable");

            assert!(distance >= SAFE_RESPAWN_DISTANCE_CELLS);
        }
    }

    #[test]
    fn never_returns_more_cells_than_requested() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_position = Vector2::new(4.5 * 48.0, 4.5 * 48.0);

        let cells = select_spawn_cells(&level, player_position, 0.0, 48, &occupied, 3, false, 7);

        assert!(cells.len() <= 3);
    }

    #[test]
    fn cluster_mode_never_exceeds_the_requested_count() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_position = Vector2::new(4.5 * 48.0, 4.5 * 48.0);

        let cells = select_spawn_cells(&level, player_position, 0.0, 48, &occupied, 12, true, 42);

        assert!(cells.len() <= 12);

        let unique: HashSet<(usize, usize)> = cells.iter().copied().collect();
        assert_eq!(unique.len(), cells.len());
    }

    // --- Munición ---

    #[test]
    fn no_extra_pickups_when_ammo_is_already_sufficient() {
        // 4 Dealers * 2 disparos * 1.5 margen = 12 disparos con
        // margen; 12 balas accesibles ya alcanzan justo, sin déficit.
        assert_eq!(extra_ammo_pickups_needed(4, 12), 0);
    }

    #[test]
    fn extra_pickups_scale_with_the_ammo_deficit() {
        // 8 Dealers * 2 * 1.5 = 24 disparos con margen; con 12 balas
        // accesibles el déficit es 12 -> ceil(12/6) = 2.
        assert_eq!(extra_ammo_pickups_needed(8, 12), 2);
    }

    #[test]
    fn extra_pickups_are_capped_even_for_enormous_deficits() {
        // La fórmula base nunca inunda el mapa: tope de 6.
        assert_eq!(extra_ammo_pickups_needed(1000, 0), 6);
    }

    #[test]
    fn diverse_ammo_splits_between_easy_and_hard_access() {
        let level = test_level(); // sala abierta 13x7 interior
        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_cell = (4, 6);
        let player_position = Vector2::new(
            player_cell.1 as f32 * 48.0 + 24.0,
            player_cell.0 as f32 * 48.0 + 24.0,
        );
        let facing = 0.0; // mirando hacia +x

        let distances = DistanceField::from_level(&level, player_cell);

        // 2 fáciles + 2 difíciles (el lote del tramo ESCASO).
        let cells =
            select_split_ammo_cells(&level, player_position, facing, 48, &occupied, 2, 2, 7);
        assert_eq!(cells.len(), 4);

        for &cell in &cells {
            assert_ne!(cell, player_cell);
            assert!(distances.distance_at(cell.0, cell.1).is_some());
        }

        // Grupo FÁCIL (primeros 2): fuera del cono de visión inmediato,
        // a pocos pasos.
        for &(row, column) in cells.iter().take(2) {
            assert!(
                !is_immediately_visible(row, column, player_position, facing, 48),
                "la munición de fácil acceso no debe aparecer delante del jugador"
            );
            let d = distances.distance_at(row, column).unwrap();
            assert!((1..=9).contains(&d), "fácil demasiado lejos: {d}");
        }

        // Grupo DIFÍCIL (últimos 2): lejos — hay que ir a por ellas.
        for &(row, column) in cells.iter().skip(2) {
            let d = distances.distance_at(row, column).unwrap();
            assert!(d >= 6, "difícil demasiado cerca: {d}");
        }

        // Determinista.
        assert_eq!(
            select_split_ammo_cells(&level, player_position, facing, 48, &occupied, 2, 2, 7),
            cells
        );
    }

    #[test]
    fn split_ammo_cells_are_spread_apart_not_clustered() {
        let level = test_level();
        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_cell = (4, 6);
        let player_position = Vector2::new(
            player_cell.1 as f32 * 48.0 + 24.0,
            player_cell.0 as f32 * 48.0 + 24.0,
        );

        let cells = select_split_ammo_cells(&level, player_position, 0.0, 48, &occupied, 2, 2, 3);
        assert_eq!(cells.len(), 4);

        for i in 0..cells.len() {
            for j in (i + 1)..cells.len() {
                assert!(
                    cell_chebyshev(cells[i], cells[j]) >= MIN_AMMO_SEPARATION_CELLS,
                    "pickups {:?} y {:?} demasiado juntos",
                    cells[i],
                    cells[j]
                );
            }
        }
    }

    #[test]
    fn split_ammo_cells_never_reuse_an_occupied_or_player_cell() {
        let level = test_level();
        let player_cell = (3, 3);
        let player_position = Vector2::new(
            player_cell.1 as f32 * 48.0 + 24.0,
            player_cell.0 as f32 * 48.0 + 24.0,
        );

        let mut occupied: HashSet<(usize, usize)> = HashSet::new();
        occupied.insert((3, 4));
        occupied.insert((4, 3));
        occupied.insert((5, 5));

        // El lote complementario: 1 fácil + 2 difíciles.
        let cells = select_split_ammo_cells(&level, player_position, 0.0, 48, &occupied, 1, 2, 1);
        assert_eq!(cells.len(), 3);
        for cell in &cells {
            assert!(!occupied.contains(cell));
            assert_ne!(*cell, player_cell);
        }

        let unique: HashSet<(usize, usize)> = cells.iter().copied().collect();
        assert_eq!(unique.len(), cells.len(), "sin duplicados");
    }

    // --- Emergency Ammo Respawn: selección de posiciones. ---

    #[test]
    fn emergency_ammo_cells_land_within_the_preferred_distance_band_when_the_map_allows_it() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_cell = (4, 6);

        let player_position = Vector2::new(
            player_cell.1 as f32 * 48.0 + 24.0,
            player_cell.0 as f32 * 48.0 + 24.0,
        );

        let distances = DistanceField::from_level(&level, player_cell);

        let cells = select_emergency_ammo_cells(&level, player_position, 48, &occupied, 2, 99);

        assert_eq!(cells.len(), 2);

        for &(row, column) in &cells {
            let distance = distances
                .distance_at(row, column)
                .expect("celda elegida debe ser alcanzable");

            // Banda estrecha y CERCANA (1..=3): la munición de
            // emergencia debe quedar al alcance inmediato del jugador.
            assert!(
                (1..=3).contains(&distance),
                "distancia fuera de banda: {distance}"
            );
        }
    }

    #[test]
    fn emergency_ammo_cells_are_never_the_players_own_cell() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_cell = (4, 6);

        let player_position = Vector2::new(
            player_cell.1 as f32 * 48.0 + 24.0,
            player_cell.0 as f32 * 48.0 + 24.0,
        );

        let cells = select_emergency_ammo_cells(&level, player_position, 48, &occupied, 2, 5);

        assert!(!cells.contains(&player_cell));
    }

    #[test]
    fn emergency_ammo_cells_never_land_on_occupied_positions() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::from([(4, 5), (4, 6), (4, 7), (3, 6)]);

        let player_position = Vector2::new(4.5 * 48.0, 4.5 * 48.0);

        let cells = select_emergency_ammo_cells(&level, player_position, 48, &occupied, 2, 12);

        for cell in &cells {
            assert!(!occupied.contains(cell));
        }
    }

    #[test]
    fn emergency_ammo_never_returns_more_cells_than_requested() {
        let level = test_level();

        let occupied: HashSet<(usize, usize)> = HashSet::new();

        let player_position = Vector2::new(4.5 * 48.0, 4.5 * 48.0);

        let cells = select_emergency_ammo_cells(&level, player_position, 48, &occupied, 2, 3);

        assert!(cells.len() <= 2);

        let unique: HashSet<(usize, usize)> = cells.iter().copied().collect();
        assert_eq!(unique.len(), cells.len());
    }

    #[test]
    fn emergency_ammo_seed_is_deterministic_per_spawn_index() {
        let a = spawn_seed_for_emergency_ammo(777, 0);
        let b = spawn_seed_for_emergency_ammo(777, 0);
        let c = spawn_seed_for_emergency_ammo(777, 1);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn emergency_ammo_seed_never_collides_with_dealer_spawn_seed() {
        // Discriminadores distintos (sección 23): la misma
        // `session_seed`/índice nunca debe producir accidentalmente
        // la misma semilla para dos sistemas distintos.
        let dealer_seed = spawn_seed_for_hand(777, 1);
        let emergency_seed = spawn_seed_for_emergency_ammo(777, 1);

        assert_ne!(dealer_seed, emergency_seed);
    }

    // --- Health Respawn por Hand: objetivo y semillas. ---

    #[test]
    fn health_pickup_target_is_always_between_three_and_five() {
        for hand_number in 2..30 {
            let target = health_pickup_target_for_hand(12345, hand_number);

            assert!(
                (3..=5).contains(&target),
                "hand {hand_number}: target {target} fuera de 3..=5"
            );
        }
    }

    #[test]
    fn health_pickup_target_is_deterministic_per_seed_and_hand() {
        let a = health_pickup_target_for_hand(555, 2);
        let b = health_pickup_target_for_hand(555, 2);

        assert_eq!(a, b);
    }

    #[test]
    fn health_pickup_target_can_differ_across_hands() {
        // No es una garantía matemática (podrían coincidir por azar),
        // pero al menos una Hand distinta en un rango amplio debe
        // producir un target distinto para esta semilla.
        let targets: HashSet<usize> = (2..20)
            .map(|hand_number| health_pickup_target_for_hand(555, hand_number))
            .collect();

        assert!(targets.len() > 1);
    }

    #[test]
    fn health_spawn_seed_never_collides_with_dealer_or_emergency_ammo_seed() {
        let dealer_seed = spawn_seed_for_hand(42, 2);
        let emergency_seed = spawn_seed_for_emergency_ammo(42, 2);
        let health_seed = spawn_seed_for_health_replenish(42, 2);

        assert_ne!(dealer_seed, health_seed);
        assert_ne!(emergency_seed, health_seed);
    }

    // --- Bloque 2, Commit 15: semilla de The Royal Flush. ---

    #[test]
    fn royal_flush_seed_is_deterministic_per_session_seed() {
        assert_eq!(
            spawn_seed_for_royal_flush(777),
            spawn_seed_for_royal_flush(777)
        );
        assert_ne!(
            spawn_seed_for_royal_flush(777),
            spawn_seed_for_royal_flush(778)
        );
    }

    #[test]
    fn royal_flush_seed_never_collides_with_the_other_resource_seeds() {
        let royal = spawn_seed_for_royal_flush(42);

        assert_ne!(royal, spawn_seed_for_hand(42, 1));
        assert_ne!(royal, spawn_seed_for_hand(42, 2));
        assert_ne!(royal, spawn_seed_for_emergency_ammo(42, 0));
        assert_ne!(royal, spawn_seed_for_health_replenish(42, 2));
    }

    // --- Bloque 3, Commit 24: semilla de The King. ---

    #[test]
    fn king_seed_is_deterministic_and_isolated_from_every_other_resource() {
        assert_eq!(spawn_seed_for_king(42), spawn_seed_for_king(42));
        assert_ne!(spawn_seed_for_king(42), spawn_seed_for_king(43));

        let king = spawn_seed_for_king(42);
        assert_ne!(king, spawn_seed_for_royal_flush(42));
        assert_ne!(king, spawn_seed_for_hand(42, 1));
        assert_ne!(king, spawn_seed_for_hand(42, 4));
        assert_ne!(king, spawn_seed_for_emergency_ammo(42, 0));
        assert_ne!(king, spawn_seed_for_health_replenish(42, 4));
    }
}
