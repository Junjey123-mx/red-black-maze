use crate::world::Level;
use raylib::prelude::Vector2;
use std::f32::consts::PI;

/// Vida máxima e inicial del jugador.
///
/// Tarea 26 introduce esta vida como estado de PARTIDA real para
/// alimentar el HUD; todavía no existe ningún sistema que la
/// reduzca (eso pertenece a una tarea futura de combate contra el
/// jugador).
pub(crate) const PLAYER_MAX_HEALTH: i32 = 100;

/// Representa al jugador dentro del laberinto.
pub struct Player {
    /// Posición del jugador expresada en píxeles.
    pub pos: Vector2,

    /// Dirección central hacia donde mira el jugador.
    pub a: f32,

    /// Campo de visión total expresado en radianes.
    pub fov: f32,

    /// Puntos de vida restantes del jugador.
    health: i32,
}

impl Player {
    /// Construye un jugador en una posición y orientación
    /// arbitrarias, con vida máxima inicial.
    ///
    /// Constructor de dominio genérico (Tarea 37): a diferencia de
    /// `from_level`, no depende de descubrir una celda de aparición
    /// en un `Level`, por lo que sirve para construir jugadores en
    /// posiciones exactas conocidas (por ejemplo, geometría de
    /// prueba determinista para raycasting/hitscan).
    pub fn new(pos: Vector2, a: f32, fov: f32) -> Self {
        Self {
            pos,
            a,
            fov,
            health: PLAYER_MAX_HEALTH,
        }
    }

    /// Coloca al jugador en el centro de la celda de aparición
    /// que el nivel ya descubrió.
    pub(crate) fn from_level(level: &Level, block_size: usize) -> Self {
        let (row, column) = level.player_spawn();

        let half_block = block_size as f32 / 2.0;

        let x = column as f32 * block_size as f32 + half_block;

        let y = row as f32 * block_size as f32 + half_block;

        Self {
            pos: Vector2::new(x, y),

            // Dirección inicial de 60 grados.
            a: PI / 3.0,

            // Campo de visión total de 60 grados.
            fov: PI / 3.0,

            health: PLAYER_MAX_HEALTH,
        }
    }

    /// Puntos de vida restantes del jugador.
    pub(crate) fn health(&self) -> i32 {
        self.health
    }

    /// Aplica daño real al jugador (Tarea 45) y retorna la cantidad
    /// REALMENTE aplicada (nunca más de lo que quedaba de vida).
    ///
    /// La vida se recorta con un piso de `0` — nunca queda
    /// negativa/underflow. Un `amount` no positivo no aplica ningún
    /// daño y retorna `0`. Esta es la ÚNICA función que escribe
    /// `self.health`; `GameSession`/`App` nunca mutan el campo
    /// directamente.
    ///
    /// Ejemplos: `100 - 10 -> 90` (retorna `10`); `5 - 10 -> 0`
    /// (retorna `5`, el daño real absorbido); `0 - 10 -> 0` (retorna
    /// `0`: no hay más vida que quitar).
    pub(crate) fn apply_damage(&mut self, amount: i32) -> i32 {
        if amount <= 0 {
            return 0;
        }

        let health_before = self.health;

        self.health = (self.health - amount).max(0);

        health_before - self.health
    }

