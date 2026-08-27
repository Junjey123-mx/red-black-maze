use std::collections::HashSet;

use raylib::prelude::Vector2;

use crate::player::{Player, Weapon, WeaponState};
use crate::world::{
    AmmoPickup, DEALER_ATTACK_RANGE_CELLS, DistanceField, Entity, EntityDamageOutcome, EntityState,
    EntityStateTransition, HealthPickup, Level,
};

use super::GameMode;
use super::hand::{self, HandHudMessage, HandState};

/// Modos de visualización disponibles.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ViewMode {
    Map2D,
    World3D,
}

/// Daño aplicado a un Dealer por cada disparo aceptado que lo
/// impacta. Con `DEALER_MAX_HEALTH = 100` (definido en
/// `world::entity`), un Dealer muere tras exactamente dos golpes.
const DEALER_DAMAGE_PER_HIT: i32 = 50;

/// Munición de reserva que otorga cada `AmmoPickup` recogido
/// (Tarea 44). Nunca se aplica directamente al cargador — siempre
/// vía `Weapon::add_reserve_ammo`, que ya respeta el tope.
const AMMO_PICKUP_AMOUNT: u32 = 6;

/// Radio de recolección de un pickup (`AmmoPickup` o `HealthPickup`),
/// en píxeles de mundo.
///
/// ~40% del ancho de una celda (`BLOCK_SIZE = 48` en el proyecto:
/// `0.4 * 48 = 19.2`), deliberadamente pequeño para que el jugador
/// no pueda recoger munición/vida a través de una pared ni desde un
/// pasillo paralelo. Ambos tipos de pickup comparten el mismo
/// criterio espacial (Health Pickup, sección 14): un solo radio, sin
/// una segunda constante duplicada.
const PICKUP_RADIUS: f32 = 19.2;

/// Vida real que restaura cada `HealthPickup` recogido (Health
/// Pickup), aplicada siempre vía `Player::heal`, que ya respeta el
/// tope `PLAYER_MAX_HEALTH` — nunca se escribe `health` directamente
/// aquí.
const HEALTH_PICKUP_AMOUNT: i32 = 20;

/// Daño que un ataque de Dealer ACEPTADO inflige al jugador
/// (Tarea 45). Las condiciones de aceptación (estado `Alert`,
/// rango, cooldown) viven en `world::Entity::attempt_attack`; esta
/// capa solo decide CUÁNTO daño corresponde a un ataque aceptado.
const DEALER_ATTACK_DAMAGE: i32 = 10;

/// Duración del flash visual de daño al jugador (Tarea 45), en
/// segundos de tiempo de PARTIDA (nunca reloj absoluto): solo
/// avanza mientras `update_hit_flash` se llame, que a su vez solo
/// ocurre dentro de `update_playing` — congelado automáticamente
/// durante `GameState::Paused`, igual que el resto de temporizadores
/// de la sesión.
const PLAYER_HIT_FLASH_DURATION: f32 = 0.12;

/// Temporizador puro del flash visual de daño: tiempo restante hasta
/// que deje de mostrarse. No conoce `Framebuffer`/color/dibujo — eso
/// vive en la capa de rendering, que solo LEE `is_active()`.
struct HitFlashState {
    remaining: f32,
}

impl HitFlashState {
    fn new() -> Self {
        Self { remaining: 0.0 }
    }

    /// Reinicia el flash a su duración completa. Llamarlo mientras
    /// ya está activo REINICIA la duración (no la extiende ni la
    /// acumula) — un segundo golpe durante el flash simplemente lo
    /// mantiene visible por otros `PLAYER_HIT_FLASH_DURATION`
    /// segundos completos desde ese instante.
    fn trigger(&mut self) {
        self.remaining = PLAYER_HIT_FLASH_DURATION;
    }

    /// Avanza el temporizador según el tiempo de PARTIDA transcurrido.
    /// Un `delta_time` no finito o no positivo se ignora sin alterar
    /// el estado.
    fn update(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }

        self.remaining = (self.remaining - delta_time).max(0.0);
    }

    fn is_active(&self) -> bool {
        self.remaining > 0.0
    }
}

/// Duración aproximada de cada cuadro de la animación de antorcha.
const TORCH_FRAME_DURATION: f32 = 0.1;

/// Número total de cuadros de la animación de antorcha.
const TORCH_FRAME_COUNT: usize = 4;

/// Estado de la animación de antorcha: cuadro actual en
/// reproducción y tiempo acumulado hacia el siguiente cambio de
/// cuadro.
///
/// Esto es estado de PARTIDA, no un recurso de textura: pertenece
/// a `GameSession`, no a `TextureManager`.
struct TorchAnimationState {
    frame_index: usize,
    elapsed_seconds: f32,
}

impl TorchAnimationState {
    fn new() -> Self {
        Self {
            frame_index: 0,
            elapsed_seconds: 0.0,
        }
    }

    /// Avanza la animación según el tiempo transcurrido desde el
    /// cuadro anterior.
    ///
    /// Un `delta_time` no finito o no positivo se ignora sin
    /// alterar el estado. El tiempo excedente sobre una duración de
    /// cuadro se conserva (no se descarta) para no perder tiempo
    /// fraccional acumulado, y un `delta_time` suficientemente
    /// grande avanza tantos cuadros como corresponda.
    fn update(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }

        self.elapsed_seconds += delta_time;

        while self.elapsed_seconds >= TORCH_FRAME_DURATION {
            self.elapsed_seconds -= TORCH_FRAME_DURATION;

            self.frame_index = (self.frame_index + 1) % TORCH_FRAME_COUNT;
        }
    }
}

/// Estado en tiempo de ejecución de la partida activa.
pub(crate) struct GameSession {
    pub(crate) level: Level,
    pub(crate) player: Player,
    pub(crate) view_mode: ViewMode,
    torch_animation: TorchAnimationState,
    weapon: Weapon,
    entities: Vec<Entity>,
    ammo_pickups: Vec<AmmoPickup>,
    health_pickups: Vec<HealthPickup>,
    hit_flash: HitFlashState,

    /// Sistema de "Dealer Hands": HAND I/II/III..., cadáveres aparte
    /// (esos viven en `Entity`/`entities`, ver `update_entities`).
    /// Pertenece a la sesión — nunca a `AudioManager`/rendering — y
    /// se reconstruye enteramente en `GameSession::new`, así que
    /// Retry/cambio de nivel/New Game siempre arrancan en HAND I sin
    /// heredar nada (ver doc de `HandState`).
    hand_state: HandState,

    /// Semilla base para derivar, de forma determinista, la
    /// distribución de spawn de cada Hand adicional
    /// (`hand::spawn_seed_for_hand`) — sección 17. Para "The Dealer's
    /// True Maze" es la semilla real del nivel (reproducible);
    /// para los tres niveles estáticos es un valor fijo por sesión
    /// que `App` decide (no necesitan una semilla "de nivel" real,
    /// pero sí una semilla determinista para poder probarse).
    hand_seed: u64,

    /// Cuántas veces se activó realmente el Emergency Ammo Respawn en
    /// esta sesión (nunca cuántas veces se EVALUÓ la condición — solo
    /// se incrementa cuando de verdad se generan pickups). Alimenta
    /// `hand::spawn_seed_for_emergency_ammo` para que cada activación
    /// sucesiva reciba una semilla distinta y determinista, en vez de
    /// repetir siempre las mismas dos celdas.
    emergency_ammo_spawn_count: u32,

    /// Modo elegido para esta partida (Portal o Horde). Fuente de
    /// verdad ÚNICA leída por `App`: ninguna otra bandera booleana
    /// (`is_horde`, `portal_enabled`, ...) duplica esta información
    /// en ningún otro lugar de `GameSession`. Todavía no condiciona
    /// ningún sistema de juego — eso llega en un commit posterior;
    /// por ahora solo viaja con la sesión para que `App` pueda
    /// preservarlo correctamente en Retry/Next Level.
    mode: GameMode,
}

impl GameSession {
    /// Crea una sesión a partir de un nivel y un jugador
    /// ya construidos.
    ///
    /// Inicia mostrando el mapa 2D, con la animación de antorcha en
    /// su cuadro inicial, crea exactamente un Dealer por cada
    /// marcador `e` que el nivel haya descubierto (centrado en su
    /// celda de aparición), y (Tarea 44) exactamente un
    /// `AmmoPickup` ACTIVO por cada marcador `a` — el arma siempre
    /// arranca con su munición inicial de siempre
    /// (`Weapon::new`); T44 no introduce persistencia de munición
    /// entre sesiones.
    ///
    /// `hand_seed` siembra el sistema de Hands (sección 17): la
    /// cantidad de Dealers de HAND I es simplemente la que el nivel
    /// ya trae (`enemy_spawns().len()`), sin usar `hand_seed` en
    /// absoluto — solo las Hands II+ derivan su distribución de esta
    /// semilla, así que reconstruir la sesión (Retry, cambio de
    /// nivel) con la MISMA semilla y la MISMA HAND I siempre produce
    /// exactamente el mismo punto de partida.
    ///
    /// `mode` viaja con la sesión como su única fuente de verdad de
    /// Portal/Horde (`GameSession::mode`); todavía no condiciona
    /// ningún sistema de juego de este constructor.
    pub(crate) fn new(
        level: Level,
        player: Player,
        block_size: usize,
        hand_seed: u64,
        mode: GameMode,
    ) -> Self {
        let entities: Vec<Entity> = level
            .enemy_spawns()
            .iter()
            .map(|&(row, column)| Entity::dealer_at_cell(row, column, block_size))
            .collect();

        let ammo_pickups = level
            .ammo_spawns()
            .iter()
            .map(|&(row, column)| AmmoPickup::at_cell(row, column, block_size))
            .collect();

        let health_pickups = level
            .health_spawns()
            .iter()
            .map(|&(row, column)| HealthPickup::at_cell(row, column, block_size))
            .collect();

        let hand_state = HandState::new(entities.len());

        Self {
            level,
            player,
            view_mode: ViewMode::Map2D,
            torch_animation: TorchAnimationState::new(),
            weapon: Weapon::new(),
            entities,
            ammo_pickups,
            health_pickups,
            hand_state,
            hand_seed,
            emergency_ammo_spawn_count: 0,
            hit_flash: HitFlashState::new(),
            mode,
        }
    }

    /// Modo de juego (Portal u Horde) con el que se construyó esta
    /// sesión.
    pub(crate) fn mode(&self) -> GameMode {
        self.mode
    }

