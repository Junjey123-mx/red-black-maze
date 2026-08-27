use raylib::prelude::Vector2;

/// Estado de combate/comportamiento de una entidad.
///
/// Precedencia de estado (de mayor a menor prioridad):
///
/// ```text
/// Dead   -> terminal, ninguna otra transición puede sobrescribirlo
/// Hit    -> prioridad temporal mientras el temporizador de golpe > 0
/// Alert  -> jugador dentro de la distancia de alerta
/// Idle   -> jugador fuera de la distancia de alerta
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityState {
    Idle,
    Alert,
    Hit,
    Dead,
}

/// Identidad visual semántica de una entidad: QUÉ es, no DÓNDE
/// está su textura. La resolución de la ruta del asset pertenece a
/// la capa de rendering (`TextureManager`), no a este módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntitySprite {
    Dealer,
    King,
}

/// Tipo de enemigo dentro del MISMO sistema de entidades (Bloque 3,
/// Commit 21).
///
/// No es un sistema paralelo: `Entity` sigue siendo una sola struct
/// con un solo pipeline de estado/daño/render/cleanup. El `kind` solo
/// selecciona los VALORES de dominio del enemigo (vida, radio de
/// impacto, y — en commits posteriores — velocidad de persecución,
/// rango/cooldown/daño de ataque) y su conjunto de sprites. The King
/// es "The Dealer ascendido a jefe", no una criatura nueva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnemyKind {
    Dealer,
    King,
}

impl EnemyKind {
    /// Distancia de alerta del tipo de enemigo, en celdas de mapa.
    ///
    /// The King usa un radio mayor (Bloque 3, Commit 22): es un jefe
    /// de arena, debe engancharse y venir a por el jugador desde más
    /// lejos que un Dealer normal.
    fn alert_distance_cells(self) -> f32 {
        match self {
            EnemyKind::Dealer => DEALER_ALERT_DISTANCE_CELLS,
            EnemyKind::King => KING_ALERT_DISTANCE_CELLS,
        }
    }

    /// Velocidad de persecución del tipo de enemigo, en px/s.
    ///
    /// The King es ligeramente más rápido que un Dealer (Bloque 3,
    /// Commit 22) para sentirse más persistente y amenazante, pero
    /// sigue dentro del rango conservador que deja al jugador (150
    /// px/s) capaz de maniobrar y distanciarse.
    fn pursuit_speed(self) -> f32 {
        match self {
            EnemyKind::Dealer => DEALER_PURSUIT_SPEED,
            EnemyKind::King => KING_PURSUIT_SPEED,
        }
    }

    /// Enfriamiento entre ataques aceptados del tipo de enemigo, en
    /// segundos (Bloque 3, Commit 23).
    ///
    /// The King golpea más fuerte pero más despacio: 1.5 s frente a
    /// los 0.9 s del Dealer. Mismo temporizado por-entidad, mismo
    /// gating; solo cambia el valor.
    fn attack_cooldown(self) -> f32 {
        match self {
            EnemyKind::Dealer => DEALER_ATTACK_COOLDOWN,
            EnemyKind::King => KING_ATTACK_COOLDOWN,
        }
    }
}

/// Transición real de `EntityState` observada durante una llamada a
/// `Entity::update`. Deliberadamente ajena a audio/presentación: es
/// dominio puro (QUÉ cambió), no CÓMO se comunica al jugador.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntityStateTransition {
    pub(crate) from: EntityState,
    pub(crate) to: EntityState,
}

/// Resultado semántico de un intento de daño sobre una entidad, para
/// que quien orquesta el combate (`GameSession`/`App`) distinga un
/// golpe real de un evento sin efecto, sin inferirlo indirectamente
/// de `EntityState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntityDamageOutcome {
    /// La entidad ya estaba muerta, o `amount` no era positivo: no
    /// se aplicó ningún daño.
    None,

    /// Daño no letal aplicado: la entidad sobrevive y entra/permanece
    /// en `Hit`.
    Hit,

    /// Daño letal aplicado: la entidad acaba de morir.
    Killed,
}

/// Vida máxima inicial de un Dealer.
const DEALER_MAX_HEALTH: i32 = 100;

/// Vida máxima inicial de The King (Bloque 3, Commit 21).
///
/// Con `WeaponTier::Standard.damage() = 50` son exactamente 20
/// impactos Standard; con `WeaponTier::RoyalFlush.damage() = 100`,
/// exactamente 10. El resultado emerge de vida vs daño del arma, sin
/// ninguna condición especial por tipo de enemigo.
const KING_MAX_HEALTH: i32 = 1000;

/// Radio de impacto del Dealer, en unidades de mundo (píxeles).
///
/// Es geometría de mundo, no presentación: se eligió
/// deliberadamente mucho menor que `BLOCK_SIZE` (48 unidades) para
/// que el círculo de impacto quede claramente contenido dentro de
/// la celda de aparición.
const DEALER_HIT_RADIUS: f32 = 12.0;

/// Radio de impacto de The King (Bloque 3, Commit 21): algo mayor
/// que el del Dealer — el jefe es un blanco más grande — pero sigue
/// contenido dentro de su celda de aparición (`< BLOCK_SIZE / 2`).
const KING_HIT_RADIUS: f32 = 18.0;

/// Duración visual del estado `Hit` tras un golpe no letal.
///
/// 0.15s es claramente observable (varios cuadros a 60 FPS) sin
/// bloquear la reevaluación de proximidad por mucho tiempo.
const DEALER_HIT_DURATION_SECONDS: f32 = 0.15;

/// Distancia de alerta, expresada en celdas de mapa. El jugador
/// dentro de esta distancia hace que un Dealer con vida y sin golpe
/// activo pase a `Alert`; fuera de ella vuelve a `Idle`.
const DEALER_ALERT_DISTANCE_CELLS: f32 = 4.0;

/// Velocidad de persecución del Dealer, en píxeles de mundo por
/// segundo.
///
/// El jugador se mueve a 150 px/s (`MOVE_SPEED`,
/// `input/controller.rs`). Este valor es exactamente la mitad
/// (50%), dentro del rango deliberadamente conservador (40%-65%) que
/// mantiene al jugador capaz de distanciarse/maniobrar frente a un
/// Dealer en persecución: la persecución crea presión, no una
/// carrera que el jugador no pueda ganar.
const DEALER_PURSUIT_SPEED: f32 = 75.0;

/// Velocidad de persecución de The King, en px/s (Bloque 3, Commit
/// 22). ~57% de la velocidad del jugador (150 px/s): más presión que
/// un Dealer (50%), todavía dentro del rango que deja escapar/
/// maniobrar.
const KING_PURSUIT_SPEED: f32 = 85.0;

