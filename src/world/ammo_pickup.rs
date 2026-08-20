use raylib::prelude::Vector2;

/// Munición recogible dentro de un nivel (Tarea 44).
///
/// Estado de PARTIDA puro, deliberadamente separado de `Entity`: un
/// pickup no tiene vida, IA, pathfinding ni estados
/// `Idle`/`Alert`/`Hit`/`Dead` — forzarlo dentro de `Entity` habría
/// obligado a darle campos/comportamiento sin sentido para él. Su
/// única responsabilidad es recordar DÓNDE está y si sigue
/// disponible en ESTA sesión.
///
/// No renderiza píxeles, no conoce `Framebuffer`/`TextureAsset`/
/// `TextureManager`, no decide cuánta munición otorga (eso vive en
/// `GameSession`, que sí conoce `Weapon`) y no realiza la
/// comprobación de distancia contra el jugador (también en
/// `GameSession`, la única capa con acceso a `Player` y `Weapon`
/// simultáneamente).
pub(crate) struct AmmoPickup {
    position: Vector2,
    active: bool,
}

impl AmmoPickup {
    /// Crea un pickup activo centrado exactamente en la celda
    /// `(row, column)`.
    ///
    /// Misma fórmula de centrado de celda que
    /// `Entity::dealer_at_cell`/`Player::from_level`/
    /// `rendering::sprites::cell_center`: no se inventa una segunda
    /// conversión cell -> world.
    pub(crate) fn at_cell(row: usize, column: usize, block_size: usize) -> Self {
        let half_block = block_size as f32 / 2.0;

        let x = column as f32 * block_size as f32 + half_block;

        let y = row as f32 * block_size as f32 + half_block;

        Self {
            position: Vector2::new(x, y),
            active: true,
        }
    }

    /// Posición en el mundo, en píxeles.
    pub(crate) fn position(&self) -> Vector2 {
        self.position
    }

    /// `true` si el pickup todavía no fue recogido en esta sesión.
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// Marca el pickup como recogido. No reaparece hasta que se
    /// construya una `GameSession` nueva (que reconstruye los
    /// pickups desde `Level::ammo_spawns` otra vez).
    pub(crate) fn deactivate(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCK_SIZE: usize = 48;

    #[test]
    fn new_pickup_is_active() {
        let pickup = AmmoPickup::at_cell(1, 2, BLOCK_SIZE);

        assert!(pickup.is_active());
    }

    #[test]
    fn position_matches_the_established_cell_center_convention() {
        let pickup = AmmoPickup::at_cell(1, 2, BLOCK_SIZE);

        assert_eq!(
            pickup.position(),
            Vector2::new(2.0 * 48.0 + 24.0, 48.0 + 24.0)
        );
    }

    #[test]
    fn deactivate_makes_it_inactive() {
        let mut pickup = AmmoPickup::at_cell(0, 0, BLOCK_SIZE);

        pickup.deactivate();

        assert!(!pickup.is_active());
    }
}