    /// Entidades activas de la sesión actual (los Dealers
    /// aparecidos a partir de los marcadores `e` del nivel).
    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Avanza el comportamiento de cada entidad (temporizador de
    /// `Hit`, reevaluación de proximidad `Idle`/`Alert`, y
    /// persecución mientras esté `Alert`) según la posición actual
    /// del jugador, y reporta ÚNICAMENTE las transiciones de estado
    /// que realmente ocurrieron (`Entity::update` ya distingue
    /// "cambio real" de "sin cambio").
    ///
    /// Ninguna entidad ataca: la persecución solo mueve la posición
    /// de los Dealers `Alert` hacia el jugador, respetando la
    /// geometría del laberinto vía `world::DistanceField` (BFS de 4
    /// direcciones sobre `Level`, la misma autoridad de
    /// transitabilidad que colisión/raycasting). El campo de
    /// distancias se calcula A LO SUMO una vez por cuadro,
    /// compartido entre todas las entidades `Alert` (nunca uno por
    /// Dealer), y se omite por completo si ninguna entidad está
    /// `Alert` este cuadro.
    ///
    /// El resultado reportado es dominio puro
    /// (`EntityStateTransition`), sin vocabulario de audio/
    /// presentación: quien interpreta el evento (`App`) decide qué
    /// hacer con él.
    ///
    /// Tarea "Dealer Hands": esta misma pasada también avanza el
    /// temporizador de cadáver de cada `Entity` `Dead`
    /// (`Entity::advance_corpse_timer`, no-op para vivas) y, al
    /// final, elimina DEFINITIVAMENTE de `self.entities` cualquier
    /// cadáver que ya cumplió `CORPSE_DESPAWN_SECONDS` — vía
    /// `Vec::retain`, que nunca invalida iteradores ni dejar índices
    /// obsoletos, incluso si varios cadáveres expiran en el mismo
    /// cuadro. Esto ocurre ANTES de que `App` construya los índices
    /// de blancos de hitscan del mismo cuadro (`update_entities` se
    /// llama antes que el bloque de disparo dentro de
    /// `update_playing`), así que esos índices siempre reflejan la
    /// colección ya depurada.
    ///
    /// Corrección "visual-only corpse": un `Entity` `Dead` participa
    /// ÚNICAMENTE en rendering + temporizador de cadáver + despawn.
    /// `Entity::update`/`attempt_attack` ya devolvían de inmediato
    /// para `Dead` (así que esto nunca fue un bug de CORRECCIÓN), pero
    /// esta pasada seguía gastando una consulta de
    /// `DistanceField::step_toward_origin` por CADA cadáver, CADA
    /// cuadro, para un resultado que `Entity::update` descartaba de
    /// inmediato — trabajo desperdiciado que escala con la cantidad de
    /// cadáveres acumulados (hasta ~50 simultáneos en "The Dealer's
    /// True Maze" entre Hands). Los cadáveres ahora se saltan
    /// explícitamente ANTES de calcular `pursuit_target`/llamar a
    /// `entity.update`, dejando el camino caliente de IA
    /// exclusivamente para Dealers vivos, sin cambiar ningún
    /// resultado observable (un cadáver nunca reportaba transición ni
    /// se movía).
    pub(crate) fn update_entities(
        &mut self,
        delta_time: f32,
        block_size: usize,
    ) -> Vec<EntityStateTransition> {
        let player_position = self.player.pos;

        let player_cell = world_to_cell(player_position, block_size);

        let any_alert = self
            .entities
            .iter()
            .any(|entity| entity.state() == EntityState::Alert);

        let distance_field = any_alert.then(|| DistanceField::from_level(&self.level, player_cell));

        let transitions = self
            .entities
            .iter_mut()
            .filter_map(|entity| {
                entity.advance_corpse_timer(delta_time);

                if entity.is_dead() {
                    return None;
                }

                /*
                 * Corrección "corner dead zone": `step_toward_origin`
                 * retorna `None` en cuanto la celda del Dealer YA ES
                 * la celda del jugador (distancia de ruta 0) — sin
                 * importar en qué punto EXACTO de esa celda se
                 * encuentre todavía el Dealer, que puede quedar a
                 * varios píxeles del centro (la última celda de la
                 * ruta se abandona en el instante en que se CRUZA su
                 * borde, no al llegar a su centro). Con el jugador
                 * cerca de una esquina de esa misma celda, esos pocos
                 * píxeles bastan para superar `DEALER_ATTACK_RANGE`
                 * (medido empíricamente: ~37.6px de distancia real
                 * contra 36.0px de rango, un hueco de ~1.6px) — el
                 * Dealer queda "congelado": ya no tiene celda
                 * siguiente hacia la que perseguir, pero tampoco está
                 * lo bastante cerca para atacar.
                 *
                 * Una vez el Dealer y el jugador comparten la MISMA
                 * celda transitable, esa celda es, por construcción,
                 * un rectángulo abierto sin paredes internas (`Tile`
                 * no tiene variantes de "media pared"): cualquier
                 * segmento recto entre dos puntos de esa celda nunca
                 * cruza una pared. Es entonces geométricamente seguro
                 * perseguir la posición EXACTA del jugador — pero
                 * SOLO mientras siga haciendo falta. El fallback se
                 * activa ÚNICAMENTE cuando las TRES condiciones se
                 * cumplen a la vez: sin siguiente paso de ruta, misma
                 * celda, Y todavía fuera de `DEALER_ATTACK_RANGE`. En
                 * cuanto la distancia real cae dentro del rango de
                 * ataque, el fallback deja de activarse (retorna
                 * `None`, el mismo comportamiento de "quedarse quieto
                 * y atacar" que ya tenía cualquier Dealer en rango
                 * antes de esta corrección) — así el Dealer cruza
                 * exactamente la zona muerta y nunca converge hacia
                 * la posición exacta del jugador ni invade el espacio
                 * de la cámara. Nunca se activa entre celdas
                 * distintas, así que sigue siendo geométricamente
                 * imposible atacar a través de una pared.
                 */
                let pursuit_target = distance_field.as_ref().and_then(|field| {
                    let entity_cell = world_to_cell(entity.position(), block_size);

                    field
                        .step_toward_origin(entity_cell)
                        .map(|(row, column)| cell_center(row, column, block_size))
                        .or_else(|| {
                            if entity_cell != player_cell {
                                return None;
                            }

                            let attack_range = block_size as f32 * DEALER_ATTACK_RANGE_CELLS;

                            let dx = entity.position().x - player_position.x;

                            let dy = entity.position().y - player_position.y;

                            (dx * dx + dy * dy > attack_range * attack_range)
                                .then_some(player_position)
                        })
                });

                entity.update(player_position, delta_time, block_size, pursuit_target)
            })
            .collect();

        self.entities.retain(|entity| !entity.should_despawn());

        transitions
    }

    /// Cantidad de Dealers VIVOS ahora mismo — nunca
    /// `entities.len()`/`is_empty()`, que seguirían contando
    /// cadáveres durante sus 15s de despawn (sección 6). Única fuente
    /// de verdad de "la Hand terminó".
    pub(crate) fn alive_dealer_count(&self) -> usize {
        self.entities
            .iter()
            .filter(|entity| !entity.is_dead())
            .count()
    }

    /// Número de Hand actualmente en curso (`1` = HAND I).
    pub(crate) fn hand_number(&self) -> usize {
        self.hand_state.hand_number()
    }

    /// Mensaje HUD del sistema de Hands para este cuadro, si hay
    /// alguno. Dominio puro — `rendering::hud` decide cómo dibujarlo.
    pub(crate) fn hand_hud_message(&self) -> HandHudMessage {
        self.hand_state.hud_message()
    }

    /// Avanza el sistema de Hands un cuadro: countdown de "The House
    /// is reloading", detección de Hand completada, y — en el
    /// instante exacto en que corresponde — spawnea la siguiente
    /// Hand (Dealers nuevos distribuidos por el mapa según
    /// `use_clusters`, más munición adicional si el presupuesto lo
    /// exige).
    ///
    /// Debe llamarse EXCLUSIVAMENTE desde el update jugable
    /// (`App::update_playing`) — mismo patrón que
    /// `process_dealer_attacks`/`collect_nearby_ammo_pickups` — para
    /// que `Paused`/`Victory`/`Defeat` lo congelen automáticamente
    /// (esos estados simplemente no vuelven a llamar
    /// `update_playing`) sin ningún caso especial nuevo: no hace
    /// falta comprobar el `GameState` aquí.
    ///
    /// `level_cap` y `use_clusters` los decide `App`/`LevelManager`
    /// (identidad del nivel activo) — `GameSession` no conoce
    /// `LevelTheme` ni si el nivel es procedural, solo ejecuta con los
    /// parámetros que se le dan.
    pub(crate) fn update_hand_state(
        &mut self,
        delta_time: f32,
        block_size: usize,
        level_cap: usize,
        use_clusters: bool,
    ) {
        let alive_count = self.alive_dealer_count();

        let Some(new_hand_count) = self.hand_state.tick(delta_time, alive_count, level_cap) else {
            return;
        };

        let mut occupied: HashSet<(usize, usize)> = HashSet::new();

        occupied.insert(world_to_cell(self.player.pos, block_size));
        occupied.insert(self.level.goal());

        for entity in &self.entities {
            occupied.insert(world_to_cell(entity.position(), block_size));
        }

        for pickup in &self.ammo_pickups {
            if pickup.is_active() {
                occupied.insert(world_to_cell(pickup.position(), block_size));
            }
        }

        for pickup in &self.health_pickups {
            if pickup.is_active() {
                occupied.insert(world_to_cell(pickup.position(), block_size));
            }
        }

        let spawn_seed = hand::spawn_seed_for_hand(self.hand_seed, self.hand_state.hand_number());

        let spawn_cells = hand::select_spawn_cells(
            &self.level,
            self.player.pos,
            self.player.a,
            block_size,
            &occupied,
            new_hand_count,
            use_clusters,
            spawn_seed,
        );

        for &(row, column) in &spawn_cells {
            self.entities
                .push(Entity::dealer_at_cell(row, column, block_size));

            occupied.insert((row, column));
        }

        eprintln!(
            "Dealer Hands — HAND {} begins: {} Dealers requested, {} placed.",
            self.hand_number(),
            new_hand_count,
            spawn_cells.len()
        );

        let accessible_ammo = self.weapon.ammo()
            + self.weapon.reserve_ammo()
            + self
                .ammo_pickups
                .iter()
                .filter(|pickup| pickup.is_active())
                .count() as u32
                * AMMO_PICKUP_AMOUNT;

        let extra_pickups_needed =
            hand::extra_ammo_pickups_needed(spawn_cells.len(), accessible_ammo);

        if extra_pickups_needed > 0 {
            let pickup_seed = spawn_seed.wrapping_add(1);

            let pickup_cells = hand::select_spawn_cells(
                &self.level,
                self.player.pos,
                self.player.a,
                block_size,
                &occupied,
                extra_pickups_needed,
                false,
                pickup_seed,
            );

            for (row, column) in pickup_cells {
                occupied.insert((row, column));

                self.ammo_pickups
                    .push(AmmoPickup::at_cell(row, column, block_size));
            }
        }

        /*
         * Health Respawn por Hand (sección 13): HAND I nunca llega
         * aquí (esta rama solo se ejecuta cuando `HandState::tick`
         * reporta que una Hand NUEVA acaba de comenzar, y
         * `HandState::new` arranca directamente en HAND I sin pasar
         * por `tick`), así que la configuración inicial del nivel
         * queda intacta por construcción — sin necesitar comprobar
         * `hand_number() > 1` explícitamente.
         *
         * `health_pickup_target_for_hand` decide un objetivo
         * determinista en 3..=5; solo se generan los que faltan para
         * alcanzarlo (`saturating_sub`), nunca se eliminan corazones
         * existentes ni se añaden más allá del objetivo (sección 16:
         * "evitar acumulación infinita").
         */
        let health_target = hand::health_pickup_target_for_hand(self.hand_seed, self.hand_number());

        let active_health_pickups = self
            .health_pickups
            .iter()
            .filter(|pickup| pickup.is_active())
            .count();

        let health_to_spawn = health_target.saturating_sub(active_health_pickups);

        if health_to_spawn > 0 {
            let health_seed =
                hand::spawn_seed_for_health_replenish(self.hand_seed, self.hand_number());

            let health_cells = hand::select_spawn_cells(
                &self.level,
                self.player.pos,
                self.player.a,
                block_size,
                &occupied,
                health_to_spawn,
                false,
                health_seed,
            );

            for (row, column) in health_cells {
                self.health_pickups
                    .push(HealthPickup::at_cell(row, column, block_size));
            }
        }
    }

    /// Detecta y resuelve un softlock de munición (Emergency Ammo
    /// Respawn): si NO quedan balas alcanzables (cargador + reserva),
    /// NO hay ningún `AmmoPickup` activo en el mapa, Y todavía quedan
    /// Dealers vivos, crea `hand::EMERGENCY_AMMO_PICKUP_COUNT`
    /// pickups nuevos en celdas cercanas y alcanzables desde el
    /// jugador.
    ///
    /// Las tres comprobaciones baratas (Dealers vivos, munición
    /// total, pickups activos) se evalúan ANTES de tocar
    /// `DistanceField`/selección de posiciones (sección 35): el BFS
    /// solo se ejecuta en el cuadro exacto en que el softlock
    /// realmente amenaza, nunca en cada cuadro normal de juego.
    ///
    /// Debe llamarse EXCLUSIVAMENTE desde el update jugable
    /// (`App::update_playing`) — mismo patrón que
    /// `collect_nearby_ammo_pickups`/`collect_nearby_health_pickups`
    /// — para que Pause/Victory/Defeat lo congelen automáticamente
    /// sin ningún caso especial nuevo.
    ///
    /// Una vez creados los pickups de emergencia, `active_ammo_pickups
    /// > 0` deja de cumplir la condición: no hace falta ninguna
    /// bandera "ya generado" aparte, la propia colección de pickups
    /// es la fuente de verdad (sección 12). Nunca reproduce
    /// `SoundEffect::AmmoPickup` — ese sonido pertenece exclusivamente
    /// a la RECOLECCIÓN (`collect_nearby_ammo_pickups`), nunca a la
    /// aparición.
    pub(crate) fn ensure_emergency_ammo(&mut self, block_size: usize) -> usize {
        if self.alive_dealer_count() == 0 {
            return 0;
        }

        let total_ammo = self.weapon.ammo() + self.weapon.reserve_ammo();

        if total_ammo > 0 {
            return 0;
        }

        if self.ammo_pickups.iter().any(|pickup| pickup.is_active()) {
            return 0;
        }

        let mut occupied: HashSet<(usize, usize)> = HashSet::new();

        occupied.insert(world_to_cell(self.player.pos, block_size));
        occupied.insert(self.level.goal());

        for entity in &self.entities {
            occupied.insert(world_to_cell(entity.position(), block_size));
        }

        for pickup in &self.ammo_pickups {
            if pickup.is_active() {
                occupied.insert(world_to_cell(pickup.position(), block_size));
            }
        }

        for pickup in &self.health_pickups {
            if pickup.is_active() {
                occupied.insert(world_to_cell(pickup.position(), block_size));
            }
        }

        let seed = hand::spawn_seed_for_emergency_ammo(
            self.hand_seed,
            self.emergency_ammo_spawn_count as u64,
        );

        self.emergency_ammo_spawn_count += 1;

        let cells = hand::select_emergency_ammo_cells(
            &self.level,
            self.player.pos,
            block_size,
            &occupied,
            hand::EMERGENCY_AMMO_PICKUP_COUNT,
            seed,
        );

        for &(row, column) in &cells {
            self.ammo_pickups
                .push(AmmoPickup::at_cell(row, column, block_size));
        }

        cells.len()
    }

    /// Aplica el daño de un golpe de Dealer aceptado a la entidad
    /// indicada, con verificación segura de límites, y reporta el
    /// resultado semántico (`EntityDamageOutcome`) para que quien
    /// interpreta el evento (`App`) pueda distinguir un golpe real de
    /// un evento sin efecto sin inferirlo de `EntityState`.
    ///
    /// Un `entity_index` fuera de rango produce `EntityDamageOutcome::None`
    /// sin entrar en pánico. La cantidad de daño y la invariante de
    /// salud/estado son responsabilidad exclusiva de
    /// `Entity::apply_damage`; este método solo coordina el acceso
    /// indexado seguro.
    pub(crate) fn damage_entity(&mut self, entity_index: usize) -> EntityDamageOutcome {
        match self.entities.get_mut(entity_index) {
            Some(entity) => entity.apply_damage(DEALER_DAMAGE_PER_HIT),

            None => EntityDamageOutcome::None,
        }
    }

    /// Resuelve los ataques de TODOS los Dealers para este cuadro
    /// (Tarea 45) y retorna el daño TOTAL realmente aplicado al
    /// jugador (`0` si ninguno atacó, o si el jugador ya estaba en
    /// `0` de vida).
    ///
    /// Debe llamarse EXCLUSIVAMENTE desde el update jugable
    /// (`App::update_playing`) — nunca desde rendering, HUD, ni
    /// `update_paused` — para que la pausa (Tarea 42) congele
    /// automáticamente cooldowns/daño/flash sin ningún caso especial
    /// nuevo, exactamente como ya ocurre con la recolección de
    /// munición (Tarea 44).
    ///
    /// Cada Dealer decide POR SU CUENTA si su ataque se acepta
    /// (`Entity::attempt_attack`: estado, rango, cooldown
    /// individual); esta capa solo coordina MÚLTIPLES Dealers y
    /// aplica el daño correspondiente vía `Player::apply_damage`
    /// (la única autoridad sobre `health`). Dos Dealers listos en el
    /// mismo cuadro SÍ pueden sumar su daño (nunca se limita
    /// artificialmente a un único atacante por cuadro), pero el
    /// flash visual se dispara COMO MUCHO una vez por esta llamada
    /// (`HitFlashState::trigger` simplemente reinicia su duración,
    /// sin acumular), y `App` decide reproducir `SoundEffect::PlayerHit`
    /// como mucho una vez leyendo el total > 0 retornado, no una vez
    /// por Dealer.
    ///
    /// Corrección "visual-only corpse": un cadáver (`Entity::is_dead`)
    /// se salta explícitamente ANTES de llamar a `attempt_attack` —
    /// `Entity::apply_damage`/`attempt_attack` ya rechazaban cualquier
    /// intento sobre una entidad `Dead` (nunca fue posible que un
    /// cadáver dañara al jugador), pero esta pasada seguía
    /// decrementando el cooldown ofensivo de CADA cadáver acumulado
    /// en cada cuadro para un resultado que siempre era `false` —
    /// trabajo desperdiciado que escala con la cantidad de cadáveres
    /// vivos en la colección. Ningún Dealer VIVO cambia su
    /// comportamiento por esto: el orden de iteración y el resultado
    /// para entidades vivas son idénticos a antes.
    pub(crate) fn process_dealer_attacks(&mut self, delta_time: f32, block_size: usize) -> i32 {
        let player_position = self.player.pos;

        let mut total_damage = 0;

        for entity in &mut self.entities {
            if entity.is_dead() {
                continue;
            }

            if entity.attempt_attack(player_position, delta_time, block_size) {
                total_damage += self.player.apply_damage(DEALER_ATTACK_DAMAGE);
            }
        }

        if total_damage > 0 {
            self.hit_flash.trigger();
        }

        total_damage
    }

    /// Avanza el temporizador del flash visual de daño según el
    /// tiempo de PARTIDA transcurrido. Ver `HitFlashState::update`.
    pub(crate) fn update_hit_flash(&mut self, delta_time: f32) {
        self.hit_flash.update(delta_time);
    }

    /// `true` mientras el flash visual de daño debe mostrarse.
    pub(crate) fn is_hit_flash_active(&self) -> bool {
        self.hit_flash.is_active()
    }

    /// Pickups de munición de la sesión actual (activos Y ya
    /// recogidos): rendering decide por sí mismo, vía
    /// `AmmoPickup::is_active`, cuáles dibujar.
    pub(crate) fn ammo_pickups(&self) -> &[AmmoPickup] {
        &self.ammo_pickups
    }

    /// Recoge cualquier `AmmoPickup` activo dentro de
    /// `PICKUP_RADIUS` de la posición actual del jugador.
    ///
    /// Debe llamarse EXCLUSIVAMENTE desde el update jugable
    /// (`App::update_playing`) — nunca desde rendering, HUD, ni el
    /// parser — para que `App::update_paused` (Tarea 42), que
    /// simplemente no invoca `update_playing`, congele la
    /// recolección automáticamente sin necesitar ningún caso
    /// especial nuevo.
    ///
    /// Un pickup se consume (`AmmoPickup::deactivate`) únicamente si
    /// `Weapon::add_reserve_ammo` reporta que realmente añadió al
    /// menos una bala; con la reserva ya en el tope, el pickup
    /// permanece disponible para no desperdiciarlo. El cargador
    /// nunca se toca aquí — solo `Weapon::add_reserve_ammo`, la
    /// única autoridad sobre la reserva.
    ///
    /// Retorna cuántos pickups se consumieron REALMENTE este cuadro
    /// (`0` la inmensa mayoría de las veces): es el único evento
    /// semántico de "recolección exitosa" — `App` lo usa para
    /// solicitar `SoundEffect::AmmoPickup` exactamente una vez POR
    /// PICKUP consumido, nunca por simple proximidad a uno todavía
    /// activo. Funciona idéntico sin importar si el pickup vino del
    /// nivel, de una Hand nueva, o de la generación procedural: los
    /// tres viven en el mismo `self.ammo_pickups` y pasan por esta
    /// misma comprobación.
    pub(crate) fn collect_nearby_ammo_pickups(&mut self) -> u32 {
        let player_position = self.player.pos;

        let mut collected = 0;

        for pickup in &mut self.ammo_pickups {
            if !pickup.is_active() {
                continue;
            }

            if !pickup_in_range(player_position, pickup.position(), PICKUP_RADIUS) {
                continue;
            }

            if self.weapon.add_reserve_ammo(AMMO_PICKUP_AMOUNT) > 0 {
                pickup.deactivate();

                collected += 1;
            }
        }

        collected
    }

    /// Pickups de vida de la sesión actual (activos Y ya recogidos):
    /// rendering decide por sí mismo, vía `HealthPickup::is_active`,
    /// cuáles dibujar.
    pub(crate) fn health_pickups(&self) -> &[HealthPickup] {
        &self.health_pickups
    }

    /// Recoge cualquier `HealthPickup` activo dentro de
    /// `PICKUP_RADIUS` de la posición actual del jugador (Health
    /// Pickup).
    ///
    /// Debe llamarse EXCLUSIVAMENTE desde el update jugable
    /// (`App::update_playing`) — mismo motivo/patrón exacto que
    /// `collect_nearby_ammo_pickups` — para que `App::update_paused`
    /// congele la curación automáticamente sin ningún caso especial
    /// nuevo.
    ///
    /// Un pickup se consume (`HealthPickup::deactivate`) únicamente
    /// si `Player::heal` reporta que realmente restauró al menos un
    /// punto de vida; con la vida ya en `PLAYER_MAX_HEALTH`, el
    /// pickup permanece disponible para curar más adelante si el
    /// jugador vuelve a recibir daño (sección 2: "el corazón debe
    /// permanecer en el nivel"). La vida nunca se toca aquí — solo
    /// `Player::heal`, la única autoridad sobre `health`.
    ///
    /// Retorna cuántos pickups se consumieron REALMENTE este cuadro:
    /// el único evento semántico de "curación exitosa" — `App` lo usa
    /// para solicitar `SoundEffect::HealthPickup` exactamente una vez
    /// POR PICKUP consumido, nunca por simple proximidad a uno
    /// todavía activo ni cuando la vida ya estaba completa.
    pub(crate) fn collect_nearby_health_pickups(&mut self) -> u32 {
        let player_position = self.player.pos;

        let mut collected = 0;

        for pickup in &mut self.health_pickups {
            if !pickup.is_active() {
                continue;
            }

            if !pickup_in_range(player_position, pickup.position(), PICKUP_RADIUS) {
                continue;
            }

            if self.player.heal(HEALTH_PICKUP_AMOUNT) > 0 {
                pickup.deactivate();

                collected += 1;
            }
        }

        collected
    }

    /// Avanza la animación de antorcha según el tiempo transcurrido
    /// desde la última actualización.
    pub(crate) fn update_torch_animation(&mut self, delta_time: f32) {
        self.torch_animation.update(delta_time);
    }

    /// Cuadro de animación de antorcha actualmente activo.
    pub(crate) fn torch_frame_index(&self) -> usize {
        self.torch_animation.frame_index
    }

    /// Avanza la máquina de estados visual del arma según el tiempo
    /// transcurrido desde la última actualización.
    pub(crate) fn update_weapon(&mut self, delta_time: f32) {
        self.weapon.update(delta_time);
    }

    /// Estado visual actualmente activo del arma.
    pub(crate) fn weapon_state(&self) -> WeaponState {
        self.weapon.state()
    }

    /// Progreso normalizado de la recarga en curso, o `None` si el
    /// arma no está recargando. Ver `Weapon::reload_progress`; solo
    /// reenvía la lectura, no posee ningún temporizador propio.
    pub(crate) fn weapon_reload_progress(&self) -> Option<f32> {
        self.weapon.reload_progress()
    }

    /// Intenta aceptar un evento de disparo, iniciando el ciclo
    /// visual del arma.
    ///
    /// Retorna `true` si el disparo fue aceptado (útil en tareas
    /// futuras para disparar el hitscan), `false` si el arma está
    /// en enfriamiento o no está `Idle`.
    pub(crate) fn try_fire_weapon(&mut self) -> bool {
        self.weapon.try_fire()
    }

    /// Intenta iniciar una recarga del arma (tecla R).
    ///
    /// Retorna `true` si la recarga fue aceptada (cargador no lleno,
    /// reserva disponible, arma en `Idle`), `false` en cualquier
    /// otro caso. La transferencia real de munición ocurre más
    /// tarde, dentro de `update_weapon`, al completarse el
    /// temporizador — nunca aquí.
    pub(crate) fn try_start_weapon_reload(&mut self) -> bool {
        self.weapon.try_start_reload()
    }

    /// Vida actual del jugador, para presentación (HUD) u otro
    /// consumidor de solo lectura.
    pub(crate) fn player_health(&self) -> i32 {
        self.player.health()
    }

    /// Munición actual del arma, para presentación (HUD) u otro
    /// consumidor de solo lectura.
    pub(crate) fn weapon_ammo(&self) -> u32 {
        self.weapon.ammo()
    }

    /// Munición de reserva del arma (fuera del cargador), para
    /// presentación (HUD) u otro consumidor de solo lectura.
    pub(crate) fn weapon_reserve_ammo(&self) -> u32 {
        self.weapon.reserve_ammo()
    }

    /// Indica si el jugador se encuentra actualmente dentro de la
    /// celda de meta (`Level::goal`).
    ///
    /// Consulta pura de solo lectura: no modifica `Player`, `Level`
    /// ni ningún otro estado, no carga niveles y no decide la
    /// transición de estado de la aplicación (eso es
    /// responsabilidad de `App`). Es la única fuente de verdad para
    /// "¿se completó el nivel?"; no existe un booleano
    /// `victory`/`completed` duplicado en ningún otro lugar.
    pub(crate) fn has_reached_goal(&self, block_size: usize) -> bool {
        let (goal_row, goal_column) = self.level.goal();

        point_reaches_goal(
            self.player.pos.x,
            self.player.pos.y,
            goal_row,
            goal_column,
            block_size,
        )
    }
}

/// Convierte una posición de mundo (píxeles) a su celda de
/// cuadrícula `(fila, columna)`, con el mismo convenio
/// `floor(coordenada / block_size)` que usan `raycasting::caster` y
/// `world::collision`. `block_size == 0` o coordenadas no
/// finitas/negativas se resuelven de forma segura a `(0, 0)` en vez
/// de entrar en pánico: `DistanceField::from_level` ya trata
/// cualquier origen fuera de rango o no transitable como
/// "inalcanzable", así que un valor degenerado aquí nunca produce
/// persecución incorrecta, solo la desactiva con seguridad.
fn world_to_cell(position: Vector2, block_size: usize) -> (usize, usize) {
    if block_size == 0 || !position.x.is_finite() || !position.y.is_finite() {
        return (0, 0);
    }

    let column = (position.x / block_size as f32).floor().max(0.0) as usize;

    let row = (position.y / block_size as f32).floor().max(0.0) as usize;

    (row, column)
}

/// Centro, en píxeles de mundo, de la celda `(row, column)`. Misma
/// convención de centrado que `Player::from_level`/
/// `Entity::dealer_at_cell`/`rendering::sprites::cell_center`.
fn cell_center(row: usize, column: usize, block_size: usize) -> Vector2 {
    let half_block = block_size as f32 / 2.0;

    Vector2::new(
        column as f32 * block_size as f32 + half_block,
        row as f32 * block_size as f32 + half_block,
    )
}

/// Comprueba si `pickup_position` está a `radius` píxeles de mundo o
/// menos de `player_position` (distancia 2D en el plano del mapa; la
/// altura del billboard sobre el suelo no participa en la
/// colección).
///
/// Función pura, extraída de `collect_nearby_ammo_pickups` para
/// poder probar directamente el radio sin construir una
/// `GameSession`/`Level` completa. Compara distancia AL CUADRADO
/// (`dx² + dy² <= radius²`) para evitar `sqrt`, tal como sugiere la
/// tarea — la claridad de la fórmula pesa más que la
/// microoptimización, pero evitar la raíz cuadrada es gratis aquí.
///
/// Compartida por `collect_nearby_ammo_pickups` y
/// `collect_nearby_health_pickups` (Health Pickup, sección 14): ambos
/// pickups usan EXACTAMENTE el mismo criterio espacial, así que no
/// existen dos copias de esta comprobación.
fn pickup_in_range(player_position: Vector2, pickup_position: Vector2, radius: f32) -> bool {
    let dx = player_position.x - pickup_position.x;

    let dy = player_position.y - pickup_position.y;

    dx * dx + dy * dy <= radius * radius
}

/// Comprueba si el punto de mundo `(player_x, player_y)` cae dentro
/// de la celda de meta `(goal_row, goal_column)`, usando el mismo
/// convenio fila/columna que el resto del proyecto
/// (`column * block_size <= x < (column + 1) * block_size`, y
/// análogamente para `y`/fila).
///
/// Función pura y libre de E/S, extraída de `has_reached_goal` para
/// poder probar directamente todos los casos límite sin construir
/// un `Level`/`Player`/`GameSession` completo.
///
/// Retorna `false` de forma segura (sin pánico ni división por
/// cero) para `block_size == 0`, coordenadas no finitas, o
/// coordenadas negativas.
fn point_reaches_goal(
    player_x: f32,
    player_y: f32,
    goal_row: usize,
    goal_column: usize,
    block_size: usize,
) -> bool {
    if block_size == 0 {
        return false;
    }

    if !player_x.is_finite() || !player_y.is_finite() {
        return false;
    }

    if player_x < 0.0 || player_y < 0.0 {
        return false;
    }

    let column = (player_x / block_size as f32).floor() as usize;

    let row = (player_y / block_size as f32).floor() as usize;

    row == goal_row && column == goal_column
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const BLOCK_SIZE: usize = 48;

    /// Mismo valor que `world::entity::CORPSE_DESPAWN_SECONDS`
    /// (`pub(crate)` solo dentro de `world`, no reexportado aquí):
    /// mismo patrón ya establecido en este módulo de pruebas para
    /// otras constantes privadas de otro módulo (por ejemplo el
    /// cooldown de ataque de 0.9s, hardcodeado como literal en los
    /// tests de ataque de más abajo en vez de importado).
    const CORPSE_DESPAWN_SECONDS: f32 = 15.0;

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Guardia RAII mínima para un archivo de nivel temporal, mismo
    /// patrón std-only ya establecido en `world::pathfinding`/las
    /// pruebas de integración: nombre único vía PID + contador,
    /// limpieza automática al salir de alcance.
    struct TempLevelFile {
        path: PathBuf,
    }

    impl TempLevelFile {
        fn write(contents: &str) -> Self {
            let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

            let file_name = format!(
                "red_black_maze_session_test_{}_{counter}.txt",
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

    fn new_test_session() -> GameSession {
        let map = "\
#######
#p   g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    /// Sesión de prueba con un único Dealer en (fila 2, columna 3).
    /// `Entity::dealer_at_cell` los aparece SIEMPRE a 48px o más de
    /// distancia entre celdas de mapa (el centro de una celda nunca
    /// queda a menos de 48px del centro de otra), así que para
    /// probar ataques (rango 36px) los tests colocan manualmente
    /// `session.player.pos` cerca del Dealer, tal como ya hacen los
    /// tests de pickups de munición con `session.player.pos`.
    fn new_test_session_with_one_dealer() -> GameSession {
        let map = "\
#######
#p    #
#  e  #
#    g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    /// Sesión de prueba con dos Dealers, en (fila 2, columna 3) y
    /// (fila 2, columna 5).
    /// Dos Dealers en celdas ADYACENTES (fila 2, columnas 3 y 4;
    /// 48px de centro a centro), deliberadamente cerca entre sí para
    /// que una única posición de jugador (el punto medio) quepa
    /// dentro del rango de ataque (36px) de AMBOS a la vez —
    /// geométricamente imposible si estuvieran a 96px o más.
    fn new_test_session_with_two_dealers() -> GameSession {
        let map = "\
#########
#p      #
#  ee   #
#      g#
#########
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    /// Coloca al jugador a `offset` píxeles del Dealer en
    /// `entity_index` (mismo eje X) y hace que ese Dealer entre en
    /// `Alert` llamando a `update_entities` una vez.
    fn move_player_near_dealer_and_alert(
        session: &mut GameSession,
        entity_index: usize,
        offset: f32,
    ) {
        let dealer_position = session.entities()[entity_index].position();

        session.player.pos = Vector2::new(dealer_position.x + offset, dealer_position.y);

        session.update_entities(0.016, BLOCK_SIZE);

        assert_eq!(session.entities()[entity_index].state(), EntityState::Alert);
    }

    // --- Tarea 45: ataques de Dealer / daño al jugador. ---

    #[test]
    fn single_ready_dealer_in_range_deals_damage_and_triggers_flash() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        assert_eq!(session.player_health(), 100);
        assert!(!session.is_hit_flash_active());

        let damage = session.process_dealer_attacks(0.016, BLOCK_SIZE);

        assert_eq!(damage, 10);
        assert_eq!(session.player_health(), 90);
        assert!(session.is_hit_flash_active());
    }

    #[test]
    fn out_of_range_dealer_deals_no_damage() {
        let mut session = new_test_session_with_one_dealer();

        // Fuera del rango de ataque (36px) pero dentro de la
        // distancia de alerta (192px), para que el Dealer entre en
        // Alert sin poder golpear.
        move_player_near_dealer_and_alert(&mut session, 0, 100.0);

        let damage = session.process_dealer_attacks(0.016, BLOCK_SIZE);

        assert_eq!(damage, 0);
        assert_eq!(session.player_health(), 100);
        assert!(!session.is_hit_flash_active());
    }

    #[test]
    fn cooldown_prevents_damage_every_frame() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        assert_eq!(session.process_dealer_attacks(0.016, BLOCK_SIZE), 10);
        assert_eq!(session.player_health(), 90);

        // Cuadros inmediatamente siguientes: sin daño adicional
        // mientras el cooldown (0.9s) siga activo.
        assert_eq!(session.process_dealer_attacks(0.016, BLOCK_SIZE), 0);
        assert_eq!(session.process_dealer_attacks(0.1, BLOCK_SIZE), 0);
        assert_eq!(session.player_health(), 90);
    }

    #[test]
    fn two_ready_dealers_deal_independent_damage_in_the_same_frame() {
        let mut session = new_test_session_with_two_dealers();

        // Punto medio entre ambos Dealers (48px de centro a centro):
        // 24px de cada uno, dentro del rango de ataque (36px) de
        // los DOS simultáneamente.
        let dealer0 = session.entities()[0].position();
        let dealer1 = session.entities()[1].position();

        let midpoint = Vector2::new((dealer0.x + dealer1.x) / 2.0, dealer0.y);

        session.player.pos = midpoint;

        session.update_entities(0.016, BLOCK_SIZE);

        assert_eq!(session.entities()[0].state(), EntityState::Alert);
        assert_eq!(session.entities()[1].state(), EntityState::Alert);

        let damage = session.process_dealer_attacks(0.016, BLOCK_SIZE);

        assert_eq!(damage, 20);
        assert_eq!(session.player_health(), 80);
    }

    #[test]
    fn hit_dealer_cannot_damage_the_player() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        session.damage_entity(0);
        assert_eq!(session.entities()[0].state(), EntityState::Hit);

        let damage = session.process_dealer_attacks(0.016, BLOCK_SIZE);

        assert_eq!(damage, 0);
        assert_eq!(session.player_health(), 100);
    }

    #[test]
    fn dead_dealer_never_damages_the_player_again() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        // Vida real del Dealer (100) requiere dos golpes de 50 para
        // morir (`DEALER_DAMAGE_PER_HIT`).
        session.damage_entity(0);
        session.damage_entity(0);
        assert_eq!(session.entities()[0].state(), EntityState::Dead);

        for _ in 0..20 {
            assert_eq!(session.process_dealer_attacks(0.5, BLOCK_SIZE), 0);
        }

        assert_eq!(session.player_health(), 100);
    }

    #[test]
    fn no_attack_this_frame_produces_no_feedback() {
        let mut session = new_test_session();

        assert_eq!(session.process_dealer_attacks(0.016, BLOCK_SIZE), 0);
        assert!(!session.is_hit_flash_active());
    }

    #[test]
    fn health_already_zero_produces_no_further_feedback() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        // Agota la vida del jugador a 0 con daño directo (evita
        // depender del timing de cooldown para llegar a 0 rápido en
        // el test).
        session.player.apply_damage(1000);
        assert_eq!(session.player_health(), 0);

        let damage = session.process_dealer_attacks(0.016, BLOCK_SIZE);

        assert_eq!(damage, 0);
        assert!(!session.is_hit_flash_active());
    }

    #[test]
    fn hit_flash_timer_progresses_and_expires() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        session.process_dealer_attacks(0.016, BLOCK_SIZE);
        assert!(session.is_hit_flash_active());

        session.update_hit_flash(0.10);
        assert!(session.is_hit_flash_active());

        session.update_hit_flash(0.03);
        assert!(!session.is_hit_flash_active());
    }

    #[test]
    fn new_damage_while_flash_active_restarts_its_duration() {
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        session.process_dealer_attacks(0.016, BLOCK_SIZE);

        session.update_hit_flash(0.10);
        assert!(session.is_hit_flash_active());

        // Deja pasar el cooldown completo (0.9s) para que el mismo
        // Dealer pueda volver a golpear y reiniciar el flash.
        session.process_dealer_attacks(0.9, BLOCK_SIZE);
        assert!(session.is_hit_flash_active());

        // Si el flash simplemente se hubiera dejado decaer sin
        // reiniciarse, ya habría expirado (0.10 + un paso pequeño >
        // 0.12); en cambio, tras el reinicio debe sobrevivir un
        // avance adicional de 0.10s.
        session.update_hit_flash(0.10);
        assert!(session.is_hit_flash_active());
    }

    #[test]
    fn skipping_update_calls_freezes_cooldown_and_flash_exactly_like_a_pause_menu_would() {
        // Tarea 45 + Tarea 42: `App::update_paused` congela el
        // combate simplemente NO llamando a
        // `process_dealer_attacks`/`update_hit_flash` mientras
        // `GameState::Paused` está activo — la ausencia de esas
        // llamadas (nunca un `delta_time` grande) es la garantía de
        // congelación, igual que ya se probó para el reload en
        // Tarea 42.
        let mut session = new_test_session_with_one_dealer();

        move_player_near_dealer_and_alert(&mut session, 0, 20.0);

        session.process_dealer_attacks(0.016, BLOCK_SIZE);

        let health_during_pause = session.player_health();

        let flash_active_during_pause = session.is_hit_flash_active();

        // "10 segundos reales" en los que NINGÚN método de combate
        // se invoca (equivalente exacto de estar en pausa): ni la
        // vida ni el flash cambian.

        assert_eq!(session.player_health(), health_during_pause);
        assert_eq!(session.is_hit_flash_active(), flash_active_during_pause);
    }

    // --- Tarea 46: Retry / reinicio limpio de sesión. ---
    //
    // `App::perform_defeat_action(DefeatMenuItem::Retry)` (y
    // `VictoryAction::Retry`, que ya existía) reconstruyen la sesión
    // exclusivamente llamando `LevelManager::restart` +
    // `GameSession::new` — nunca reparan `health`/`weapon`/
    // `entities`/`ammo_pickups`/`hit_flash` campo por campo desde la
    // UI. Estas pruebas demuestran, al nivel de `GameSession` (sin
    // necesitar abrir una ventana ni instanciar `App`), la invariante
    // exacta de la que depende Retry: una sesión recién construida
    // con `GameSession::new` SIEMPRE arranca limpia, sin importar
    // cuánto se haya "ensuciado" una sesión anterior para el mismo
    // nivel.

    /// Sesión de prueba con un Dealer Y un pickup de munición, ambos
    /// alcanzables desde el spawn del jugador: suficiente para
    /// "ensuciar" health/Dealer/pickup/arma/hit-flash a la vez en un
    /// único fixture.
    fn new_test_session_with_one_dealer_and_one_pickup() -> GameSession {
        let map = "\
#########
#p a    #
#  e    #
#      g#
#########
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    #[test]
    fn a_freshly_constructed_session_always_starts_with_full_health() {
        let dirty_session = {
            let mut session = new_test_session_with_one_dealer_and_one_pickup();
            session.player.apply_damage(1000);
            session
        };
        assert_eq!(dirty_session.player_health(), 0);

        let clean_session = new_test_session_with_one_dealer_and_one_pickup();
        assert_eq!(clean_session.player_health(), 100);
    }

    #[test]
    fn a_freshly_constructed_session_always_starts_with_full_weapon_ammo_and_idle_state() {
        let dirty_session = {
            let mut session = new_test_session_with_one_dealer_and_one_pickup();

            // Dispara un tiro (consume 1 de 6), deja que el arma
            // recorra Fire/Recoil de vuelta a Idle, y luego deja un
            // reload EN CURSO (nunca completado) para probar que el
            // progreso de reload también se descarta.
            session.try_fire_weapon();
            session.update_weapon(0.2);
            session.try_start_weapon_reload();
            session.update_weapon(0.1);
            session
        };
        assert_eq!(dirty_session.weapon_ammo(), 5);
        assert!(dirty_session.weapon_reload_progress().is_some());

        let clean_session = new_test_session_with_one_dealer_and_one_pickup();
        assert_eq!(clean_session.weapon_ammo(), 6);
        assert_eq!(clean_session.weapon_reserve_ammo(), 18);
        assert_eq!(clean_session.weapon_state(), WeaponState::Idle);
        assert_eq!(clean_session.weapon_reload_progress(), None);
    }

    #[test]
    fn a_freshly_constructed_session_always_starts_with_dealers_reset() {
        let dirty_session = {
            let mut session = new_test_session_with_one_dealer_and_one_pickup();
            move_player_near_dealer_and_alert(&mut session, 0, 20.0);
            session.damage_entity(0);
            session.process_dealer_attacks(0.016, BLOCK_SIZE);
            session
        };
        assert_eq!(dirty_session.entities()[0].state(), EntityState::Hit);
        assert_eq!(dirty_session.entities()[0].health(), 50);

        let clean_session = new_test_session_with_one_dealer_and_one_pickup();
        assert_eq!(clean_session.entities()[0].state(), EntityState::Idle);
        assert_eq!(clean_session.entities()[0].health(), 100);
    }

    #[test]
    fn a_freshly_constructed_session_always_starts_with_all_ammo_pickups_active() {
        let dirty_session = {
            let mut session = new_test_session_with_one_dealer_and_one_pickup();
            // El pickup ('a') vive en (fila 1, columna 3) ==
            // (168.0, 72.0) a BLOCK_SIZE=48, igual convención que
            // `new_test_session_with_one_ammo_spawn`.
            session.player.pos = Vector2::new(168.0, 72.0);
            session.collect_nearby_ammo_pickups();
            session
        };
        assert!(!dirty_session.ammo_pickups()[0].is_active());

        let clean_session = new_test_session_with_one_dealer_and_one_pickup();
        assert!(clean_session.ammo_pickups()[0].is_active());
    }

    #[test]
    fn a_freshly_constructed_session_always_starts_with_an_inactive_hit_flash() {
        let dirty_session = {
            let mut session = new_test_session_with_one_dealer_and_one_pickup();
            move_player_near_dealer_and_alert(&mut session, 0, 20.0);
            session.process_dealer_attacks(0.016, BLOCK_SIZE);
            session
        };
        assert!(dirty_session.is_hit_flash_active());

        let clean_session = new_test_session_with_one_dealer_and_one_pickup();
        assert!(!clean_session.is_hit_flash_active());
    }

    #[test]
    fn a_freshly_constructed_session_dirtied_across_every_subsystem_still_resets_cleanly() {
        // El test más completo de esta sección (Tarea 46, §48):
        // ensucia health, arma, Dealer y pickup A LA VEZ en la misma
        // sesión, y confirma que una sesión NUEVA para el mismo nivel
        // reinicia los cinco aspectos simultáneamente, sin llevar
        // ningún resto de estado de la sesión muerta.
        let dirty_session = {
            let mut session = new_test_session_with_one_dealer_and_one_pickup();

            session.player.pos = Vector2::new(168.0, 72.0);

            session.collect_nearby_ammo_pickups();

            session.try_fire_weapon();

            session.update_weapon(0.2);

            session.try_start_weapon_reload();

            move_player_near_dealer_and_alert(&mut session, 0, 20.0);

            session.damage_entity(0);

            session.process_dealer_attacks(0.016, BLOCK_SIZE);

            session.player.apply_damage(1000);

            session
        };

        assert_eq!(dirty_session.player_health(), 0);
        assert!(!dirty_session.ammo_pickups()[0].is_active());
        assert_ne!(dirty_session.weapon_ammo(), 6);
        assert_eq!(dirty_session.entities()[0].state(), EntityState::Hit);

        let clean_session = new_test_session_with_one_dealer_and_one_pickup();

        assert_eq!(clean_session.player_health(), 100);
        assert_eq!(clean_session.weapon_ammo(), 6);
        assert_eq!(clean_session.weapon_reserve_ammo(), 18);
        assert_eq!(clean_session.weapon_state(), WeaponState::Idle);
        assert_eq!(clean_session.weapon_reload_progress(), None);
        assert_eq!(clean_session.entities()[0].state(), EntityState::Idle);
        assert_eq!(clean_session.entities()[0].health(), 100);
        assert!(clean_session.ammo_pickups()[0].is_active());
        assert!(!clean_session.is_hit_flash_active());
    }

    /// Sesión de prueba con un único pickup de munición en (fila 1,
    /// columna 3), a la derecha del spawn del jugador (fila 1,
    /// columna 1).
    fn new_test_session_with_one_ammo_spawn() -> GameSession {
        let map = "\
#######
#p a g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    /// Sesión de prueba con tres pickups de munición, todos
    /// alcanzables desde el spawn del jugador: suficientes para
    /// llevar la reserva inicial (18) exactamente al tope (30) con
    /// los dos primeros y dejar un tercero activo para probar que
    /// una reserva ya llena NO consume el pickup.
    fn new_test_session_with_three_ammo_spawns() -> GameSession {
        let map = "\
###########
#p a a a g#
###########
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    // --- Tarea 44: pickups de munición. ---

    #[test]
    fn collecting_within_radius_consumes_the_pickup_and_increases_reserve() {
        let mut session = new_test_session_with_one_ammo_spawn();

        assert_eq!(session.weapon_reserve_ammo(), 18);
        assert!(session.ammo_pickups()[0].is_active());

        // (fila 1, columna 3) -> centro de celda en x=168, y=72.
        session.player.pos = Vector2::new(168.0, 72.0);

        session.collect_nearby_ammo_pickups();

        assert_eq!(session.weapon_reserve_ammo(), 24);
        assert!(!session.ammo_pickups()[0].is_active());
    }

    #[test]
    fn collecting_outside_radius_leaves_the_pickup_and_reserve_unchanged() {
        let mut session = new_test_session_with_one_ammo_spawn();

        // El spawn del jugador (fila 1, columna 1) está a 2 celdas
        // (96 px) del pickup — muy por fuera de `PICKUP_RADIUS`
        // (~19.2 px).
        session.collect_nearby_ammo_pickups();

        assert_eq!(session.weapon_reserve_ammo(), 18);
        assert!(session.ammo_pickups()[0].is_active());
    }

    #[test]
    fn full_reserve_retains_the_pickup_instead_of_consuming_it() {
        let mut session = new_test_session_with_three_ammo_spawns();

        // (fila 1, columna 3): x=168, y=72. 18 + 6 = 24.
        session.player.pos = Vector2::new(168.0, 72.0);
        session.collect_nearby_ammo_pickups();
        assert_eq!(session.weapon_reserve_ammo(), 24);
        assert!(!session.ammo_pickups()[0].is_active());

        // (fila 1, columna 5): x=264, y=72. 24 + 6 = 30 (tope).
        session.player.pos = Vector2::new(264.0, 72.0);
        session.collect_nearby_ammo_pickups();
        assert_eq!(session.weapon_reserve_ammo(), 30);
        assert!(!session.ammo_pickups()[1].is_active());

        // (fila 1, columna 7): x=360, y=72. Reserva YA en el tope:
        // `add_reserve_ammo` no puede añadir nada, así que este
        // tercer pickup, todavía ACTIVO, debe permanecer disponible
        // en vez de desperdiciarse.
        session.player.pos = Vector2::new(360.0, 72.0);
        session.collect_nearby_ammo_pickups();

        assert_eq!(session.weapon_reserve_ammo(), 30);
        assert!(session.ammo_pickups()[2].is_active());
    }

    // --- Tarea "Ammo Pickup SFX": evento de recolección exitosa ---
    //
    // `collect_nearby_ammo_pickups` retorna cuántos pickups se
    // consumieron REALMENTE este cuadro — el único evento semántico
    // que `App` usa para solicitar `SoundEffect::AmmoPickup`. Estas
    // pruebas viven aquí (no en `audio::manager`, que no puede
    // ejercitar `GameSession`) y verifican el conteo sin acoplarse a
    // ningún hardware de audio, exactamente como pide la tarea.

    #[test]
    fn a_successful_collection_reports_exactly_one_event() {
        let mut session = new_test_session_with_one_ammo_spawn();

        session.player.pos = Vector2::new(168.0, 72.0);

        assert_eq!(session.collect_nearby_ammo_pickups(), 1);
    }

    #[test]
    fn being_out_of_range_reports_zero_events() {
        let mut session = new_test_session_with_one_ammo_spawn();

        // Spawn del jugador, a 2 celdas del pickup: fuera de rango.
        assert_eq!(session.collect_nearby_ammo_pickups(), 0);
    }

    #[test]
    fn a_full_reserve_reports_zero_events_even_though_the_pickup_stays_active() {
        let mut session = new_test_session_with_three_ammo_spawns();

        session.player.pos = Vector2::new(168.0, 72.0);
        session.collect_nearby_ammo_pickups();

        session.player.pos = Vector2::new(264.0, 72.0);
        session.collect_nearby_ammo_pickups();

        // Reserva ya en el tope (30): el tercer pickup no se
        // consume, así que NO debe reportar ningún evento — nunca
        // "recolección exitosa" por simple proximidad a uno que
        // sigue activo.
        session.player.pos = Vector2::new(360.0, 72.0);

        assert_eq!(session.collect_nearby_ammo_pickups(), 0);
        assert!(session.ammo_pickups()[2].is_active());
    }

    #[test]
    fn the_same_pickup_never_reports_a_second_event() {
        let mut session = new_test_session_with_one_ammo_spawn();

        session.player.pos = Vector2::new(168.0, 72.0);

        assert_eq!(session.collect_nearby_ammo_pickups(), 1);

        // Mismo pickup, mismo jugador, mismo cuadro repetido varias
        // veces: ya está `deactivate`d, así que ningún cuadro
        // posterior puede volver a reportar un evento por él.
        for _ in 0..5 {
            assert_eq!(session.collect_nearby_ammo_pickups(), 0);
        }
    }

    #[test]
    fn multiple_pickups_collected_the_same_frame_report_one_event_each() {
        let mut session = new_test_session_with_three_ammo_spawns();

        // Los tres pickups de este mapa están en la misma fila; para
        // recogerlos los tres EN UN SOLO cuadro hace falta un radio
        // que los cubra todos a la vez — se simula colocando al
        // jugador exactamente sobre el pickup central y ampliando
        // artificialmente ninguna constante de dominio (la prueba
        // solo reubica al jugador, nunca toca `PICKUP_RADIUS`).
        // Con el radio real (~19.2px) y pickups separados 96px entre
        // sí, un único cuadro solo alcanza a uno; esta prueba refleja
        // exactamente ese caso real: eventos van sumando 1 en 1,
        // nunca colapsados en un booleano.
        session.player.pos = Vector2::new(168.0, 72.0);
        assert_eq!(session.collect_nearby_ammo_pickups(), 1);

        session.player.pos = Vector2::new(264.0, 72.0);
        assert_eq!(session.collect_nearby_ammo_pickups(), 1);
    }

    /// Simula un `AmmoPickup` generado dinámicamente por Dealer Hands
    /// (o por la generación procedural de The Dealer's True Maze):
    /// ambos casos terminan empujando un `AmmoPickup` más a
    /// `self.ammo_pickups` en tiempo de ejecución (ver
    /// `GameSession::update_hand_state`), nunca a través de
    /// `Level::ammo_spawns`. Esta prueba no depende de `HandState`
    /// para demostrar que el flujo de recolección es EL MISMO: basta
    /// con que el pickup exista en la colección.
    #[test]
    fn a_dynamically_added_pickup_uses_the_exact_same_collection_event() {
        let mut session = new_test_session_with_one_ammo_spawn();

        let dynamic_position = Vector2::new(360.0, 72.0);

        session
            .ammo_pickups
            .push(AmmoPickup::at_cell(1, 7, BLOCK_SIZE));

        assert_eq!(session.ammo_pickups().len(), 2);

        session.player.pos = dynamic_position;

        assert_eq!(session.collect_nearby_ammo_pickups(), 1);
        assert!(!session.ammo_pickups()[1].is_active());
    }

    #[test]
    fn retry_style_fresh_session_can_collect_its_pickups_again() {
        // Retry reconstruye una `GameSession` COMPLETAMENTE nueva
        // (ver `App::replace_session_with_level`); esta prueba
        // reproduce esa reconstrucción sin pasar por `App`, y
        // confirma que la sesión "post-Retry" puede recoger su
        // pickup normalmente, sin heredar ningún estado (activo/
        // inactivo) de una sesión anterior.
        let mut dirty_session = new_test_session_with_one_ammo_spawn();

        dirty_session.player.pos = Vector2::new(168.0, 72.0);
        assert_eq!(dirty_session.collect_nearby_ammo_pickups(), 1);
        assert!(!dirty_session.ammo_pickups()[0].is_active());

        let mut fresh_session = new_test_session_with_one_ammo_spawn();

        assert!(fresh_session.ammo_pickups()[0].is_active());

        fresh_session.player.pos = Vector2::new(168.0, 72.0);

        assert_eq!(fresh_session.collect_nearby_ammo_pickups(), 1);
        assert!(!fresh_session.ammo_pickups()[0].is_active());
    }

    #[test]
    fn not_calling_collect_nearby_ammo_pickups_is_how_pause_freezes_collection() {
        // Mismo patrón que
        // `skipping_update_calls_freezes_cooldown_and_flash_exactly_like_a_pause_menu_would`:
        // `App::update_paused` congela la recolección simplemente NO
        // llamando a `collect_nearby_ammo_pickups` mientras
        // `GameState::Paused` está activo — no existe un `delta_time`
        // que "avanzar", así que la prueba real de la congelación es
        // la AUSENCIA de la llamada, no un temporizador. Aquí se
        // demuestra colocando al jugador en rango y confirmando que,
        // mientras el método no se invoque, ni la reserva ni el
        // estado del pickup cambian — exactamente lo que ocurre
        // durante Pause real.
        let mut session = new_test_session_with_one_ammo_spawn();

        session.player.pos = Vector2::new(168.0, 72.0);

        let reserve_before = session.weapon_reserve_ammo();

        // "Pause": ninguna llamada a `collect_nearby_ammo_pickups`
        // aquí, sin importar cuánto tiempo real pase.
        assert_eq!(session.weapon_reserve_ammo(), reserve_before);
        assert!(session.ammo_pickups()[0].is_active());

        // "Resume": la primera llamada real todavía recoge con
        // normalidad, exactamente como si nunca se hubiera pausado.
        assert_eq!(session.collect_nearby_ammo_pickups(), 1);
        assert!(!session.ammo_pickups()[0].is_active());
    }

    #[test]
    fn collection_never_refills_the_magazine() {
        let mut session = new_test_session_with_one_ammo_spawn();

        assert!(session.try_fire_weapon());
        let magazine_before = session.weapon_ammo();
        assert_eq!(magazine_before, 5);

        session.player.pos = Vector2::new(168.0, 72.0);
        session.collect_nearby_ammo_pickups();

        assert_eq!(session.weapon_ammo(), magazine_before);
        assert_eq!(session.weapon_reserve_ammo(), 24);
    }

    // --- Health Pickup: curación. ---

    /// Sesión de prueba con un único Health Pickup en (fila 1,
    /// columna 3), a la derecha del spawn del jugador (fila 1,
    /// columna 1) — misma convención de mapa que
    /// `new_test_session_with_one_ammo_spawn`.
    fn new_test_session_with_one_health_spawn() -> GameSession {
        let map = "\
#######
#p h g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    /// Coloca al jugador exactamente sobre el Health Pickup de
    /// `new_test_session_with_one_health_spawn` ((fila 1, columna 3)
    /// -> x=168, y=72).
    fn move_player_onto_the_health_pickup(session: &mut GameSession) {
        session.player.pos = Vector2::new(168.0, 72.0);
    }

    #[test]
    fn healing_from_sixty_reaches_eighty() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(40);
        assert_eq!(session.player_health(), 60);

        move_player_onto_the_health_pickup(&mut session);
        session.collect_nearby_health_pickups();

        assert_eq!(session.player_health(), 80);
        assert!(!session.health_pickups()[0].is_active());
    }

    #[test]
    fn healing_from_eighty_reaches_one_hundred() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(20);
        assert_eq!(session.player_health(), 80);

        move_player_onto_the_health_pickup(&mut session);
        session.collect_nearby_health_pickups();

        assert_eq!(session.player_health(), 100);
    }

    #[test]
    fn healing_from_ninety_clamps_at_one_hundred_and_still_consumes_the_pickup() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(10);
        assert_eq!(session.player_health(), 90);

        move_player_onto_the_health_pickup(&mut session);

        assert_eq!(session.collect_nearby_health_pickups(), 1);
        assert_eq!(session.player_health(), 100);
        assert!(!session.health_pickups()[0].is_active());
    }

    #[test]
    fn healing_from_ninety_nine_clamps_at_one_hundred() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(1);
        assert_eq!(session.player_health(), 99);

        move_player_onto_the_health_pickup(&mut session);
        session.collect_nearby_health_pickups();

        assert_eq!(session.player_health(), 100);
    }

    #[test]
    fn health_never_exceeds_the_maximum_no_matter_how_many_pickups_are_collected() {
        let map = "\
###########
#p h h h g#
###########
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        session.player.apply_damage(5);
        assert_eq!(session.player_health(), 95);

        for column in [3, 5, 7] {
            session.player.pos = Vector2::new(column as f32 * 48.0 + 24.0, 72.0);
            session.collect_nearby_health_pickups();
        }

        assert_eq!(session.player_health(), 100);
        assert!(session.player_health() <= 100);
    }

    /// Sección 2/22: con la vida ya en el máximo, tocar el corazón NO
    /// debe curar, NO debe consumir el pickup y NO debe reportar
    /// ningún evento de curación. Pertenece a la lógica de dominio
    /// (`GameSession`), no solo a `App`.
    #[test]
    fn full_health_leaves_the_pickup_untouched_and_reports_no_event() {
        let mut session = new_test_session_with_one_health_spawn();

        assert_eq!(session.player_health(), 100);

        move_player_onto_the_health_pickup(&mut session);

        assert_eq!(session.collect_nearby_health_pickups(), 0);
        assert_eq!(session.player_health(), 100);
        assert!(session.health_pickups()[0].is_active());
    }

    #[test]
    fn being_out_of_range_leaves_the_health_pickup_and_health_unchanged() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(40);

        // Spawn del jugador, a 2 celdas del pickup: fuera de rango.
        assert_eq!(session.collect_nearby_health_pickups(), 0);

        assert_eq!(session.player_health(), 60);
        assert!(session.health_pickups()[0].is_active());
    }