/// Distancia de alerta de The King, en celdas de mapa (Bloque 3,
/// Commit 22). Mayor que la del Dealer (4) — el jefe persigue desde
/// el otro extremo de la arena.
const KING_ALERT_DISTANCE_CELLS: f32 = 6.0;

/// Distancia mínima, expresada en celdas de mapa, a la que el Dealer
/// deja de avanzar hacia su siguiente punto de ruta. Evita que la
/// entidad oscile/tiemble al llegar exactamente al centro de una
/// celda; en la práctica el propio `DistanceField` ya detiene la
/// persecución al entrar a la celda del jugador (distancia 0), así
/// que este umbral solo protege el último tramo de cada paso
/// intermedio.
const DEALER_PURSUIT_STOP_DISTANCE_CELLS: f32 = 0.05;

/// Rango de ataque cuerpo a cuerpo del Dealer, expresado en celdas
/// de mapa (Tarea 45), igual que `DEALER_ALERT_DISTANCE_CELLS`: se
/// multiplica por `block_size` en el momento de comprobar el rango,
/// nunca se fija en píxeles absolutos aquí.
///
/// Con `BLOCK_SIZE = 48` (valor real del proyecto) esto es
/// `0.75 * 48 = 36.0` píxeles — deliberadamente MENOR que una celda
/// completa: la geometría normal del laberinto (las paredes ocupan
/// la celda completa, y el jugador/Dealer permanecen dentro de la
/// unión de celdas transitables 4-conectadas por la persecución
/// existente) ya impide que un Dealer golpee a través de una pared
/// completa, sin necesitar ningún chequeo de línea de visión nuevo:
/// un rango de 36px no alcanza a cruzar una celda de 48px de pared
/// en ninguna configuración recta.
///
/// `pub(crate)` (en vez de privado) para que `GameSession` pueda
/// derivar exactamente el mismo umbral al decidir cuándo el fallback
/// de persecución "misma celda, sin siguiente paso de ruta" (corner
/// dead zone) debe seguir empujando al Dealer hacia la posición
/// exacta del jugador, y cuándo debe dejar de hacerlo — nunca una
/// segunda copia del valor.
pub(crate) const DEALER_ATTACK_RANGE_CELLS: f32 = 0.75;

/// Cooldown entre ataques aceptados de un mismo Dealer, en segundos.
const DEALER_ATTACK_COOLDOWN: f32 = 0.9;

/// Cooldown entre ataques aceptados de The King, en segundos (Bloque
/// 3, Commit 23): un ataque pesado y lento — valor fijo, no un rango.
const KING_ATTACK_COOLDOWN: f32 = 1.5;

/// Tarea "Dealer Hands": segundos de tiempo de PARTIDA (nunca reloj
/// absoluto — se congela automáticamente durante `Paused`, exactamente
/// como el resto de temporizadores de esta entidad) que un cadáver
/// permanece en la colección activa de entidades antes de ser
/// elegible para eliminación definitiva. Única fuente de verdad de
/// este valor en todo el proyecto.
pub(crate) const CORPSE_DESPAWN_SECONDS: f32 = 15.0;

/// Entidad de dominio del mundo/juego: posición, vida, estado de
/// comportamiento, identidad visual y radio de impacto.
///
/// No renderiza píxeles, no conoce `Framebuffer`/`TextureAsset`/
/// `TextureManager`, no hace raycasting ni lee input. Es estado
/// puro de partida.
pub(crate) struct Entity {
    position: Vector2,
    health: i32,
    state: EntityState,
    sprite: EntitySprite,
    kind: EnemyKind,
    hit_radius: f32,
    hit_time_remaining: f32,

    /// Tiempo restante (segundos) antes de que este Dealer, EN
    /// PARTICULAR, pueda volver a atacar. Cooldown por-entidad — no
    /// existe ningún timer global compartido entre Dealers.
    attack_cooldown_remaining: f32,

    /// Tiempo de PARTIDA transcurrido desde que esta entidad murió
    /// (`0.0` mientras está viva). Solo avanza vía
    /// `advance_corpse_timer`, llamado exclusivamente para entidades
    /// `Dead` — nunca es un reloj de pared independiente del bucle de
    /// actualización, así que `Paused` lo congela automáticamente
    /// (esa llamada solo ocurre dentro de `update_playing`, igual que
    /// el resto de temporizadores de partida).
    corpse_elapsed: f32,
}

impl Entity {
    /// Crea un Dealer centrado exactamente en la celda `(row,
    /// column)`, con vida máxima, estado `Idle` y su radio de
    /// impacto congelado.
    pub(crate) fn dealer_at_cell(row: usize, column: usize, block_size: usize) -> Self {
        let half_block = block_size as f32 / 2.0;

        let x = column as f32 * block_size as f32 + half_block;

        let y = row as f32 * block_size as f32 + half_block;

        Self {
            position: Vector2::new(x, y),
            health: DEALER_MAX_HEALTH,
            state: EntityState::Idle,
            sprite: EntitySprite::Dealer,
            kind: EnemyKind::Dealer,
            hit_radius: DEALER_HIT_RADIUS,
            hit_time_remaining: 0.0,
            attack_cooldown_remaining: 0.0,
            corpse_elapsed: 0.0,
        }
    }

    /// Crea a The King centrado exactamente en la celda `(row,
    /// column)`, con `KING_MAX_HEALTH`, estado `Idle` y su radio de
    /// impacto de jefe (Bloque 3, Commit 21).
    ///
    /// Misma struct, mismo pipeline de estado/daño/render/cleanup que
    /// `dealer_at_cell`; solo cambian los valores y el `kind`/`sprite`.
    ///
    /// `GameSession` lo invoca al spawnear la Final Hand (Commit 24).
    #[allow(dead_code)]
    pub(crate) fn king_at_cell(row: usize, column: usize, block_size: usize) -> Self {
        let half_block = block_size as f32 / 2.0;

        let x = column as f32 * block_size as f32 + half_block;

        let y = row as f32 * block_size as f32 + half_block;

        Self {
            position: Vector2::new(x, y),
            health: KING_MAX_HEALTH,
            state: EntityState::Idle,
            sprite: EntitySprite::King,
            kind: EnemyKind::King,
            hit_radius: KING_HIT_RADIUS,
            hit_time_remaining: 0.0,
            attack_cooldown_remaining: 0.0,
            corpse_elapsed: 0.0,
        }
    }

    /// Tipo de enemigo (Dealer o King). Fuente de verdad de los
    /// valores de dominio específicos del enemigo. Lo consumen
    /// `GameSession` (daño de ataque por tipo, spawn de la Final Hand,
    /// barra de vida del jefe) y el audio del jefe.
    pub(crate) fn kind(&self) -> EnemyKind {
        self.kind
    }

    /// Posición actual en el mundo, en píxeles.
    pub(crate) fn position(&self) -> Vector2 {
        self.position
    }

