use raylib::prelude::Vector2;

/// Estado de combate/comportamiento de una entidad.
///
/// En Tarea 23 toda entidad permanece en `Idle`: `Alert`, `Hit` y
/// `Dead` existen como parte del modelo, pero ninguna transición
/// real hacia ellos ocurre todavía (eso pertenece a Tarea 24).
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

/// Vida máxima inicial de un Dealer.
const DEALER_MAX_HEALTH: i32 = 100;

/// Radio de impacto del Dealer, en unidades de mundo (píxeles).
///
/// Es geometría de mundo, no presentación: se eligió
/// deliberadamente mucho menor que `BLOCK_SIZE` (48 unidades) para
/// que el círculo de impacto quede claramente contenido dentro de
/// la celda de aparición.
const DEALER_HIT_RADIUS: f32 = 12.0;

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
    #[allow(dead_code)]
    pub(crate) fn is_dead(&self) -> bool {
        self.state == EntityState::Dead
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
}