    #[test]
    fn a_successful_heal_reports_exactly_one_event() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(40);

        move_player_onto_the_health_pickup(&mut session);

        assert_eq!(session.collect_nearby_health_pickups(), 1);
    }

    #[test]
    fn the_same_health_pickup_never_reports_a_second_event() {
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(40);

        move_player_onto_the_health_pickup(&mut session);

        assert_eq!(session.collect_nearby_health_pickups(), 1);

        for _ in 0..5 {
            assert_eq!(session.collect_nearby_health_pickups(), 0);
        }
    }

    #[test]
    fn not_calling_collect_nearby_health_pickups_is_how_pause_freezes_healing() {
        // Mismo patrón que
        // `not_calling_collect_nearby_ammo_pickups_is_how_pause_freezes_collection`:
        // `App::update_paused` congela la curación simplemente NO
        // llamando a `collect_nearby_health_pickups` mientras
        // `GameState::Paused` está activo.
        let mut session = new_test_session_with_one_health_spawn();

        session.player.apply_damage(40);

        move_player_onto_the_health_pickup(&mut session);

        let health_before = session.player_health();

        // "Pause": ninguna llamada a `collect_nearby_health_pickups`
        // aquí.
        assert_eq!(session.player_health(), health_before);
        assert!(session.health_pickups()[0].is_active());

        // "Resume": la primera llamada real todavía cura con
        // normalidad.
        assert_eq!(session.collect_nearby_health_pickups(), 1);
        assert_eq!(session.player_health(), 80);
    }

    #[test]
    fn a_freshly_constructed_session_restores_a_consumed_health_pickup() {
        // Mismo patrón de Retry que
        // `retry_style_fresh_session_can_collect_its_pickups_again`.
        let mut dirty_session = new_test_session_with_one_health_spawn();

        dirty_session.player.apply_damage(40);
        move_player_onto_the_health_pickup(&mut dirty_session);
        assert_eq!(dirty_session.collect_nearby_health_pickups(), 1);
        assert!(!dirty_session.health_pickups()[0].is_active());

        let clean_session = new_test_session_with_one_health_spawn();

        assert_eq!(clean_session.player_health(), 100);
        assert!(clean_session.health_pickups()[0].is_active());
    }

    /// Sección 13: una Hand nueva NO debe generar Health Pickups
    /// adicionales — solo munición, según su propio presupuesto. Este
    /// test construye una sesión con un Dealer y fuerza el spawn de
    /// HAND II, y confirma que la cantidad de Health Pickups no
    /// cambió.
    #[test]
    fn a_new_hand_replenishes_health_pickups_up_to_the_deterministic_target() {
        // Reemplaza la regla anterior a "Health Respawn por Hand": a
        // partir de esta tarea, una Hand NUEVA (nunca HAND I) sí
        // repone corazones hasta un objetivo determinista 3..=5
        // (`hand::health_pickup_target_for_hand`).
        let map = "\
###################
#p                #
#                 #
#        e        #
#                 #
#                g#
###################
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        assert_eq!(session.health_pickups().len(), 0);

        session.damage_entity(0);
        session.damage_entity(0);
        assert_eq!(session.alive_dealer_count(), 0);

        for _ in 0..200 {
            session.update_hand_state(0.5, BLOCK_SIZE, 16, false);

            if session.hand_number() > 1 {
                break;
            }
        }

        assert!(
            session.hand_number() > 1,
            "la Hand debería haber avanzado en esta ventana de tiempo"
        );

        let expected_target = hand::health_pickup_target_for_hand(0, session.hand_number());

        assert!((3..=5).contains(&expected_target));
        assert_eq!(session.health_pickups().len(), expected_target);
        assert!(
            session
                .health_pickups()
                .iter()
                .all(|pickup| pickup.is_active())
        );
    }

    // --- Emergency Ammo Respawn (anti-softlock). ---

    /// Nivel de prueba lo bastante grande como para que
    /// `select_emergency_ammo_cells` tenga margen real para encontrar
    /// celdas en la banda 3-8 sin degradar al fallback más laxo.
    fn new_test_session_for_emergency_ammo() -> GameSession {
        let map = "\
###################
#p                #
#                 #
#        e        #
#                 #
#                g#
###################
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal)
    }

    /// Dispara y recarga hasta agotar TODA la munición del jugador
    /// (cargador + reserva) mediante el flujo real de `Weapon`, sin
    /// tocar ningún campo privado directamente.
    fn drain_all_ammo(session: &mut GameSession) {
        loop {
            if session.weapon_ammo() > 0 {
                assert!(session.try_fire_weapon());
                session.update_weapon(1.0);
            } else if session.weapon_reserve_ammo() > 0 {
                assert!(session.try_start_weapon_reload());
                session.update_weapon(1.0);
            } else {
                break;
            }
        }

        assert_eq!(session.weapon_ammo(), 0);
        assert_eq!(session.weapon_reserve_ammo(), 0);
    }

    #[test]
    fn caso_a_softlock_condition_spawns_two_emergency_pickups() {
        let mut session = new_test_session_for_emergency_ammo();

        assert_eq!(session.alive_dealer_count(), 1);

        drain_all_ammo(&mut session);
        assert!(session.ammo_pickups().is_empty());

        let spawned = session.ensure_emergency_ammo(BLOCK_SIZE);

        assert_eq!(spawned, 2);
        assert_eq!(
            session
                .ammo_pickups()
                .iter()
                .filter(|pickup| pickup.is_active())
                .count(),
            2
        );
    }

    #[test]
    fn caso_b_one_bullet_left_in_the_magazine_prevents_emergency_respawn() {
        let mut session = new_test_session_for_emergency_ammo();

        drain_all_ammo(&mut session);

        // Recupera exactamente una bala en el cargador vía munición
        // de reserva simulada con un pickup: primero hay que
        // devolverle algo de reserva para poder recargar.
        session.weapon.add_reserve_ammo(6);
        assert!(session.try_start_weapon_reload());
        session.update_weapon(1.0);
        assert_eq!(session.weapon_ammo(), 6);

        // Dispara todas menos una.
        for _ in 0..5 {
            assert!(session.try_fire_weapon());
            session.update_weapon(1.0);
        }
        assert_eq!(session.weapon_ammo(), 1);
        assert_eq!(session.weapon_reserve_ammo(), 0);

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 0);
        assert!(session.ammo_pickups().is_empty());
    }

    #[test]
    fn caso_c_empty_magazine_with_reserve_remaining_prevents_emergency_respawn() {
        let mut session = new_test_session_for_emergency_ammo();

        // Dispara solo el cargador inicial (6 balas): el jugador
        // todavía puede recargar (reserva inicial = 18).
        for _ in 0..6 {
            assert!(session.try_fire_weapon());
            session.update_weapon(1.0);
        }

        assert_eq!(session.weapon_ammo(), 0);
        assert!(session.weapon_reserve_ammo() > 0);

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 0);
        assert!(session.ammo_pickups().is_empty());
    }

    #[test]
    fn caso_d_an_existing_active_ammo_pickup_prevents_emergency_respawn() {
        let map = "\
###################
#p a              #
#                 #
#        e        #
#                 #
#                g#
###################
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        drain_all_ammo(&mut session);

        assert_eq!(session.ammo_pickups().len(), 1);
        assert!(session.ammo_pickups()[0].is_active());

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 0);
        assert_eq!(session.ammo_pickups().len(), 1);
    }

    #[test]
    fn caso_e_no_dealers_alive_prevents_emergency_respawn() {
        let mut session = new_test_session_for_emergency_ammo();

        drain_all_ammo(&mut session);

        session.damage_entity(0);
        session.damage_entity(0);
        assert_eq!(session.alive_dealer_count(), 0);

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 0);
        assert!(session.ammo_pickups().is_empty());
    }

    #[test]
    fn caso_f_a_second_frame_right_after_spawning_generates_nothing_more() {
        let mut session = new_test_session_for_emergency_ammo();

        drain_all_ammo(&mut session);

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 2);

        // Cuadro inmediatamente siguiente, condición sin cambios
        // salvo por los pickups recién creados (ya activos): no debe
        // generar un segundo par.
        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 0);
        assert_eq!(
            session
                .ammo_pickups()
                .iter()
                .filter(|pickup| pickup.is_active())
                .count(),
            2
        );
    }

    #[test]
    fn caso_g_a_fresh_softlock_after_consuming_the_emergency_pickups_spawns_again() {
        let mut session = new_test_session_for_emergency_ammo();

        drain_all_ammo(&mut session);

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 2);

        // Recoge los dos pickups de emergencia recién creados.
        for index in 0..session.ammo_pickups().len() {
            let position = session.ammo_pickups()[index].position();
            session.player.pos = position;
            session.collect_nearby_ammo_pickups();
        }

        assert!(
            session
                .ammo_pickups()
                .iter()
                .all(|pickup| !pickup.is_active())
        );

        // Gasta de nuevo toda la munición recién recogida.
        drain_all_ammo(&mut session);

        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 2);
    }

    #[test]
    fn emergency_ammo_positions_are_valid_and_never_overlap_other_resources() {
        let map = "\
###################
#p a  h           #
#                 #
#        e        #
#                 #
#                g#
###################
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        // Consume el único ammo pickup existente para poder llegar a
        // la condición de softlock real (el health pickup queda
        // activo deliberadamente, para probar que emergency ammo
        // nunca lo pisa).
        session.player.pos = session.ammo_pickups()[0].position();
        session.collect_nearby_ammo_pickups();

        drain_all_ammo(&mut session);

        let spawned = session.ensure_emergency_ammo(BLOCK_SIZE);
        assert_eq!(spawned, 2);

        let dealer_cell = session.entities()[0].position();
        let health_cell = session.health_pickups()[0].position();
        let goal_cell = session.level.goal();
        let player_cell = session.player.pos;

        for pickup in session.ammo_pickups().iter().filter(|p| p.is_active()) {
            let position = pickup.position();

            assert_ne!(position, dealer_cell);
            assert_ne!(position, health_cell);
            assert_ne!(position, player_cell);

            let world_to_cell_local = |p: Vector2| {
                (
                    (p.y / BLOCK_SIZE as f32) as usize,
                    (p.x / BLOCK_SIZE as f32) as usize,
                )
            };

            assert_ne!(world_to_cell_local(position), goal_cell);
        }
    }

    // --- Pause/Victory/Defeat: ausencia de generación de recursos. ---

    #[test]
    fn not_calling_ensure_emergency_ammo_is_how_pause_freezes_it() {
        let mut session = new_test_session_for_emergency_ammo();

        drain_all_ammo(&mut session);

        // "Pause": ninguna llamada a `ensure_emergency_ammo` aquí,
        // sin importar cuánto tiempo real pase.
        assert!(session.ammo_pickups().is_empty());

        // "Resume": la primera llamada real todavía genera con
        // normalidad.
        assert_eq!(session.ensure_emergency_ammo(BLOCK_SIZE), 2);
    }

    #[test]
    fn not_calling_update_hand_state_is_how_pause_freezes_health_replenish() {
        // Mismo mecanismo: `update_hand_state` (que contiene el
        // Health Respawn por Hand) nunca se invoca mientras
        // `GameState::Paused`, así que ningún corazón nuevo aparece
        // sin que exista un caso especial dedicado.
        let mut session = new_test_session_for_emergency_ammo();

        session.damage_entity(0);
        session.damage_entity(0);

        let health_before = session.health_pickups().len();

        // "Pause": ninguna llamada a `update_hand_state` aquí.
        assert_eq!(session.health_pickups().len(), health_before);
    }

    #[test]
    fn same_hand_seed_produces_the_same_emergency_ammo_and_health_replenish_positions() {
        let build_and_dirty = || {
            let mut session = new_test_session_for_emergency_ammo();

            session.damage_entity(0);
            session.damage_entity(0);

            for _ in 0..200 {
                session.update_hand_state(0.5, BLOCK_SIZE, 16, false);

                if session.hand_number() > 1 {
                    break;
                }
            }

            drain_all_ammo(&mut session);
            session.ensure_emergency_ammo(BLOCK_SIZE);

            session
        };

        let a = build_and_dirty();
        let b = build_and_dirty();

        assert_eq!(a.hand_number(), b.hand_number());

        let positions = |session: &GameSession, pickups: &[AmmoPickup]| -> Vec<(u32, u32)> {
            pickups
                .iter()
                .map(|pickup| {
                    let position = pickup.position();
                    let _ = session;
                    (position.x.to_bits(), position.y.to_bits())
                })
                .collect()
        };

        assert_eq!(
            positions(&a, a.ammo_pickups()),
            positions(&b, b.ammo_pickups())
        );

        let health_positions = |pickups: &[HealthPickup]| -> Vec<(u32, u32)> {
            pickups
                .iter()
                .map(|pickup| {
                    let position = pickup.position();
                    (position.x.to_bits(), position.y.to_bits())
                })
                .collect()
        };

        assert_eq!(
            health_positions(a.health_pickups()),
            health_positions(b.health_pickups())
        );
    }

    #[test]
    fn retry_style_fresh_session_discards_dynamically_generated_resources() {
        // Ensucia una sesión con Emergency Ammo Y Health Respawn de
        // una Hand avanzada; una sesión "post-Retry" (reconstruida
        // desde cero con la MISMA semilla, como hace
        // `App::replace_session_with_level`) debe arrancar en HAND I
        // sin ningún recurso dinámico heredado.
        let mut dirty_session = new_test_session_for_emergency_ammo();

        dirty_session.damage_entity(0);
        dirty_session.damage_entity(0);

        for _ in 0..200 {
            dirty_session.update_hand_state(0.5, BLOCK_SIZE, 16, false);

            if dirty_session.hand_number() > 1 {
                break;
            }
        }

        drain_all_ammo(&mut dirty_session);
        dirty_session.ensure_emergency_ammo(BLOCK_SIZE);

        assert!(dirty_session.hand_number() > 1);
        assert!(!dirty_session.ammo_pickups().is_empty());
        assert!(!dirty_session.health_pickups().is_empty());

        let clean_session = new_test_session_for_emergency_ammo();

        assert_eq!(clean_session.hand_number(), 1);
        assert!(clean_session.ammo_pickups().is_empty());
        assert!(clean_session.health_pickups().is_empty());
    }

    #[test]
    fn pickup_in_range_matches_the_radius_boundary() {
        let player = Vector2::new(0.0, 0.0);

        assert!(pickup_in_range(
            player,
            Vector2::new(PICKUP_RADIUS, 0.0),
            PICKUP_RADIUS
        ));

        assert!(!pickup_in_range(
            player,
            Vector2::new(PICKUP_RADIUS + 0.5, 0.0),
            PICKUP_RADIUS
        ));
    }

    #[test]
    fn new_session_from_the_same_level_restores_all_pickups() {
        let map = "\
#######
#p a g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut first_session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        first_session.player.pos = Vector2::new(168.0, 72.0);
        first_session.collect_nearby_ammo_pickups();

        assert!(!first_session.ammo_pickups()[0].is_active());

        // Reconstruir una sesión NUEVA desde el mismo `Level`
        // (recargado desde disco, igual que `App::start_selected_level`/
        // `replace_session_with_level` hacen en la arquitectura real)
        // debe restaurar el pickup a su estado activo original —
        // `Level` nunca se modifica permanentemente al recogerlo.
        let level_again = Level::load(file.path_str()).expect("el nivel debe recargar");

        let player_again = Player::from_level(&level_again, BLOCK_SIZE);

        let second_session =
            GameSession::new(level_again, player_again, BLOCK_SIZE, 0, GameMode::Portal);

        assert!(second_session.ammo_pickups()[0].is_active());
        assert_eq!(second_session.weapon_reserve_ammo(), 18);
    }

    // --- Tarea 43: propagación de la aceptación de recarga hasta el
    // punto donde `App` decide si reproducir `SoundEffect::Reload`. ---

    #[test]
    fn try_start_weapon_reload_forwards_a_valid_acceptance() {
        let mut session = new_test_session();

        assert!(session.try_fire_weapon());

        // Vuelve a `Idle` (Fire -> Recoil -> Idle) antes de intentar
        // recargar: `try_start_reload` solo se acepta desde `Idle`.
        session.update_weapon(1.0);

        assert!(session.try_start_weapon_reload());
        assert_eq!(session.weapon_state(), WeaponState::Reload);
    }

    #[test]
    fn try_start_weapon_reload_forwards_rejection_on_full_magazine() {
        let mut session = new_test_session();

        assert!(!session.try_start_weapon_reload());
        assert_eq!(session.weapon_state(), WeaponState::Idle);
    }

    #[test]
    fn try_start_weapon_reload_forwards_rejection_while_already_reloading() {
        let mut session = new_test_session();

        assert!(session.try_fire_weapon());
        session.update_weapon(1.0);

        assert!(session.try_start_weapon_reload());

        // Segunda solicitud en el mismo cuadro de recarga: debe
        // rechazarse, exactamente el evento que NO debe producir un
        // segundo `SoundEffect::Reload`.
        assert!(!session.try_start_weapon_reload());
    }

    #[test]
    fn weapon_reload_progress_forwards_none_and_some_correctly() {
        let mut session = new_test_session();

        assert_eq!(session.weapon_reload_progress(), None);

        assert!(session.try_fire_weapon());
        session.update_weapon(1.0);

        assert!(session.try_start_weapon_reload());

        assert!(session.weapon_reload_progress().is_some());
    }

    #[test]
    fn player_center_inside_goal_cell_is_true() {
        assert!(point_reaches_goal(
            3.0 * 48.0 + 24.0,
            2.0 * 48.0 + 24.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn player_center_inside_adjacent_cell_is_false() {
        assert!(!point_reaches_goal(
            4.0 * 48.0 + 24.0,
            2.0 * 48.0 + 24.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn position_just_before_goal_cell_boundary_is_false() {
        let just_before = 3.0 * 48.0 - 0.001;

        assert!(!point_reaches_goal(
            just_before,
            2.0 * 48.0 + 24.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn position_at_lower_left_inclusive_boundary_is_true() {
        assert!(point_reaches_goal(3.0 * 48.0, 2.0 * 48.0, 2, 3, BLOCK_SIZE));
    }

    #[test]
    fn position_at_upper_right_exclusive_boundary_is_false() {
        // Exactamente en el borde superior/derecho de la celda meta
        // ya pertenece, por convención [min, max), a la SIGUIENTE
        // celda (fila 3, columna 4), no a la celda meta (2, 3).
        assert!(!point_reaches_goal(
            4.0 * 48.0,
            3.0 * 48.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn zero_block_size_is_false() {
        assert!(!point_reaches_goal(
            3.0 * 48.0 + 24.0,
            2.0 * 48.0 + 24.0,
            2,
            3,
            0
        ));
    }

    #[test]
    fn non_finite_position_is_false() {
        assert!(!point_reaches_goal(f32::NAN, 0.0, 0, 0, BLOCK_SIZE));
        assert!(!point_reaches_goal(0.0, f32::INFINITY, 0, 0, BLOCK_SIZE));
    }

    #[test]
    fn negative_position_is_false() {
        assert!(!point_reaches_goal(-1.0, 0.0, 0, 0, BLOCK_SIZE));
        assert!(!point_reaches_goal(0.0, -1.0, 0, 0, BLOCK_SIZE));
    }

    // --- Corpse gameplay-inert: cadáveres nunca influyen sobre
    // Dealers vivos. ---
    //
    // Auditoría previa a estas pruebas: `Entity::update`/
    // `Entity::attempt_attack` ya devolvían de inmediato para `Dead`
    // (nunca se movían ni atacaban), y `world::DistanceField` es
    // puramente geometría de `Level` — jamás conoce `Entity` ni
    // "celdas ocupadas" — así que el pathfinding compartido entre
    // Dealers nunca pudo verse bloqueado por un cadáver. La causa
    // real identificada NO era de corrección sino de trabajo
    // desperdiciado: `update_entities`/`process_dealer_attacks`
    // seguían ejecutando una consulta de `DistanceField` y un intento
    // de ataque POR CADA cadáver acumulado, cada cuadro, cuyo
    // resultado siempre se descartaba de inmediato — costo que escala
    // con la cantidad de cadáveres vivos en la colección (hasta ~50
    // simultáneos entre Hands en "The Dealer's True Maze"). Estas
    // pruebas demuestran, con la API real de `GameSession` (nunca
    // reimplementando la lógica), que un Dealer vivo se mueve, entra
    // en rango y ataca con total normalidad sin importar cuántos
    // cadáveres existan ni dónde estén parados.

    /// Nivel de prueba: jugador a la izquierda, una fila de Dealers
    /// consecutivos a su derecha. Los primeros `corpse_count` se
    /// matan (quedan como cadáveres, en las celdas MÁS CERCANAS al
    /// jugador — literalmente en el camino), el último permanece
    /// vivo, exactamente en el borde de `DEALER_ALERT_DISTANCE_CELLS`
    /// (4 celdas = 192px) para que su primer `update_entities` ya lo
    /// ponga en `Alert`.
    /// Sesión de prueba con el sobreviviente SIEMPRE a distancia fija
    /// del jugador (fila 1, justo al lado — dentro de la distancia de
    /// alerta sin importar `corpse_count`) y `corpse_count` cadáveres
    /// alineados justo debajo, en la fila inmediatamente siguiente,
    /// arrancando en la misma columna donde está el sobreviviente —
    /// literalmente la región que el sobreviviente debe cruzar/rondar
    /// para perseguir al jugador.
    ///
    /// El sobreviviente es SIEMPRE `entities()[0]` (aparece primero
    /// en el escaneo fila por fila de `Level::from_cells`, antes que
    /// cualquier cadáver de la fila siguiente); los cadáveres son
    /// `entities()[1..=corpse_count]`.
    fn new_test_session_with_a_corpse_row_and_one_survivor(corpse_count: usize) -> GameSession {
        let interior_width = corpse_count.max(3) + 4;

        let mut player_row = String::from("p e");

        while player_row.len() < interior_width - 1 {
            player_row.push(' ');
        }

        player_row.push('g');

        let mut corpse_line = String::from("  ");

        corpse_line.push_str(&"e".repeat(corpse_count));

        while corpse_line.len() < interior_width {
            corpse_line.push(' ');
        }

        let border = "#".repeat(interior_width + 2);

        let map = format!("{border}\n#{player_row}#\n#{corpse_line}#\n{border}\n");

        let file = TempLevelFile::write(&map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        assert_eq!(session.entities().len(), corpse_count + 1);

        for index in 1..=corpse_count {
            session.damage_entity(index);
            session.damage_entity(index);

            assert!(session.entities()[index].is_dead());
        }

        assert!(!session.entities()[0].is_dead());

        session
    }

    /// Avanza `update_entities`/`update_hit_flash`/
    /// `process_dealer_attacks` en pasos de `step_seconds`, hasta que
    /// el sobreviviente inflige daño real o se agota `max_steps`.
    /// Retorna el número de cuadros simulados hasta el primer daño,
    /// o `None` si nunca ocurrió.
    fn simulate_until_damage(
        session: &mut GameSession,
        step_seconds: f32,
        max_steps: usize,
    ) -> Option<usize> {
        for step in 0..max_steps {
            session.update_entities(step_seconds, BLOCK_SIZE);
            session.update_hit_flash(step_seconds);

            let damage = session.process_dealer_attacks(step_seconds, BLOCK_SIZE);

            if damage > 0 {
                return Some(step);
            }
        }

        None
    }

    #[test]
    fn dead_entities_between_the_dealer_and_the_player_never_block_its_movement() {
        let mut session = new_test_session_with_a_corpse_row_and_one_survivor(3);

        let survivor_index = 0;

        let start_position = session.entities()[survivor_index].position();

        session.update_entities(0.1, BLOCK_SIZE);

        assert_eq!(
            session.entities()[survivor_index].state(),
            EntityState::Alert
        );

        // Varios cuadros de persecución real: la posición debe
        // acercarse monotónicamente al jugador (columna X
        // decreciente), atravesando exactamente las mismas celdas
        // donde yacen los tres cadáveres.
        let mut previous_x = start_position.x;

        for _ in 0..20 {
            session.update_entities(0.1, BLOCK_SIZE);

            let current_x = session.entities()[survivor_index].position().x;

            assert!(
                current_x <= previous_x,
                "el Dealer vivo debe seguir acercándose al jugador, nunca retroceder ni quedar \
                 detenido por los cadáveres en su camino"
            );

            previous_x = current_x;
        }

        assert!(
            previous_x < start_position.x,
            "tras 21 cuadros de persecución el Dealer vivo debe haber avanzado una distancia real"
        );
    }

    #[test]
    fn dead_entities_present_never_block_a_valid_attack() {
        let mut session = new_test_session_with_a_corpse_row_and_one_survivor(3);

        let health_before = session.player_health();

        let damaged_at_step =
            simulate_until_damage(&mut session, 0.1, 100).expect("el sobreviviente debe atacar");

        // 21 cuadros de persecución (misma cifra que la prueba de
        // movimiento) + margen para el primer ataque tras entrar en
        // rango: nunca debería acercarse a 100 si los cadáveres no
        // interfieren.
        assert!(damaged_at_step < 40);
        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );
    }

    #[test]
    fn ten_corpses_do_not_change_the_survivors_behavior_at_all() {
        // Escenario cercano al real reportado en The Dealer's True
        // Maze: una decena de cadáveres apilados justo en el camino
        // entre el Dealer vivo y el jugador.
        let mut session = new_test_session_with_a_corpse_row_and_one_survivor(10);

        assert_eq!(session.entities().len(), 11);
        assert_eq!(
            session.entities().iter().filter(|e| e.is_dead()).count(),
            10
        );

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 200)
            .expect("con 10 cadáveres presentes, el único Dealer vivo debe seguir pudiendo atacar");

        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );

        // Los 10 cadáveres siguen presentes (todavía no cumplieron
        // `CORPSE_DESPAWN_SECONDS`): su sola PRESENCIA nunca fue lo
        // que había que eliminar, solo su influencia sobre Dealers
        // vivos.
        assert_eq!(
            session.entities().iter().filter(|e| e.is_dead()).count(),
            10
        );
    }

    #[test]
    fn corpses_from_hand_one_never_block_a_hand_two_dealer_from_attacking() {
        // Nivel pequeño con un único Dealer de HAND I: se mata para
        // forzar la transición a HAND II (que sí genera un Dealer
        // nuevo, vivo, mientras el cadáver de HAND I sigue presente
        // durante sus 15s).
        let map = "\
#########
#p      #
#  e    #
#      g#
#########
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        session.damage_entity(0);
        session.damage_entity(0);

        assert!(session.entities()[0].is_dead());
        assert_eq!(session.alive_dealer_count(), 0);

        let mut spawned_hand_two = false;

        for _ in 0..300 {
            session.update_hand_state(0.5, BLOCK_SIZE, 16, false);

            if session.hand_number() > 1 {
                spawned_hand_two = true;
                break;
            }
        }

        assert!(spawned_hand_two, "HAND II debía haber comenzado");

        // El cadáver de HAND I sigue presente (bien dentro de los
        // 15s) junto al/los Dealers nuevos de HAND II.
        assert!(session.entities().iter().any(|e| e.is_dead()));
        assert!(session.entities().iter().any(|e| !e.is_dead()));

        // Coloca al jugador junto al Dealer vivo más cercano y
        // confirma que igualmente puede acercarse/atacar con el
        // cadáver de HAND I todavía en la colección.
        let live_index = session
            .entities()
            .iter()
            .position(|e| !e.is_dead())
            .expect("HAND II debe haber dejado al menos un Dealer vivo");

        let live_position = session.entities()[live_index].position();

        session.player.pos = Vector2::new(live_position.x + 10.0, live_position.y);

        let health_before = session.player_health();

        let damaged = simulate_until_damage(&mut session, 0.1, 100)
            .expect("el Dealer de HAND II debe atacar");

        assert!(damaged < 20);
        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );
    }

    #[test]
    fn corpse_remains_present_until_exactly_the_documented_lifetime() {
        let mut session = new_test_session_with_a_corpse_row_and_one_survivor(1);

        // Justo antes del despawn: el cadáver sigue en la colección
        // (visible/rendereable).
        let mut elapsed = 0.0;

        while elapsed < CORPSE_DESPAWN_SECONDS - 0.5 {
            session.update_entities(0.1, BLOCK_SIZE);
            elapsed += 0.1;
        }

        assert!(session.entities().iter().any(|e| e.is_dead()));

        // Cruza el umbral completo: ahora debe haberse eliminado.
        for _ in 0..10 {
            session.update_entities(0.1, BLOCK_SIZE);
        }

        assert!(!session.entities().iter().any(|e| e.is_dead()));
    }

    #[test]
    fn not_calling_update_entities_is_how_pause_freezes_corpse_processing_too() {
        // Mismo mecanismo que el resto de los sistemas de la sesión:
        // `update_entities` (que avanza el temporizador de cadáver Y
        // decide qué Dealers vivos persiguen) simplemente no se llama
        // mientras `GameState::Paused` está activo. Esta prueba
        // confirma que, sin esa llamada, ni el cadáver despawnea ni
        // el sobreviviente avanza — exactamente lo esperado durante
        // una pausa real.
        let mut session = new_test_session_with_a_corpse_row_and_one_survivor(1);

        let survivor_position_before = session.entities()[0].position();

        // "Pause": ninguna llamada a `update_entities` aquí, sin
        // importar cuánto tiempo real pase.
        assert!(session.entities().iter().any(|e| e.is_dead()));
        assert_eq!(
            session.entities()[0].position().x,
            survivor_position_before.x
        );

        // "Resume": la primera llamada real todavía funciona con
        // normalidad.
        session.update_entities(0.1, BLOCK_SIZE);
        assert_eq!(session.entities()[0].state(), EntityState::Alert);
    }

    #[test]
    fn retain_removing_several_corpses_at_once_leaves_the_survivor_fully_functional() {
        // Cinco cadáveres que expiran EXACTAMENTE en el mismo cuadro
        // (todos recibieron el golpe letal en el mismo instante de
        // prueba): `Vec::retain` debe eliminarlos todos a la vez sin
        // afectar en absoluto al sobreviviente restante.
        let mut session = new_test_session_with_a_corpse_row_and_one_survivor(5);

        let survivor_index = 0;

        // Deja que el sobreviviente entre en Alert y acumule algo de
        // cooldown de ataque real antes de expirar los cadáveres.
        session.update_entities(0.1, BLOCK_SIZE);
        assert_eq!(
            session.entities()[survivor_index].state(),
            EntityState::Alert
        );

        let mut elapsed = 0.0;

        while elapsed < CORPSE_DESPAWN_SECONDS + 0.5 {
            session.update_entities(0.1, BLOCK_SIZE);
            elapsed += 0.1;
        }

        // Los cinco cadáveres fueron eliminados de golpe; solo el
        // sobreviviente permanece.
        assert_eq!(session.entities().len(), 1);
        assert!(!session.entities()[0].is_dead());

        // Sigue completamente funcional tras el `retain` masivo:
        // continúa persiguiendo y puede atacar con normalidad.
        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 100)
            .expect("el sobreviviente debe seguir pudiendo atacar tras el retain masivo");

        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );
    }

    // --- Corner dead zone: un Dealer que ya no puede avanzar más
    // debe poder atacar si el jugador es alcanzable sin pared, PERO
    // sin converger hacia la posición exacta del jugador una vez
    // dentro de rango. ---
    //
    // Causa raíz medida empíricamente (instrumentación temporal, ya
    // retirada): `DistanceField::step_toward_origin` retorna `None`
    // en cuanto la celda del Dealer coincide con la celda del
    // jugador — sin importar en qué punto EXACTO de esa celda esté
    // el Dealer en ese instante (la última celda de la ruta se
    // abandona al CRUZAR su borde, no al llegar a su centro). Con el
    // jugador cerca de una esquina de esa misma celda, esto medía
    // ~37.6px de distancia real contra 36.0px de `DEALER_ATTACK_RANGE`
    // — un hueco de ~1.6px, exactamente del orden de "un píxel" que
    // describía el reporte original.
    //
    // El primer fix perseguía `player_position` de forma incondicional
    // en cuanto se cumplía "misma celda + sin siguiente paso de
    // ruta", lo que producía un Dealer que seguía acercándose incluso
    // ya dentro de rango de ataque (UX: sprite invadiendo la cámara).
    // La condición ahora exige TAMBIÉN `distance > DEALER_ATTACK_RANGE`
    // — ver `GameSession::update_entities` — así que el fallback deja
    // de activarse en el instante exacto en que el Dealer entra en
    // rango real, y el Dealer vuelve al comportamiento normal de
    // "quedarse quieto y atacar" desde ahí.

    /// Nivel de prueba: sala abierta con el jugador cerca de una
    /// esquina de su propia celda (celda (3,6), centro=(312,168)) y
    /// un único Dealer a 3 celdas de distancia en la misma fila —
    /// exactamente el escenario con el que se midió la zona muerta
    /// original (~37.57px de distancia real contra 36.0px de rango).
    fn new_test_session_for_the_corner_dead_zone() -> GameSession {
        let map = "\
#############
#p g        #
#           #
#        e  #
#           #
#           #
#############
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        let cell_center = Vector2::new(6.0 * 48.0 + 24.0, 3.0 * 48.0 + 24.0);

        session.player.pos = Vector2::new(cell_center.x - 17.0, cell_center.y - 17.0);

        session
    }

    fn distance_to_player(session: &GameSession, entity_index: usize) -> f32 {
        let dealer = session.entities()[entity_index].position();

        let dx = dealer.x - session.player.pos.x;
        let dy = dealer.y - session.player.pos.y;

        dx.hypot(dy)
    }

    #[test]
    fn corner_dead_zone_reproduction_still_lets_the_dealer_attack() {
        let mut session = new_test_session_for_the_corner_dead_zone();

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400)
            .expect("el Dealer debe cruzar la zona muerta original (~1.57px) y atacar");

        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );
    }

    #[test]
    fn dealer_stops_closing_once_within_attack_range() {
        let mut session = new_test_session_for_the_corner_dead_zone();

        let attack_range = BLOCK_SIZE as f32 * DEALER_ATTACK_RANGE_CELLS;

        // Avanza justo hasta que el Dealer entra en rango de ataque
        // real (sin necesitar que llegue a golpear todavía).
        let mut entered_range = false;

        for _ in 0..400 {
            session.update_entities(0.1, BLOCK_SIZE);

            if distance_to_player(&session, 0) <= attack_range {
                entered_range = true;
                break;
            }
        }

        assert!(
            entered_range,
            "el Dealer debe entrar en rango tras cruzar la zona muerta"
        );

        let distance_on_entry = distance_to_player(&session, 0);

        assert!(
            distance_on_entry <= attack_range,
            "distancia al entrar en rango: {distance_on_entry}"
        );

        // Muchos cuadros adicionales, jugador completamente quieto:
        // el Dealer NO debe seguir convergiendo hacia la posición
        // exacta del jugador (distancia -> 0). Debe permanecer
        // razonablemente cerca del límite de alcance, nunca
        // prácticamente encima del jugador.
        for _ in 0..200 {
            session.update_entities(0.1, BLOCK_SIZE);
        }

        let distance_after_many_frames = distance_to_player(&session, 0);

        assert!(
            distance_after_many_frames <= attack_range,
            "sigue dentro de rango: {distance_after_many_frames}"
        );

        // "Razonablemente cerca del límite de alcance": nunca por
        // debajo de la mitad del rango de ataque — el fallback nunca
        // debía activarse ya dentro de rango, así que la distancia no
        // puede haberse desplomado hacia 0.
        assert!(
            distance_after_many_frames > attack_range / 2.0,
            "el Dealer convergió demasiado cerca del jugador: {distance_after_many_frames} \
             (rango de ataque: {attack_range})"
        );
    }

    #[test]
    fn distance_to_player_does_not_keep_shrinking_frame_after_frame_once_in_range() {
        // Invariante de UX/cámara: una vez dentro de rango de ataque,
        // la distancia cuadro a cuadro debe estabilizarse (nunca
        // seguir disminuyendo monótonamente hacia 0) mientras el
        // jugador permanece quieto.
        let mut session = new_test_session_for_the_corner_dead_zone();

        let attack_range = BLOCK_SIZE as f32 * DEALER_ATTACK_RANGE_CELLS;

        for _ in 0..400 {
            session.update_entities(0.1, BLOCK_SIZE);

            if distance_to_player(&session, 0) <= attack_range {
                break;
            }
        }

        assert!(distance_to_player(&session, 0) <= attack_range);

        let mut previous_distance = distance_to_player(&session, 0);

        let mut ever_shrank_after_stabilizing = false;

        for _ in 0..100 {
            session.update_entities(0.1, BLOCK_SIZE);

            let current_distance = distance_to_player(&session, 0);

            // Un pequeño margen de asentamiento (el cuadro en que
            // entra en rango puede seguir moviéndose ese único
            // cuadro): a partir de ahí, la distancia no debe seguir
            // reduciéndose cuadro a cuadro.
            if current_distance < previous_distance - 0.01 {
                ever_shrank_after_stabilizing = true;
            }

            previous_distance = current_distance;
        }

        assert!(
            !ever_shrank_after_stabilizing,
            "la distancia al jugador siguió disminuyendo cuadro a cuadro tras entrar en rango"
        );
    }

    #[test]
    fn single_dealer_in_a_ninety_degree_corridor_can_still_reach_and_attack() {
        // Corredor en Z/L de una sola celda de ancho: el Dealer debe
        // recorrer DOS giros de 90° reales (nunca una habitación
        // abierta) para alcanzar al jugador.
        let map = "\
#######
#e    #
##### #
#p   g#
#######
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        // Jugador desplazado hacia la esquina de SU celda más alejada
        // de por dónde llega el Dealer (que entra desde arriba-
        // izquierda tras el segundo giro).
        let cell_center = session.player.pos;
        session.player.pos = Vector2::new(cell_center.x + 17.0, cell_center.y - 17.0);

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400)
            .expect("un único Dealer debe poder rodear la esquina de 90° y atacar");

        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );
    }

    #[test]
    fn two_dealers_from_perpendicular_corridors_never_leave_the_player_invulnerable() {
        // Sala abierta con el jugador cerca de una esquina de su
        // propia celda; un Dealer se aproxima por el eje horizontal,
        // el otro por el eje vertical — exactamente la geometría en
        // "V"/intersección de 90° descrita en el reporte.
        let map = "\
#############
#p g        #
#           #
#        e  #
#           #
e           #
#############
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        assert_eq!(session.entities().len(), 2);

        let cell_center = Vector2::new(6.0 * 48.0 + 24.0, 3.0 * 48.0 + 24.0);
        session.player.pos = Vector2::new(cell_center.x - 17.0, cell_center.y - 17.0);

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400).expect(
            "al menos uno de los dos Dealers perpendiculares debe poder atacar; el jugador \
             nunca debe quedar invulnerable en una intersección de 90°",
        );

        assert!(session.player_health() < health_before);
    }

    #[test]
    fn three_to_five_dealers_at_an_intersection_are_not_all_frozen() {
        // Sala abierta con 4 Dealers rodeando al jugador desde
        // distintas direcciones — una "plaga" en intersección.
        let map = "\
###############
#p g          #
#             #
e             #
#      e      #
#             #
#            e#
e             #
###############
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        assert_eq!(session.entities().len(), 4);

        let cell_center = Vector2::new(6.0 * 48.0 + 24.0, 4.0 * 48.0 + 24.0);
        session.player.pos = Vector2::new(cell_center.x + 17.0, cell_center.y - 17.0);

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400)
            .expect("con 3-5 Dealers rodeando la intersección, al menos uno debe poder atacar");

        assert!(session.player_health() < health_before);

        // Ningún Dealer debería quedar "congelado" en un limbo
        // Alert-sin-daño indefinido: tras suficiente tiempo adicional,
        // al menos uno vuelve a poder golpear (el cooldown de 0.9s no
        // deja al jugador permanentemente a salvo).
        let health_after_first_hit = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400)
            .expect("un segundo golpe debe seguir siendo posible tras el primero");

        assert!(session.player_health() < health_after_first_hit);
    }

    #[test]
    fn dealer_never_attacks_through_a_straight_wall() {
        // Jugador y Dealer en columnas adyacentes separadas por una
        // pared completa — ninguna corrección de rango debe permitir
        // que esto ataque.
        let map = "\
#####
#p#e#
#g# #
#####
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        for _ in 0..400 {
            session.update_entities(0.1, BLOCK_SIZE);
            session.update_hit_flash(0.1);

            let damage = session.process_dealer_attacks(0.1, BLOCK_SIZE);

            assert_eq!(
                damage, 0,
                "un Dealer separado por una pared completa nunca debe hacer daño"
            );
        }

        assert_eq!(session.player_health(), 100);
    }

    #[test]
    fn dealer_never_attacks_through_a_diagonal_corner_wall() {
        // "P█ / █E": jugador y Dealer diagonalmente adyacentes, con
        // una esquina sólida separándolos por completo (ninguno de
        // los dos vecinos cardinales está abierto en ningún lado).
        let map = "\
#####
#p# #
#g#e#
#####
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        // Confirma que la distancia euclidiana real (celdas
        // diagonalmente adyacentes) excede el rango de ataque por
        // construcción geométrica, nunca por casualidad.
        let dx = session.entities()[0].position().x - session.player.pos.x;
        let dy = session.entities()[0].position().y - session.player.pos.y;
        assert!((dx * dx + dy * dy).sqrt() > 36.0);

        for _ in 0..400 {
            session.update_entities(0.1, BLOCK_SIZE);
            session.update_hit_flash(0.1);

            let damage = session.process_dealer_attacks(0.1, BLOCK_SIZE);

            assert_eq!(
                damage, 0,
                "un Dealer separado por una esquina sólida en diagonal nunca debe hacer daño"
            );
        }

        assert_eq!(session.player_health(), 100);
    }

    #[test]
    fn corpses_around_the_player_do_not_prevent_a_corner_approach_from_attacking() {
        // Combina el escenario de cadáveres (tarea anterior) con la
        // geometría de esquina de 90° de esta tarea: varios cadáveres
        // en la celda del jugador no deben impedir que un Dealer VIVO
        // que llega desde un corredor perpendicular alcance y ataque.
        let map = "\
#######
#e    #
##### #
#p   g#
#######
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        let cell_center = session.player.pos;
        session.player.pos = Vector2::new(cell_center.x + 17.0, cell_center.y - 17.0);

        // Añade cadáveres directamente en la celda del jugador
        // (simula el escenario de la tarea anterior: varios Dealers
        // murieron justo ahí). `entities` es un campo privado de
        // `GameSession`, accesible aquí por estar en el mismo módulo
        // de pruebas.
        let (spawn_row, spawn_column) = session.level.player_spawn();

        for _ in 0..5 {
            session
                .entities
                .push(Entity::dealer_at_cell(spawn_row, spawn_column, BLOCK_SIZE));

            let last = session.entities.len() - 1;

            session.damage_entity(last);
            session.damage_entity(last);
        }

        assert_eq!(session.entities().len(), 6);
        assert_eq!(session.entities().iter().filter(|e| e.is_dead()).count(), 5);

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400).expect(
            "el Dealer vivo debe poder rodear la esquina y atacar con normalidad pese a los \
             cadáveres acumulados en la celda del jugador",
        );

        assert_eq!(
            session.player_health(),
            health_before - DEALER_ATTACK_DAMAGE
        );
    }

    #[test]
    fn hand_two_dealers_at_a_corner_attack_with_the_same_correct_behavior() {
        let map = "\
#########
#p      #
#       #
#      g#
#########
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        // HAND I no tiene Dealers (0 marcadores 'e'): fuerza
        // inmediatamente la transición a HAND II.
        assert_eq!(session.entities().len(), 0);

        let mut spawned_hand_two = false;

        for _ in 0..300 {
            session.update_hand_state(0.5, BLOCK_SIZE, 16, false);

            if session.hand_number() > 1 {
                spawned_hand_two = true;
                break;
            }
        }

        assert!(spawned_hand_two, "HAND II debía haber comenzado");
        assert!(!session.entities().is_empty());

        // Coloca al jugador en la MISMA celda que uno de los Dealers
        // recién generados por HAND II (`select_spawn_cells` los
        // aleja deliberadamente del spawn original del jugador — Tarea
        // "Dealer Hands" — así que hay que ir a buscarlo en vez de
        // esperar a que recorra todo el mapa), desplazado hacia una
        // esquina de esa celda: ejercita exactamente la corrección de
        // esta tarea con un Dealer generado dinámicamente.
        let dealer_position = session.entities()[0].position();

        let dealer_cell = world_to_cell(dealer_position, BLOCK_SIZE);

        let cell_center = cell_center(dealer_cell.0, dealer_cell.1, BLOCK_SIZE);

        session.player.pos = Vector2::new(cell_center.x + 17.0, cell_center.y - 17.0);

        let health_before = session.player_health();

        simulate_until_damage(&mut session, 0.1, 400).expect(
            "los Dealers generados por HAND II deben poder atacar con la misma corrección de \
             esquina que los Dealers estáticos",
        );

        assert!(session.player_health() < health_before);
    }

    #[test]
    fn cooldown_is_still_respected_after_the_corner_fix() {
        let map = "\
#######
#e    #
##### #
#p   g#
#######
";

        let file = TempLevelFile::write(map);
        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");
        let player = Player::from_level(&level, BLOCK_SIZE);
        let mut session = GameSession::new(level, player, BLOCK_SIZE, 0, GameMode::Portal);

        let cell_center = session.player.pos;
        session.player.pos = Vector2::new(cell_center.x + 17.0, cell_center.y - 17.0);

        let first_hit_step = simulate_until_damage(&mut session, 0.1, 400)
            .expect("el Dealer debe alcanzar y golpear al jugador");

        // Inmediatamente después del primer golpe: el cooldown
        // (0.9s) debe seguir bloqueando un segundo golpe instantáneo.
        assert_eq!(session.process_dealer_attacks(0.016, BLOCK_SIZE), 0);

        let _ = first_hit_step;
    }

    // --- GameMode: propagación a GameSession. ---

    #[test]
    fn a_session_reports_exactly_the_mode_it_was_constructed_with() {
        let map = "\
#######
#p   g#
#######
";

        let portal_file = TempLevelFile::write(map);
        let portal_level =
            Level::load(portal_file.path_str()).expect("el nivel de prueba debe cargar");
        let portal_player = Player::from_level(&portal_level, BLOCK_SIZE);
        let portal_session =
            GameSession::new(portal_level, portal_player, BLOCK_SIZE, 0, GameMode::Portal);

        assert_eq!(portal_session.mode(), GameMode::Portal);

        let horde_file = TempLevelFile::write(map);
        let horde_level =
            Level::load(horde_file.path_str()).expect("el nivel de prueba debe cargar");
        let horde_player = Player::from_level(&horde_level, BLOCK_SIZE);
        let horde_session =
            GameSession::new(horde_level, horde_player, BLOCK_SIZE, 0, GameMode::Horde);

        assert_eq!(horde_session.mode(), GameMode::Horde);
    }
}