    /// Puntos de vida restantes.
    #[allow(dead_code)]
    pub(crate) fn health(&self) -> i32 {
        self.health
    }

    /// Estado de comportamiento actual.
    pub(crate) fn state(&self) -> EntityState {
        self.state
    }

    /// Identidad visual de la entidad.
    pub(crate) fn sprite(&self) -> EntitySprite {
        self.sprite
    }

    /// Radio de impacto usado por el hitscan, en píxeles de mundo.
    pub(crate) fn hit_radius(&self) -> f32 {
        self.hit_radius
    }

    /// Indica si la entidad está muerta.
    pub(crate) fn is_dead(&self) -> bool {
        self.state == EntityState::Dead
    }

    /// Avanza el temporizador de cadáver. No-op absoluto mientras la
    /// entidad sigue viva (`corpse_elapsed` nunca avanza fuera de
    /// `Dead`, así que revivir conceptualmente — cosa que hoy no
    /// ocurre, `Dead` es terminal — nunca heredaría tiempo viejo), y
    /// también no-op para `delta_time` no finito/no positivo, mismo
    /// patrón que el resto de temporizadores de esta entidad.
    ///
    /// Debe llamarse EXCLUSIVAMENTE desde dentro del update jugable
    /// (`GameSession::update_entities`, a su vez solo invocado por
    /// `App::update_playing`) para que `Paused` lo congele
    /// automáticamente sin ningún caso especial — mismo patrón ya
    /// establecido para cooldowns/persecución/flash de daño.
    pub(crate) fn advance_corpse_timer(&mut self, delta_time: f32) {
        if self.state != EntityState::Dead {
            return;
        }

        if delta_time.is_finite() && delta_time > 0.0 {
            self.corpse_elapsed += delta_time;
        }
    }

    /// `true` una vez que un cadáver ya cumplió
    /// `CORPSE_DESPAWN_SECONDS` de tiempo de partida visible. `false`
    /// para cualquier entidad viva, sin importar cuánto tiempo lleve
    /// existiendo (`corpse_elapsed` permanece en `0.0` mientras vive).
    pub(crate) fn should_despawn(&self) -> bool {
        self.state == EntityState::Dead && self.corpse_elapsed >= CORPSE_DESPAWN_SECONDS
    }

    /// Aplica daño controlado a la entidad y reporta el resultado
    /// semántico del intento (`EntityDamageOutcome`), para que quien
    /// orquesta el combate pueda distinguir un golpe real de un
    /// evento sin efecto sin inferirlo de `EntityState`.
    ///
    /// Una entidad ya `Dead` ignora cualquier daño adicional; un
    /// `amount` no positivo también se ignora (`None` en ambos
    /// casos). En caso contrario la vida se recorta con un piso de
    /// `0` (nunca queda negativa). Si la vida llega a `0` la entidad
    /// pasa a `Dead` de forma terminal (`Killed`); en caso contrario
    /// pasa a `Hit` y (re)inicia su temporizador de golpe (`Hit`).
    pub(crate) fn apply_damage(&mut self, amount: i32) -> EntityDamageOutcome {
        if self.state == EntityState::Dead {
            return EntityDamageOutcome::None;
        }

        if amount <= 0 {
            return EntityDamageOutcome::None;
        }

        self.health = (self.health - amount).max(0);

        if self.health == 0 {
            self.state = EntityState::Dead;
            self.hit_time_remaining = 0.0;

            EntityDamageOutcome::Killed
        } else {
            self.state = EntityState::Hit;
            self.hit_time_remaining = DEALER_HIT_DURATION_SECONDS;

            /*
             * Tarea 45: cualquier ataque ofensivo pendiente/listo se
             * interrumpe reiniciando el cooldown al COMPLETO al
             * entrar en `Hit`, en vez de dejarlo como estaba. Sin
             * esto, un Dealer golpeado justo cuando su cooldown ya
             * había expirado podría recuperarse de `Hit` y atacar
             * de inmediato en el mismo instante; con el reinicio,
             * el jugador gana un respiro real de
             * `DEALER_ATTACK_COOLDOWN` completo tras cada golpe
             * aceptado contra ese Dealer.
             */
            self.attack_cooldown_remaining = self.kind.attack_cooldown();

            EntityDamageOutcome::Hit
        }
    }

    /// Avanza el comportamiento de la entidad: decrementa el
    /// temporizador de `Hit` y, cuando ya no hay golpe activo,
    /// reevalúa `Idle`/`Alert` según la distancia al jugador, y —
    /// únicamente si el estado resultante es `Alert` — avanza hacia
    /// `pursuit_target`. Reporta `Some(EntityStateTransition)`
    /// únicamente cuando el estado ACTUALMENTE cambió durante esta
    /// llamada; `None` si permanece igual (incluyendo `Dead` y `Hit`
    /// con temporizador aún activo).
    ///
    /// `Dead` es terminal y esta función retorna inmediatamente sin
    /// alterar nada (nunca se mueve). Mientras `hit_time_remaining`
    /// siga siendo positivo, `Hit` NO es sobrescrito por la
    /// reevaluación de proximidad, ni siquiera dentro de la misma
    /// llamada, y la entidad NO se mueve durante esa reacción.
    ///
    /// `pursuit_target`, si existe, es la posición de mundo (centro
    /// de la siguiente celda transitable en la ruta hacia el
    /// jugador) ya resuelta por el llamador (`GameSession`, usando
    /// `world::DistanceField` sobre el `Level` real); esta función
    /// NO conoce `Level` ni calcula ninguna ruta, solo avanza hacia
    /// el punto ya decidido. Esto preserva la pureza de `Entity`
    /// (sin dependencia de mundo/mapa) exactamente como antes.
    pub(crate) fn update(
        &mut self,
        player_position: Vector2,
        delta_time: f32,
        block_size: usize,
        pursuit_target: Option<Vector2>,
    ) -> Option<EntityStateTransition> {
        if self.state == EntityState::Dead {
            return None;
        }

        let previous_state = self.state;

        if self.state == EntityState::Hit {
            if delta_time.is_finite() && delta_time > 0.0 {
                self.hit_time_remaining = (self.hit_time_remaining - delta_time).max(0.0);
            }

            if self.hit_time_remaining > 0.0 {
                return None;
            }
        }

        let alert_distance = block_size as f32 * self.kind.alert_distance_cells();

        let dx = self.position.x - player_position.x;

        let dy = self.position.y - player_position.y;

        let distance_squared = dx * dx + dy * dy;

        self.state = if distance_squared <= alert_distance * alert_distance {
            EntityState::Alert
        } else {
            EntityState::Idle
        };

        if self.state == EntityState::Alert {
            self.pursue(pursuit_target, delta_time, block_size);
        }

        if self.state == previous_state {
            None
        } else {
            Some(EntityStateTransition {
                from: previous_state,
                to: self.state,
            })
        }
    }

