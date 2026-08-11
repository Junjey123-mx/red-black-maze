use crate::world::Level;
use raylib::prelude::Vector2;
use std::f32::consts::PI;

/// Representa al jugador dentro del laberinto.
pub struct Player {
    /// Posición del jugador expresada en píxeles.
    pub pos: Vector2,

    /// Dirección central hacia donde mira el jugador.
    pub a: f32,

    /// Campo de visión total expresado en radianes.
    pub fov: f32,
}

impl Player {
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
        }
    }
}
