use raylib::prelude::Vector2;

/// The Royal Flush recogible dentro de un nivel (Bloque 2, Commit 14).
///
/// Estado de PARTIDA puro, con la MISMA forma que
/// `AmmoPickup`/`HealthPickup` y por el mismo motivo: no tiene vida,
/// IA, pathfinding ni estados propios — solo recuerda DÓNDE está y si
/// sigue disponible en ESTA run.
///
/// A diferencia de la munición/vida, la mejora es ÚNICA: `GameSession`
/// guarda como mucho un `RoyalFlushPickup` por run (un `Option`, no un
/// `Vec`) y nunca vuelve a spawnear otro una vez colocado, recogido o
/// no.
///
/// No renderiza píxeles, no conoce `Framebuffer`/`TextureManager`, no
/// decide qué efecto tiene recogerlo (el ascenso de `WeaponTier` vive
/// en `GameSession`, la única capa con acceso al `Weapon`) y no
/// realiza la comprobación de distancia contra el jugador (también en
/// `GameSession`).
pub(crate) struct RoyalFlushPickup {
    position: Vector2,
    active: bool,
}

impl RoyalFlushPickup {
    /// Crea la mejora activa centrada exactamente en la celda
    /// `(row, column)`.
    ///
    /// Misma fórmula de centrado de celda que
    /// `AmmoPickup::at_cell`/`HealthPickup::at_cell`/
    /// `Entity::dealer_at_cell`: no se inventa una segunda conversión
    /// cell -> world.
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

    /// `true` si la mejora todavía no fue recogida en esta run.
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// Marca la mejora como recogida. No reaparece: `GameSession`
    /// nunca coloca otra en la misma run, y una run nueva (Retry/
    /// cambio de nivel/menú) reconstruye la sesión entera desde cero.
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
        let pickup = RoyalFlushPickup::at_cell(1, 2, BLOCK_SIZE);

        assert!(pickup.is_active());
    }

    #[test]
    fn position_matches_the_established_cell_center_convention() {
        let pickup = RoyalFlushPickup::at_cell(1, 2, BLOCK_SIZE);

        assert_eq!(
            pickup.position(),
            Vector2::new(2.0 * 48.0 + 24.0, 48.0 + 24.0)
        );
    }

    #[test]
    fn deactivate_makes_it_inactive() {
        let mut pickup = RoyalFlushPickup::at_cell(0, 0, BLOCK_SIZE);

        pickup.deactivate();

        assert!(!pickup.is_active());
    }
}