    /// Avanza la posición hacia `target` a `kind.pursuit_speed()`
    /// px/s, respetando `delta_time` (movimiento independiente del
    /// framerate, igual que `process_events` del jugador).
    ///
    /// No-op seguro si `target` es `None` (sin ruta disponible, por
    /// ejemplo el jugador ya está en la misma celda), o si
    /// `delta_time` no es finito/positivo. Nunca sobrepasa `target`:
    /// si el paso de este cuadro alcanzaría o superaría la
    /// distancia restante, la posición se ajusta EXACTAMENTE a
    /// `target` en vez de overshoot, evitando oscilación.
    ///
    /// Como `target` es siempre el centro de una celda transitable
    /// 4-conectada a la celda actual (resuelta por
    /// `DistanceField::step_toward_origin`), el segmento recto entre
    /// ambos centros permanece dentro de la unión de esas dos celdas
    /// abiertas: nunca cruza una pared ni corta una esquina
    /// bloqueada.
    fn pursue(&mut self, target: Option<Vector2>, delta_time: f32, block_size: usize) {
        let Some(target) = target else {
            return;
        };

        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }

        let dx = target.x - self.position.x;

        let dy = target.y - self.position.y;

        let distance = dx.hypot(dy);

        let stop_distance = block_size as f32 * DEALER_PURSUIT_STOP_DISTANCE_CELLS;

        if distance <= stop_distance {
            return;
        }

        let step = self.kind.pursuit_speed() * delta_time;

