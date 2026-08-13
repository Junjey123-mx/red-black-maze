use crate::player::{Player, Weapon, WeaponState};
use crate::world::{Entity, Level};

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
}

impl GameSession {
    /// Crea una sesión a partir de un nivel y un jugador
    /// ya construidos.
    ///
    /// Inicia mostrando el mapa 2D, con la animación de antorcha en
    /// su cuadro inicial, y crea exactamente un Dealer por cada
    /// marcador `e` que el nivel haya descubierto, centrado en su
    /// celda de aparición.
    pub(crate) fn new(level: Level, player: Player, block_size: usize) -> Self {
        let entities = level
            .enemy_spawns()
            .iter()
            .map(|&(row, column)| Entity::dealer_at_cell(row, column, block_size))
            .collect();

        Self {
            level,
            player,
            view_mode: ViewMode::Map2D,
            torch_animation: TorchAnimationState::new(),
            weapon: Weapon::new(),
            entities,
        }
    }

    /// Entidades activas de la sesión actual (los Dealers
    /// aparecidos a partir de los marcadores `e` del nivel).
    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Avanza el comportamiento estático (temporizador de `Hit` y
    /// reevaluación de proximidad `Idle`/`Alert`) de cada entidad
    /// según la posición actual del jugador.
    ///
    /// Ninguna entidad se mueve, ataca ni persigue: esto es
    /// únicamente el temporizado/reconocimiento por distancia que
    /// `Entity::update` ya implementa de forma pura.
    pub(crate) fn update_entities(&mut self, delta_time: f32, block_size: usize) {
        let player_position = self.player.pos;

        for entity in &mut self.entities {
            entity.update(player_position, delta_time, block_size);
        }
    }

    /// Aplica el daño de un golpe de Dealer aceptado a la entidad
    /// indicada, con verificación segura de límites.
    ///
    /// Un `entity_index` fuera de rango se ignora sin entrar en
    /// pánico. La cantidad de daño y la invariante de salud/estado
    /// son responsabilidad exclusiva de `Entity::apply_damage`; este
    /// método solo coordina el acceso indexado seguro.
    pub(crate) fn damage_entity(&mut self, entity_index: usize) {
        if let Some(entity) = self.entities.get_mut(entity_index) {
            entity.apply_damage(DEALER_DAMAGE_PER_HIT);
        }
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

    /// Intenta aceptar un evento de disparo, iniciando el ciclo
    /// visual del arma.
    ///
    /// Retorna `true` si el disparo fue aceptado (útil en tareas
    /// futuras para disparar el hitscan), `false` si el arma está
    /// en enfriamiento o no está `Idle`.
    pub(crate) fn try_fire_weapon(&mut self) -> bool {
        self.weapon.try_fire()
    }
}
