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
}