        if step >= distance {
            self.position = target;
        } else {
            self.position.x += dx / distance * step;

            self.position.y += dy / distance * step;
        }
    }

    /// Intenta un ataque cuerpo a cuerpo contra el jugador este
    /// cuadro (Tarea 45): decrementa SIEMPRE el cooldown ofensivo
    /// según `delta_time` (independiente del framerate, igual que
    /// el resto de temporizadores de la entidad — nunca reloj
    /// absoluto), y retorna `true` únicamente si el ataque fue
    /// ACEPTADO este cuadro.
    ///
    /// Un ataque se acepta solo si se cumplen las TRES condiciones a
    /// la vez: `state == Alert` (nunca `Idle`/`Hit`/`Dead`),
    /// distancia horizontal al jugador `<= DEALER_ATTACK_RANGE`, y
    /// el cooldown ya llegó a `0.0`. Al aceptarse, el cooldown se
    /// recarga al completo (`DEALER_ATTACK_COOLDOWN`) — nunca se
    /// acepta un segundo ataque del mismo Dealer en el mismo cuadro
    /// ni en cuadros inmediatamente siguientes.
    ///
    /// Esta función NO conoce `Weapon`/`AudioManager`/cuánto daño
    /// causa un ataque: solo reporta SI ocurrió. `GameSession` (la
    /// única capa con acceso simultáneo a `Player` y a todos los
    /// Dealers) decide cuánta vida restar y cómo agregar el
    /// feedback de varios Dealers en el mismo cuadro.
    pub(crate) fn attempt_attack(
        &mut self,
        player_position: Vector2,
        delta_time: f32,
        block_size: usize,
    ) -> bool {
        if delta_time.is_finite() && delta_time > 0.0 {
            self.attack_cooldown_remaining = (self.attack_cooldown_remaining - delta_time).max(0.0);
        }

        if self.state != EntityState::Alert {
            return false;
        }

        if self.attack_cooldown_remaining > 0.0 {
            return false;
        }

        let attack_range = block_size as f32 * DEALER_ATTACK_RANGE_CELLS;

        let dx = self.position.x - player_position.x;

        let dy = self.position.y - player_position.y;

        if dx * dx + dy * dy > attack_range * attack_range {
            return false;
        }

        self.attack_cooldown_remaining = self.kind.attack_cooldown();

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dealer_starts_centered_in_its_cell() {
        let entity = Entity::dealer_at_cell(2, 3, 48);

        let position = entity.position();

        assert!((position.x - (3.0 * 48.0 + 24.0)).abs() < f32::EPSILON);
        assert!((position.y - (2.0 * 48.0 + 24.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn dealer_starts_with_full_health() {
        let entity = Entity::dealer_at_cell(0, 0, 48);

        assert_eq!(entity.health(), DEALER_MAX_HEALTH);
    }

    #[test]
    fn dealer_starts_idle() {
        let entity = Entity::dealer_at_cell(0, 0, 48);

        assert_eq!(entity.state(), EntityState::Idle);
        assert!(!entity.is_dead());
    }

    #[test]
    fn dealer_hit_radius_is_positive_and_smaller_than_block_size() {
        let entity = Entity::dealer_at_cell(0, 0, 48);

        assert!(entity.hit_radius() > 0.0);
        assert!(entity.hit_radius() < 48.0);
    }

    #[test]
    fn non_lethal_damage_reduces_health_and_enters_hit() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);

        assert_eq!(entity.health(), 50);
        assert_eq!(entity.state(), EntityState::Hit);
    }

    #[test]
    fn lethal_damage_zeroes_health_and_enters_dead() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert_eq!(entity.health(), 0);
        assert_eq!(entity.state(), EntityState::Dead);
    }

    #[test]
    fn health_never_goes_below_zero() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);
        entity.apply_damage(50);

        assert_eq!(entity.health(), 0);
        assert_eq!(entity.state(), EntityState::Dead);
    }

    #[test]
    fn damage_to_dead_entity_does_nothing() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert!(entity.is_dead());

        entity.apply_damage(50);

        assert_eq!(entity.health(), 0);
        assert_eq!(entity.state(), EntityState::Dead);
    }

    #[test]
    fn nearby_player_makes_idle_dealer_alert() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.016, 48, None);

        assert_eq!(entity.state(), EntityState::Alert);
    }

    #[test]
    fn far_player_keeps_alert_dealer_idle() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.016, 48, None);

        assert_eq!(entity.state(), EntityState::Alert);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        entity.update(far_player, 0.016, 48, None);

        assert_eq!(entity.state(), EntityState::Idle);
    }

    #[test]
    fn hit_state_survives_while_timer_is_active() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);

        assert_eq!(entity.state(), EntityState::Hit);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        // Un delta pequeño no debe agotar el temporizador de 0.15s.
        entity.update(near_player, 0.016, 48, None);

        assert_eq!(entity.state(), EntityState::Hit);
    }

    #[test]
    fn hit_recovers_to_awareness_after_timer_expires() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);

        assert_eq!(entity.state(), EntityState::Hit);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        // Delta mayor que la duración de Hit (0.15s): debe expirar
        // y reevaluar la proximidad en la misma llamada.
        entity.update(far_player, 0.20, 48, None);

        assert_eq!(entity.state(), EntityState::Idle);
    }

    #[test]
    fn dead_remains_dead_across_updates() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert!(entity.is_dead());

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.5, 48, None);

        assert_eq!(entity.state(), EntityState::Dead);
        assert!(entity.is_dead());
    }

    // --- Bloque 3, Commit 21: The King como entidad. ---

    #[test]
    fn king_starts_centered_with_boss_identity_and_one_thousand_health() {
        let king = Entity::king_at_cell(2, 3, 48);

        assert!((king.position().x - (3.0 * 48.0 + 24.0)).abs() < f32::EPSILON);
        assert!((king.position().y - (2.0 * 48.0 + 24.0)).abs() < f32::EPSILON);
        assert_eq!(king.health(), 1000);
        assert_eq!(king.state(), EntityState::Idle);
        assert_eq!(king.sprite(), EntitySprite::King);
        assert_eq!(king.kind(), EnemyKind::King);
        assert!(!king.is_dead());
    }

    #[test]
    fn king_hit_radius_is_positive_and_contained_in_its_cell() {
        let king = Entity::king_at_cell(0, 0, 48);

        assert!(king.hit_radius() > 0.0);
        assert!(king.hit_radius() < 48.0 / 2.0);
        // El jefe es un blanco mayor que un Dealer.
        assert!(king.hit_radius() > Entity::dealer_at_cell(0, 0, 48).hit_radius());
    }

    #[test]
    fn king_takes_twenty_standard_hits_to_die() {
        let mut king = Entity::king_at_cell(0, 0, 48);

        for shot in 1..=19 {
            assert_eq!(king.apply_damage(50), EntityDamageOutcome::Hit);
            assert_eq!(king.health(), 1000 - shot * 50);
            assert!(!king.is_dead());
        }

        assert_eq!(king.apply_damage(50), EntityDamageOutcome::Killed);
        assert_eq!(king.health(), 0);
        assert_eq!(king.state(), EntityState::Dead);
    }

    #[test]
    fn king_takes_ten_royal_flush_hits_to_die() {
        let mut king = Entity::king_at_cell(0, 0, 48);

        for shot in 1..=9 {
            assert_eq!(king.apply_damage(100), EntityDamageOutcome::Hit);
            assert_eq!(king.health(), 1000 - shot * 100);
        }

        assert_eq!(king.apply_damage(100), EntityDamageOutcome::Killed);
        assert!(king.is_dead());
    }

    #[test]
    fn king_first_hit_leaves_it_at_950_and_in_hit_state() {
        let mut king = Entity::king_at_cell(0, 0, 48);

        king.apply_damage(50);

        assert_eq!(king.health(), 950);
        assert_eq!(king.state(), EntityState::Hit);
    }

    #[test]
    fn dead_king_stays_dead_and_despawns_like_any_corpse() {
        let mut king = Entity::king_at_cell(0, 0, 48);

        for _ in 0..20 {
            king.apply_damage(50);
        }
        assert!(king.is_dead());

        king.advance_corpse_timer(CORPSE_DESPAWN_SECONDS);
        assert!(king.should_despawn());
    }

    #[test]
    fn adding_the_king_does_not_change_dealer_health() {
        assert_eq!(Entity::dealer_at_cell(0, 0, 48).health(), 100);
        assert_eq!(Entity::dealer_at_cell(0, 0, 48).kind(), EnemyKind::Dealer);
    }

    // --- Bloque 3, Commit 22: persecución de The King. ---

    #[test]
    fn king_alerts_from_farther_away_than_a_dealer_would() {
        let mut king = Entity::king_at_cell(0, 0, 48);
        let mut dealer = Entity::dealer_at_cell(0, 0, 48);

        // 5 celdas: dentro del radio del King (6), fuera del Dealer (4).
        let player = Vector2::new(king.position().x + 5.0 * 48.0, king.position().y);

        king.update(player, 0.016, 48, None);
        dealer.update(player, 0.016, 48, None);

        assert_eq!(king.state(), EntityState::Alert);
        assert_eq!(dealer.state(), EntityState::Idle);
    }

    #[test]
    fn king_pursues_faster_than_a_dealer_over_the_same_step() {
        let mut king = Entity::king_at_cell(0, 0, 48);
        let mut dealer = Entity::dealer_at_cell(0, 0, 48);

        let player = Vector2::new(king.position().x + 10.0, king.position().y);
        king.update(player, 0.016, 48, None);
        dealer.update(player, 0.016, 48, None);
        assert_eq!(king.state(), EntityState::Alert);
        assert_eq!(dealer.state(), EntityState::Alert);

        let start = king.position();
        let target = Vector2::new(start.x + 480.0, start.y);

        king.update(player, 0.1, 48, Some(target));
        dealer.update(player, 0.1, 48, Some(target));

        let king_moved = king.position().x - start.x;
        let dealer_moved = dealer.position().x - start.x;

        assert!(king_moved > dealer_moved);
        // 85 vs 75 px/s -> exactamente ese ratio a delta idéntico.
        assert!((king_moved / dealer_moved - 85.0 / 75.0).abs() < 1e-3);
    }

    #[test]
    fn king_still_snaps_to_its_target_without_overshooting() {
        let mut king = Entity::king_at_cell(0, 0, 48);

        let player = Vector2::new(king.position().x + 10.0, king.position().y);
        king.update(player, 0.016, 48, None);

        let start = king.position();
        // 3px de objetivo: por debajo del paso de este cuadro (8.5px a
        // 85px/s en 0.1s), que lo sobrepasaría sin el recorte.
        let target = Vector2::new(start.x + 3.0, start.y);

        king.update(player, 0.1, 48, Some(target));

        assert_eq!(king.position().x, target.x);
        assert_eq!(king.position().y, target.y);
    }

    #[test]
    fn dead_or_hit_king_never_moves_even_with_a_pursuit_target() {
        let mut king = Entity::king_at_cell(0, 0, 48);
        let player = Vector2::new(king.position().x + 10.0, king.position().y);
        let target = Vector2::new(king.position().x + 48.0, king.position().y);

        king.apply_damage(50); // -> Hit
        let after_hit = king.position();
        king.update(player, 0.016, 48, Some(target));
        assert_eq!(king.position().x, after_hit.x);

        for _ in 0..20 {
            king.apply_damage(50);
        }
        assert!(king.is_dead());
        let after_dead = king.position();
        king.update(player, 0.5, 48, Some(target));
        assert_eq!(king.position().x, after_dead.x);
        assert_eq!(king.position().y, after_dead.y);
    }

    #[test]
    fn dealer_pursuit_is_unchanged_by_the_king_parameters() {
        let mut dealer = Entity::dealer_at_cell(0, 0, 48);
        let player = Vector2::new(dealer.position().x + 10.0, dealer.position().y);
        dealer.update(player, 0.016, 48, None);

        let start = dealer.position();
        let target = Vector2::new(start.x + 480.0, start.y);
        dealer.update(player, 0.1, 48, Some(target));

        // 75 px/s * 0.1 s = 7.5 px, exactamente como antes del Bloque 3.
        assert!(((dealer.position().x - start.x) - 7.5).abs() < 1e-3);
    }

    // --- Bloque 3, Commit 23: ataque pesado de The King. ---

    /// Pone a The King en `Alert`, dentro de rango, listo para atacar.
    fn alert_king_in_range() -> (Entity, Vector2) {
        let mut king = Entity::king_at_cell(0, 0, BLOCK_SIZE);
        let player = Vector2::new(king.position().x + 10.0, king.position().y);
        king.update(player, 0.016, BLOCK_SIZE, None);
        assert_eq!(king.state(), EntityState::Alert);
        (king, player)
    }

    #[test]
    fn king_attack_respects_its_slower_cooldown() {
        let (mut king, player) = alert_king_in_range();

        assert!(king.attempt_attack(player, 0.016, BLOCK_SIZE));

        // A 0.9 s (el cooldown de un Dealer) The King TODAVÍA no puede
        // volver a atacar.
        assert!(!king.attempt_attack(player, 0.9, BLOCK_SIZE));

        // Justo antes de 1.5 s (0.9 + 0.58 = 1.48): sigue bloqueado.
        assert!(!king.attempt_attack(player, 0.58, BLOCK_SIZE));

        // Pasa de 1.5 s: ya puede.
        assert!(king.attempt_attack(player, 0.05, BLOCK_SIZE));
    }

    #[test]
    fn king_attack_never_lands_every_frame() {
        let (mut king, player) = alert_king_in_range();

        let mut accepted = 0;
        // ~1.6 s simulados en cuadros de 0.016 s: con cooldown 1.5 s,
        // como mucho 2 ataques, jamás 100.
        for _ in 0..100 {
            if king.attempt_attack(player, 0.016, BLOCK_SIZE) {
                accepted += 1;
            }
        }
        assert!((1..=2).contains(&accepted));
    }

    #[test]
    fn a_hit_king_gets_the_full_king_cooldown_before_it_can_attack_again() {
        let (mut king, player) = alert_king_in_range();

        king.apply_damage(50); // -> Hit, recarga cooldown al valor de King
        king.update(player, 0.20, BLOCK_SIZE, None); // expira Hit -> Alert
        assert_eq!(king.state(), EntityState::Alert);

        // 0.9 s no bastan (cooldown de King es 1.5 s).
        assert!(!king.attempt_attack(player, 0.9, BLOCK_SIZE));
        assert!(king.attempt_attack(player, 0.6, BLOCK_SIZE));
    }

    #[test]
    fn dealer_attack_cooldown_is_unchanged_by_the_king_values() {
        let (mut dealer, player) = alert_dealer_in_range();

        assert!(dealer.attempt_attack(player, 0.016, BLOCK_SIZE));
        assert!(!dealer.attempt_attack(player, 0.9 - 0.016, BLOCK_SIZE));
        // A 0.9 s exactos vuelve a estar disponible: comportamiento
        // idéntico al de antes del Bloque 3.
        assert!(dealer.attempt_attack(player, 0.016, BLOCK_SIZE));
    }

    // --- EntityDamageOutcome ---

    #[test]
    fn non_lethal_damage_returns_hit_outcome() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        assert_eq!(entity.apply_damage(50), EntityDamageOutcome::Hit);
        assert_eq!(entity.health(), 50);
    }

    #[test]
    fn lethal_damage_returns_killed_outcome() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);

        assert_eq!(entity.apply_damage(50), EntityDamageOutcome::Killed);
        assert_eq!(entity.health(), 0);
    }

    #[test]
    fn damage_to_dead_entity_returns_none_outcome() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert!(entity.is_dead());

        assert_eq!(entity.apply_damage(50), EntityDamageOutcome::None);
    }

    #[test]
    fn non_positive_damage_returns_none_outcome() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        assert_eq!(entity.apply_damage(0), EntityDamageOutcome::None);
        assert_eq!(entity.apply_damage(-10), EntityDamageOutcome::None);
        assert_eq!(entity.health(), DEALER_MAX_HEALTH);
    }

    #[test]
    fn health_semantics_follow_100_50_0() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        assert_eq!(entity.health(), 100);

        entity.apply_damage(50);
        assert_eq!(entity.health(), 50);

        entity.apply_damage(50);
        assert_eq!(entity.health(), 0);
    }

    // --- EntityStateTransition ---

    #[test]
    fn idle_to_alert_reports_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        let transition = entity.update(near_player, 0.016, 48, None);

        assert_eq!(
            transition,
            Some(EntityStateTransition {
                from: EntityState::Idle,
                to: EntityState::Alert,
            })
        );
    }

    #[test]
    fn alert_to_idle_reports_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.016, 48, None);
        assert_eq!(entity.state(), EntityState::Alert);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        let transition = entity.update(far_player, 0.016, 48, None);

        assert_eq!(
            transition,
            Some(EntityStateTransition {
                from: EntityState::Alert,
                to: EntityState::Idle,
            })
        );
    }

    #[test]
    fn idle_to_idle_reports_no_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        let transition = entity.update(far_player, 0.016, 48, None);

        assert_eq!(transition, None);
        assert_eq!(entity.state(), EntityState::Idle);
    }

    #[test]
    fn hit_to_idle_after_expiry_reports_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        assert_eq!(entity.state(), EntityState::Hit);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        let transition = entity.update(far_player, 0.20, 48, None);

        assert_eq!(
            transition,
            Some(EntityStateTransition {
                from: EntityState::Hit,
                to: EntityState::Idle,
            })
        );
    }

    #[test]
    fn hit_to_alert_after_expiry_reports_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        assert_eq!(entity.state(), EntityState::Hit);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        let transition = entity.update(near_player, 0.20, 48, None);

        assert_eq!(
            transition,
            Some(EntityStateTransition {
                from: EntityState::Hit,
                to: EntityState::Alert,
            })
        );
    }

    #[test]
    fn hit_still_active_reports_no_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        let transition = entity.update(near_player, 0.016, 48, None);

        assert_eq!(transition, None);
        assert_eq!(entity.state(), EntityState::Hit);
    }

    #[test]
    fn dead_update_reports_no_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert!(entity.is_dead());

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        let transition = entity.update(near_player, 0.5, 48, None);

        assert_eq!(transition, None);
        assert_eq!(entity.state(), EntityState::Dead);
    }

    // --- Persecución (Alert pursuit) ---

    #[test]
    fn alert_dealer_moves_closer_to_a_given_pursuit_target() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        // Primero entra a Alert (sin objetivo de persecución todavía).
        entity.update(near_player, 0.016, 48, None);
        assert_eq!(entity.state(), EntityState::Alert);

        let start_position = entity.position();

        let target = Vector2::new(start_position.x + 48.0, start_position.y);

        let distance_before = (target.x - start_position.x).hypot(target.y - start_position.y);

        entity.update(near_player, 0.1, 48, Some(target));

        let distance_after = (target.x - entity.position().x).hypot(target.y - entity.position().y);

        assert!(distance_after < distance_before);
        // Todavía no debe haber sobrepasado el objetivo con un solo
        // paso de 0.1s a 75px/s (7.5px < 48px de distancia inicial).
        assert!(distance_after > 0.0);
    }

    #[test]
    fn idle_dealer_does_not_move_even_with_a_pursuit_target() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        let start_position = entity.position();

        let target = Vector2::new(start_position.x + 48.0, start_position.y);

        entity.update(far_player, 0.1, 48, Some(target));

        assert_eq!(entity.state(), EntityState::Idle);
        assert_eq!(entity.position().x, start_position.x);
        assert_eq!(entity.position().y, start_position.y);
    }

    #[test]
    fn hit_dealer_does_not_move_during_hit_reaction_even_with_a_pursuit_target() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.apply_damage(50);
        assert_eq!(entity.state(), EntityState::Hit);

        let start_position = entity.position();

        let target = Vector2::new(start_position.x + 48.0, start_position.y);

        // Delta pequeño: el temporizador de Hit (0.15s) sigue activo.
        entity.update(near_player, 0.016, 48, Some(target));

        assert_eq!(entity.state(), EntityState::Hit);
        assert_eq!(entity.position().x, start_position.x);
        assert_eq!(entity.position().y, start_position.y);
    }

    #[test]
    fn dead_dealer_never_moves_even_with_a_pursuit_target() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert!(entity.is_dead());

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        let start_position = entity.position();

        let target = Vector2::new(start_position.x + 48.0, start_position.y);

        entity.update(near_player, 0.5, 48, Some(target));

        assert_eq!(entity.state(), EntityState::Dead);
        assert_eq!(entity.position().x, start_position.x);
        assert_eq!(entity.position().y, start_position.y);
    }

    #[test]
    fn pursuit_step_scales_with_delta_time() {
        let mut small_delta_entity = Entity::dealer_at_cell(0, 0, 48);

        let mut large_delta_entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(
            small_delta_entity.position().x + 10.0,
            small_delta_entity.position().y,
        );

        small_delta_entity.update(near_player, 0.016, 48, None);
        large_delta_entity.update(near_player, 0.016, 48, None);

        let start = small_delta_entity.position();

        let target = Vector2::new(start.x + 480.0, start.y);

        small_delta_entity.update(near_player, 0.01, 48, Some(target));
        large_delta_entity.update(near_player, 0.02, 48, Some(target));

        let small_moved = small_delta_entity.position().x - start.x;

        let large_moved = large_delta_entity.position().x - start.x;

        // El doble de delta_time produce (aproximadamente) el doble
        // de avance: movimiento independiente del framerate.
        assert!(large_moved > small_moved);
        assert!((large_moved - 2.0 * small_moved).abs() < 1e-3);
    }

    #[test]
    fn pursuit_snaps_to_target_instead_of_overshooting() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.016, 48, None);
        assert_eq!(entity.state(), EntityState::Alert);

        let start = entity.position();

        // Objetivo a 5px: por encima del umbral de parada (2.4px a
        // block_size=48) pero por debajo del paso de este cuadro
        // (0.1s a 75px/s = 7.5px), que lo sobrepasaría si no se
        // recortara.
        let target = Vector2::new(start.x + 5.0, start.y);

        entity.update(near_player, 0.1, 48, Some(target));

        assert_eq!(entity.position().x, target.x);
        assert_eq!(entity.position().y, target.y);
    }

    #[test]
    fn pursuit_ignores_non_finite_or_non_positive_delta_time() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.016, 48, None);
        assert_eq!(entity.state(), EntityState::Alert);

        let start = entity.position();

        let target = Vector2::new(start.x + 48.0, start.y);

        entity.update(near_player, 0.0, 48, Some(target));
        entity.update(near_player, -1.0, 48, Some(target));
        entity.update(near_player, f32::NAN, 48, Some(target));

        assert_eq!(entity.position().x, start.x);
        assert_eq!(entity.position().y, start.y);
    }

    // --- Tarea 45: ataques de Dealer ---

    const BLOCK_SIZE: usize = 48;

    /// Pone un Dealer nuevo en `Alert`, dentro de rango de ataque,
    /// listo para atacar de inmediato (cooldown en `0.0`).
    fn alert_dealer_in_range() -> (Entity, Vector2) {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        let player_position = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(player_position, 0.016, BLOCK_SIZE, None);

        assert_eq!(entity.state(), EntityState::Alert);

        (entity, player_position)
    }

    #[test]
    fn ready_alert_dealer_in_range_attacks_immediately() {
        let (mut entity, player_position) = alert_dealer_in_range();

        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn distance_exactly_at_the_range_boundary_is_eligible() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        let attack_range = BLOCK_SIZE as f32 * DEALER_ATTACK_RANGE_CELLS;

        // Justo dentro de la distancia de alerta (4 celdas) para
        // entrar en Alert, pero a EXACTAMENTE `attack_range` para
        // probar el límite inclusive.
        let player_position = Vector2::new(entity.position().x + attack_range, entity.position().y);

        entity.update(player_position, 0.016, BLOCK_SIZE, None);
        assert_eq!(entity.state(), EntityState::Alert);

        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn distance_just_beyond_the_range_is_not_eligible() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        let attack_range = BLOCK_SIZE as f32 * DEALER_ATTACK_RANGE_CELLS;

        let player_position = Vector2::new(
            entity.position().x + attack_range + 1.0,
            entity.position().y,
        );

        entity.update(player_position, 0.016, BLOCK_SIZE, None);
        assert_eq!(entity.state(), EntityState::Alert);

        assert!(!entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn out_of_range_dealer_cannot_attack() {
        let (mut entity, _) = alert_dealer_in_range();

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        // Todavía "Alert" en cuanto a `attempt_attack` (que no
        // recalcula estado por sí mismo), pero fuera de rango.
        assert!(!entity.attempt_attack(far_player, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn attack_enters_cooldown_and_blocks_the_immediate_next_frame() {
        let (mut entity, player_position) = alert_dealer_in_range();

        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));

        assert!(!entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn attack_is_blocked_before_the_cooldown_duration_elapses() {
        let (mut entity, player_position) = alert_dealer_in_range();

        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));

        // 0.89s acumulados: todavía por debajo de 0.9s.
        assert!(!entity.attempt_attack(player_position, 0.89 - 0.016, BLOCK_SIZE));
    }

    #[test]
    fn attack_is_available_again_at_or_after_the_cooldown_duration() {
        let (mut entity, player_position) = alert_dealer_in_range();

        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));

        assert!(!entity.attempt_attack(player_position, 0.9 - 0.016, BLOCK_SIZE));

        // El resto exacto para completar 0.9s.
        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn cooldown_does_not_cause_damage_every_frame() {
        let (mut entity, player_position) = alert_dealer_in_range();

        let mut accepted_count = 0;

        // 60 cuadros a ~0.016s (~0.96s de tiempo simulado): con un
        // cooldown de 0.9s, como mucho puede aceptarse 2 veces
        // (t=0 y t~=0.9s), nunca 60.
        for _ in 0..60 {
            if entity.attempt_attack(player_position, 0.016, BLOCK_SIZE) {
                accepted_count += 1;
            }
        }

        assert!(accepted_count <= 2);
        assert!(accepted_count >= 1);
    }

    #[test]
    fn idle_dealer_cannot_attack_even_in_range_and_ready() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        assert_eq!(entity.state(), EntityState::Idle);

        let close_player = Vector2::new(entity.position().x + 5.0, entity.position().y);

        assert!(!entity.attempt_attack(close_player, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn hit_dealer_cannot_attack() {
        let (mut entity, player_position) = alert_dealer_in_range();

        entity.apply_damage(10);
        assert_eq!(entity.state(), EntityState::Hit);

        assert!(!entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn entering_hit_resets_the_attack_cooldown_to_full() {
        let (mut entity, player_position) = alert_dealer_in_range();

        // Cooldown ya listo (recién construido). Un golpe no letal
        // debe recargarlo al completo en vez de dejarlo en 0.0.
        entity.apply_damage(10);
        assert_eq!(entity.state(), EntityState::Hit);

        // Deja expirar el temporizador de Hit (0.15s) para volver a
        // Alert, y confirma que el Dealer NO puede golpear de
        // inmediato al recuperarse. `Entity::update` nunca decrementa
        // `attack_cooldown_remaining` (solo `attempt_attack` lo
        // hace), así que el cooldown sigue en el valor COMPLETO
        // (0.9s) recargado al entrar en `Hit`.
        entity.update(player_position, 0.20, BLOCK_SIZE, None);
        assert_eq!(entity.state(), EntityState::Alert);

        assert!(!entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));
    }

    #[test]
    fn dead_dealer_never_attacks_across_many_updates() {
        let (mut entity, player_position) = alert_dealer_in_range();

        entity.apply_damage(1000);
        assert!(entity.is_dead());

        for _ in 0..100 {
            assert!(!entity.attempt_attack(player_position, 0.5, BLOCK_SIZE));
        }
    }

    #[test]
    fn attempt_attack_ignores_non_finite_or_non_positive_delta_time_for_cooldown_ticking() {
        let (mut entity, player_position) = alert_dealer_in_range();

        assert!(entity.attempt_attack(player_position, 0.016, BLOCK_SIZE));

        // Cooldown activo; deltas inválidos no deben decrementarlo
        // ni permitir un ataque prematuro.
        assert!(!entity.attempt_attack(player_position, 0.0, BLOCK_SIZE));
        assert!(!entity.attempt_attack(player_position, -1.0, BLOCK_SIZE));
        assert!(!entity.attempt_attack(player_position, f32::NAN, BLOCK_SIZE));
    }

    // --- Dealer Hands: temporizador de cadáver ---

    #[test]
    fn corpse_timer_does_not_advance_while_alive() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        entity.advance_corpse_timer(20.0);

        assert!(!entity.should_despawn());
    }

    #[test]
    fn corpse_is_not_despawnable_before_the_full_duration() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        entity.apply_damage(1000);
        assert!(entity.is_dead());

        entity.advance_corpse_timer(CORPSE_DESPAWN_SECONDS - 0.1);

        assert!(!entity.should_despawn());
    }

    #[test]
    fn corpse_becomes_despawnable_at_or_after_the_full_duration() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        entity.apply_damage(1000);
        assert!(entity.is_dead());

        entity.advance_corpse_timer(CORPSE_DESPAWN_SECONDS);

        assert!(entity.should_despawn());
    }

    #[test]
    fn corpse_timer_accumulates_across_many_small_updates() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        entity.apply_damage(1000);

        for _ in 0..935 {
            entity.advance_corpse_timer(0.016);
        }

        // 935 * 0.016 = 14.96s: todavía no llega a 15.0s.
        assert!(!entity.should_despawn());

        entity.advance_corpse_timer(0.05);

        assert!(entity.should_despawn());
    }

    #[test]
    fn corpse_timer_ignores_non_finite_or_non_positive_delta_time() {
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        entity.apply_damage(1000);

        entity.advance_corpse_timer(0.0);
        entity.advance_corpse_timer(-5.0);
        entity.advance_corpse_timer(f32::NAN);
        entity.advance_corpse_timer(f32::INFINITY);

        assert!(!entity.should_despawn());
    }

    #[test]
    fn pausing_freezes_the_corpse_timer_because_it_never_advances_without_an_explicit_call() {
        // No hay reloj de pared: `corpse_elapsed` SOLO avanza cuando
        // `advance_corpse_timer` es invocado explícitamente (desde
        // `update_playing`, nunca desde `update_paused`). Simular
        // "20 segundos reales de Pause" es, en la práctica, no
        // llamar a `advance_corpse_timer` en absoluto durante ese
        // intervalo — exactamente lo que esta prueba demuestra no
        // moviendo el timer entre dos verificaciones.
        let mut entity = Entity::dealer_at_cell(0, 0, BLOCK_SIZE);

        entity.apply_damage(1000);
        entity.advance_corpse_timer(7.0);

        // "Pause": ninguna llamada a advance_corpse_timer aquí,
        // sin importar cuánto tiempo real pase.
        assert!(!entity.should_despawn());

        // "Resume": retoma exactamente en 7.0s, no en 7.0 + tiempo
        // de pausa.
        entity.advance_corpse_timer(CORPSE_DESPAWN_SECONDS - 7.0 - 0.01);

        assert!(!entity.should_despawn());
    }

    #[test]
    fn dead_dealer_never_pursues_or_attacks_regardless_of_corpse_age() {
        let (mut entity, player_position) = alert_dealer_in_range();

        entity.apply_damage(1000);
        assert!(entity.is_dead());

        entity.advance_corpse_timer(10.0);

        let start_position = entity.position();

        let target = Vector2::new(start_position.x + 48.0, start_position.y);

        entity.update(player_position, 0.5, BLOCK_SIZE, Some(target));

        assert_eq!(entity.position().x, start_position.x);
        assert_eq!(entity.position().y, start_position.y);
        assert!(!entity.attempt_attack(player_position, 0.5, BLOCK_SIZE));
        assert_eq!(entity.apply_damage(50), EntityDamageOutcome::None);
    }
}
