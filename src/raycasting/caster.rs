use crate::config::BLOCK_SIZE;
use crate::player::Player;
use crate::world::Level;

use super::hit::RayHit;

/// Lanza un único rayo desde el jugador y calcula dónde impacta.
///
/// `ray_angle` representa la dirección específica del rayo.
pub(crate) fn cast_ray(level: &Level, player: &Player, ray_angle: f32) -> RayHit {
    const STEP_SIZE: f32 = 1.0;

    let map_width = level.width() as f32 * BLOCK_SIZE as f32;

    let map_height = level.height() as f32 * BLOCK_SIZE as f32;

    let max_distance = (map_width * map_width + map_height * map_height).sqrt();

    let mut distance = 0.0;

    while distance <= max_distance {
        /*
         * Posición actual del rayo:
         *
         * x = jugador.x + distancia × cos(ángulo)
         * y = jugador.y + distancia × sin(ángulo)
         */
        let ray_x = player.pos.x + distance * ray_angle.cos();

        let ray_y = player.pos.y + distance * ray_angle.sin();

        /*
         * Detenerse si el rayo sale del mapa.
         */
        if ray_x < 0.0 || ray_y < 0.0 || ray_x >= map_width || ray_y >= map_height {
            return RayHit {
                distance,
                tile: '#',
            };
        }

        /*
         * Convertir la posición en píxeles a una posición
         * dentro de la matriz del laberinto.
         */
        let column = (ray_x / BLOCK_SIZE as f32).floor() as usize;

        let row = (ray_y / BLOCK_SIZE as f32).floor() as usize;

        let Some(cell) = level.cell_at(row, column) else {
            return RayHit {
                distance,
                tile: '#',
            };
        };

        /*
         * Si no es una celda transitable, el rayo
         * encontró una pared.
         */
        if !level.is_walkable(row, column) {
            return RayHit {
                distance,
                tile: cell,
            };
        }

        distance += STEP_SIZE;
    }

    RayHit {
        distance: max_distance,
        tile: '#',
    }
}