    /// Restaura vida real al jugador (Health Pickup) y retorna la
    /// cantidad REALMENTE aplicada (nunca más de lo que faltaba para
    /// llegar a `PLAYER_MAX_HEALTH`).
    ///
    /// La vida se recorta con un techo de `PLAYER_MAX_HEALTH` — nunca
    /// queda por encima. Un `amount` no positivo no cura nada y
    /// retorna `0`. Esta es la ÚNICA función que incrementa
    /// `self.health`; `GameSession`/`App` nunca mutan el campo
    /// directamente. Mismo patrón "solo lo realmente aplicado" que
    /// `apply_damage` y que `Weapon::add_reserve_ammo`.
    ///
    /// Ejemplos: `60 + 20 -> 80` (retorna `20`); `90 + 20 -> 100`
    /// (retorna `10`, lo único que faltaba); `100 + 20 -> 100`
    /// (retorna `0`: ya estaba en el máximo).
    pub(crate) fn heal(&mut self, amount: i32) -> i32 {
        if amount <= 0 {
            return 0;
        }

        let health_before = self.health;

        self.health = (self.health + amount).min(PLAYER_MAX_HEALTH);

        self.health - health_before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un jugador con la misma inicialización que
    /// `Player::from_level`, sin depender de leer un archivo de
    /// nivel desde disco: la prueba solo necesita verificar el
    /// estado de vida inicial, no el descubrimiento del spawn.
    fn new_test_player() -> Player {
        Player {
            pos: Vector2::new(0.0, 0.0),
            a: PI / 3.0,
            fov: PI / 3.0,
            health: PLAYER_MAX_HEALTH,
        }
    }

    #[test]
    fn new_player_starts_with_full_health() {
        let player = new_test_player();

        assert_eq!(player.health(), PLAYER_MAX_HEALTH);
        assert_eq!(player.health(), 100);
    }

    // --- Tarea 45: Player::apply_damage. ---

    #[test]
    fn ordinary_damage_reduces_health_and_returns_the_amount_applied() {
        let mut player = new_test_player();

        let applied = player.apply_damage(10);

        assert_eq!(player.health(), 90);
        assert_eq!(applied, 10);
    }

    #[test]
    fn damage_exceeding_remaining_health_clamps_to_zero() {
        let mut player = new_test_player();

        player.apply_damage(95);
        assert_eq!(player.health(), 5);

        let applied = player.apply_damage(10);

        assert_eq!(player.health(), 0);
        assert_eq!(applied, 5);
    }

    #[test]
    fn damage_at_zero_health_applies_nothing_and_never_underflows() {
        let mut player = new_test_player();

        player.apply_damage(1000);
        assert_eq!(player.health(), 0);

        let applied = player.apply_damage(10);

        assert_eq!(player.health(), 0);
        assert_eq!(applied, 0);
    }

    #[test]
    fn non_positive_damage_is_a_no_op() {
        let mut player = new_test_player();

        assert_eq!(player.apply_damage(0), 0);
        assert_eq!(player.apply_damage(-10), 0);
        assert_eq!(player.health(), PLAYER_MAX_HEALTH);
    }

    // --- Health Pickup: Player::heal. ---

    #[test]
    fn ordinary_heal_increases_health_and_returns_the_amount_applied() {
        let mut player = new_test_player();

        player.apply_damage(40);
        assert_eq!(player.health(), 60);

        let applied = player.heal(20);

        assert_eq!(player.health(), 80);
        assert_eq!(applied, 20);
    }

    #[test]
    fn heal_clamps_at_the_maximum_and_returns_only_what_was_missing() {
        let mut player = new_test_player();

        player.apply_damage(10);
        assert_eq!(player.health(), 90);

        let applied = player.heal(20);

        assert_eq!(player.health(), PLAYER_MAX_HEALTH);
        assert_eq!(applied, 10);
    }

    #[test]
    fn heal_at_full_health_applies_nothing() {
        let mut player = new_test_player();

        assert_eq!(player.health(), PLAYER_MAX_HEALTH);

        let applied = player.heal(20);

        assert_eq!(player.health(), PLAYER_MAX_HEALTH);
        assert_eq!(applied, 0);
    }

    #[test]
    fn non_positive_heal_is_a_no_op() {
        let mut player = new_test_player();

        player.apply_damage(50);

        assert_eq!(player.heal(0), 0);
        assert_eq!(player.heal(-10), 0);
        assert_eq!(player.health(), 50);
    }

    #[test]
    fn exact_heal_lands_precisely_on_the_maximum() {
        let mut player = new_test_player();

        player.apply_damage(20);
        assert_eq!(player.health(), 80);

        let applied = player.heal(20);

        assert_eq!(player.health(), PLAYER_MAX_HEALTH);
        assert_eq!(applied, 20);
    }
}
