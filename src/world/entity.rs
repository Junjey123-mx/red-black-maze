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

/// Radio de impacto del Dealer, en unidades de mundo (píxeles).
///
/// Es geometría de mundo, no presentación: se eligió
/// deliberadamente mucho menor que `BLOCK_SIZE` (48 unidades) para
/// que el círculo de impacto quede claramente contenido dentro de
/// la celda de aparición.
const DEALER_HIT_RADIUS: f32 = 12.0;

/// Duración visual del estado `Hit` tras un golpe no letal.
///
/// 0.15s es claramente observable (varios cuadros a 60 FPS) sin
/// bloquear la reevaluación de proximidad por mucho tiempo.
const DEALER_HIT_DURATION_SECONDS: f32 = 0.15;

/// Distancia de alerta, expresada en celdas de mapa. El jugador
/// dentro de esta distancia hace que un Dealer con vida y sin golpe
/// activo pase a `Alert`; fuera de ella vuelve a `Idle`.
const DEALER_ALERT_DISTANCE_CELLS: f32 = 4.0;

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
    hit_radius: f32,
    hit_time_remaining: f32,
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
            hit_radius: DEALER_HIT_RADIUS,
            hit_time_remaining: 0.0,
        }
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

            EntityDamageOutcome::Hit
        }
    }

    /// Avanza el comportamiento estático de la entidad: decrementa
    /// el temporizador de `Hit` y, cuando ya no hay golpe activo,
    /// reevalúa `Idle`/`Alert` según la distancia al jugador.
    /// Reporta `Some(EntityStateTransition)` únicamente cuando el
    /// estado ACTUALMENTE cambió durante esta llamada; `None` si
    /// permanece igual (incluyendo `Dead` y `Hit` con temporizador
    /// aún activo).
    ///
    /// `Dead` es terminal y esta función retorna inmediatamente sin
    /// alterar nada. Mientras `hit_time_remaining` siga siendo
    /// positivo, `Hit` NO es sobrescrito por la reevaluación de
    /// proximidad, ni siquiera dentro de la misma llamada: la
    /// entidad no se mueve, no ataca y no persigue; esto es
    /// únicamente reconocimiento visual por distancia, sin línea de
    /// visión.
    pub(crate) fn update(
        &mut self,
        player_position: Vector2,
        delta_time: f32,
        block_size: usize,
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

        let alert_distance = block_size as f32 * DEALER_ALERT_DISTANCE_CELLS;

        let dx = self.position.x - player_position.x;

        let dy = self.position.y - player_position.y;

        let distance_squared = dx * dx + dy * dy;

        self.state = if distance_squared <= alert_distance * alert_distance {
            EntityState::Alert
        } else {
            EntityState::Idle
        };

        if self.state == previous_state {
            None
        } else {
            Some(EntityStateTransition {
                from: previous_state,
                to: self.state,
            })
        }
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

        entity.update(near_player, 0.016, 48);

        assert_eq!(entity.state(), EntityState::Alert);
    }

    #[test]
    fn far_player_keeps_alert_dealer_idle() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.016, 48);

        assert_eq!(entity.state(), EntityState::Alert);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        entity.update(far_player, 0.016, 48);

        assert_eq!(entity.state(), EntityState::Idle);
    }

    #[test]
    fn hit_state_survives_while_timer_is_active() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);

        assert_eq!(entity.state(), EntityState::Hit);

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        // Un delta pequeño no debe agotar el temporizador de 0.15s.
        entity.update(near_player, 0.016, 48);

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
        entity.update(far_player, 0.20, 48);

        assert_eq!(entity.state(), EntityState::Idle);
    }

    #[test]
    fn dead_remains_dead_across_updates() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        entity.apply_damage(50);

        assert!(entity.is_dead());

        let near_player = Vector2::new(entity.position().x + 10.0, entity.position().y);

        entity.update(near_player, 0.5, 48);

        assert_eq!(entity.state(), EntityState::Dead);
        assert!(entity.is_dead());
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

        let transition = entity.update(near_player, 0.016, 48);

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

        entity.update(near_player, 0.016, 48);
        assert_eq!(entity.state(), EntityState::Alert);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        let transition = entity.update(far_player, 0.016, 48);

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

        let transition = entity.update(far_player, 0.016, 48);

        assert_eq!(transition, None);
        assert_eq!(entity.state(), EntityState::Idle);
    }

    #[test]
    fn hit_to_idle_after_expiry_reports_transition() {
        let mut entity = Entity::dealer_at_cell(0, 0, 48);

        entity.apply_damage(50);
        assert_eq!(entity.state(), EntityState::Hit);

        let far_player = Vector2::new(entity.position().x + 10_000.0, entity.position().y);

        let transition = entity.update(far_player, 0.20, 48);

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

        let transition = entity.update(near_player, 0.20, 48);

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

        let transition = entity.update(near_player, 0.016, 48);

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

        let transition = entity.update(near_player, 0.5, 48);

        assert_eq!(transition, None);
        assert_eq!(entity.state(), EntityState::Dead);
    }
}
