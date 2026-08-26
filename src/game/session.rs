use std::collections::HashSet;

use raylib::prelude::Vector2;

use crate::player::{Player, Weapon, WeaponState};
use crate::world::{
    AmmoPickup, DistanceField, Entity, EntityDamageOutcome, EntityState, EntityStateTransition,
    Level,
};

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

/// Radio de recolección de un `AmmoPickup`, en píxeles de mundo.
///
/// ~40% del ancho de una celda (`BLOCK_SIZE = 48` en el proyecto:
/// `0.4 * 48 = 19.2`), deliberadamente pequeño para que el jugador
/// no pueda recoger munición a través de una pared ni desde un
/// pasillo paralelo.
const AMMO_PICKUP_RADIUS: f32 = 19.2;

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
    pub(crate) fn new(level: Level, player: Player, block_size: usize, hand_seed: u64) -> Self {
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

        let hand_state = HandState::new(entities.len());

        Self {
            level,
            player,
            view_mode: ViewMode::Map2D,
            torch_animation: TorchAnimationState::new(),
            weapon: Weapon::new(),
            entities,
            ammo_pickups,
            hand_state,
            hand_seed,
            hit_flash: HitFlashState::new(),
        }
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
    pub(crate) fn update_entities(
        &mut self,
        delta_time: f32,
        block_size: usize,
    ) -> Vec<EntityStateTransition> {
        let player_position = self.player.pos;

        let any_alert = self
            .entities
            .iter()
            .any(|entity| entity.state() == EntityState::Alert);

        let distance_field = any_alert.then(|| {
            let player_cell = world_to_cell(player_position, block_size);

            DistanceField::from_level(&self.level, player_cell)
        });

        let transitions = self
            .entities
            .iter_mut()
            .filter_map(|entity| {
                entity.advance_corpse_timer(delta_time);

                let pursuit_target = distance_field.as_ref().and_then(|field| {
                    let entity_cell = world_to_cell(entity.position(), block_size);

                    field
                        .step_toward_origin(entity_cell)
                        .map(|(row, column)| cell_center(row, column, block_size))
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
                self.ammo_pickups
                    .push(AmmoPickup::at_cell(row, column, block_size));
            }
        }
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
    pub(crate) fn process_dealer_attacks(&mut self, delta_time: f32, block_size: usize) -> i32 {
        let player_position = self.player.pos;

        let mut total_damage = 0;

        for entity in &mut self.entities {
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
    /// `AMMO_PICKUP_RADIUS` de la posición actual del jugador.
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

            if !ammo_pickup_in_range(player_position, pickup.position(), AMMO_PICKUP_RADIUS) {
                continue;
            }

            if self.weapon.add_reserve_ammo(AMMO_PICKUP_AMOUNT) > 0 {
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
fn ammo_pickup_in_range(player_position: Vector2, pickup_position: Vector2, radius: f32) -> bool {
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

        GameSession::new(level, player, BLOCK_SIZE, 0)
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

        GameSession::new(level, player, BLOCK_SIZE, 0)
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

        GameSession::new(level, player, BLOCK_SIZE, 0)
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

        GameSession::new(level, player, BLOCK_SIZE, 0)
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

        GameSession::new(level, player, BLOCK_SIZE, 0)
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

        GameSession::new(level, player, BLOCK_SIZE, 0)
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
        // (96 px) del pickup — muy por fuera de `AMMO_PICKUP_RADIUS`
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
        // solo reubica al jugador, nunca toca `AMMO_PICKUP_RADIUS`).
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

    #[test]
    fn ammo_pickup_in_range_matches_the_radius_boundary() {
        let player = Vector2::new(0.0, 0.0);

        assert!(ammo_pickup_in_range(
            player,
            Vector2::new(AMMO_PICKUP_RADIUS, 0.0),
            AMMO_PICKUP_RADIUS
        ));

        assert!(!ammo_pickup_in_range(
            player,
            Vector2::new(AMMO_PICKUP_RADIUS + 0.5, 0.0),
            AMMO_PICKUP_RADIUS
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

        let mut first_session = GameSession::new(level, player, BLOCK_SIZE, 0);

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

        let second_session = GameSession::new(level_again, player_again, BLOCK_SIZE, 0);

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
}
