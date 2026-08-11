use raylib::prelude::Vector2;

use crate::config::BLOCK_SIZE;
use crate::player::Player;
use crate::world::Level;

use super::hit::RayHit;

/// Umbral utilizado para considerar una componente de dirección
/// como efectivamente nula al buscar cruces de borde de celda.
const DIRECTION_EPSILON: f32 = 1e-6;

/// Lanza un único rayo desde el jugador y calcula dónde impacta.
///
/// `ray_angle` representa la dirección específica del rayo.
pub(crate) fn cast_ray(level: &Level, player: &Player, ray_angle: f32) -> RayHit {
    const STEP_SIZE: f32 = 1.0;

    let direction_x = ray_angle.cos();

    let direction_y = ray_angle.sin();

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
        let ray_x = player.pos.x + distance * direction_x;

        let ray_y = player.pos.y + distance * direction_y;

        /*
         * Detenerse si el rayo sale del mapa.
         */
        if ray_x < 0.0 || ray_y < 0.0 || ray_x >= map_width || ray_y >= map_height {
            return fallback_hit(player, direction_x, direction_y, distance);
        }

        /*
         * Convertir la posición en píxeles a una posición
         * dentro de la matriz del laberinto.
         */
        let column = (ray_x / BLOCK_SIZE as f32).floor() as usize;

        let row = (ray_y / BLOCK_SIZE as f32).floor() as usize;

        let Some(cell) = level.cell_at(row, column) else {
            return fallback_hit(player, direction_x, direction_y, distance);
        };

        /*
         * Si no es una celda transitable, el rayo
         * encontró una pared.
         *
         * El punto muestreado por el marcher está dentro de la
         * celda; a partir de aquí se refina matemáticamente hasta
         * el borde exacto de entrada.
         */
        if !level.is_walkable(row, column) {
            return calculate_wall_hit(player.pos, direction_x, direction_y, row, column, cell);
        }

        distance += STEP_SIZE;
    }

    fallback_hit(player, direction_x, direction_y, max_distance)
}

/// Calcula el punto exacto donde el rayo entra a la celda de
/// pared ya identificada por el marcher, junto con el
/// desplazamiento de textura normalizado sobre la cara golpeada.
fn calculate_wall_hit(
    player_position: Vector2,
    direction_x: f32,
    direction_y: f32,
    row: usize,
    column: usize,
    tile: char,
) -> RayHit {
    let cell_left = column as f32 * BLOCK_SIZE as f32;

    let cell_right = cell_left + BLOCK_SIZE as f32;

    let cell_top = row as f32 * BLOCK_SIZE as f32;

    let cell_bottom = cell_top + BLOCK_SIZE as f32;

    /*
     * Candidato de entrada por el eje X: el borde izquierdo o
     * derecho según el signo de la dirección. `None` cuando la
     * dirección es efectivamente nula en ese eje.
     */
    let entry_x = if direction_x > DIRECTION_EPSILON {
        Some((cell_left - player_position.x) / direction_x)
    } else if direction_x < -DIRECTION_EPSILON {
        Some((cell_right - player_position.x) / direction_x)
    } else {
        None
    };

    /*
     * Candidato de entrada por el eje Y: el borde superior o
     * inferior según el signo de la dirección.
     */
    let entry_y = if direction_y > DIRECTION_EPSILON {
        Some((cell_top - player_position.y) / direction_y)
    } else if direction_y < -DIRECTION_EPSILON {
        Some((cell_bottom - player_position.y) / direction_y)
    } else {
        None
    };

    /*
     * La entrada real a la celda es la más tardía de los
     * candidatos válidos: el rayo no está completamente dentro
     * de la celda hasta que cruza AMBOS bordes.
     *
     * `is_vertical_face` indica si el borde determinante fue uno
     * X (cara vertical) o uno Y (cara horizontal). En un empate
     * exacto se elige la cara vertical de forma determinista.
     */
    let (exact_distance, is_vertical_face) = match (entry_x, entry_y) {
        (Some(x), Some(y)) if y > x => (y, false),
        (Some(x), Some(_)) => (x, true),
        (Some(x), None) => (x, true),
        (None, Some(y)) => (y, false),
        (None, None) => (0.0, true),
    };

    let exact_distance = exact_distance.max(0.0);

    let impact_x = player_position.x + direction_x * exact_distance;

    let impact_y = player_position.y + direction_y * exact_distance;

    /*
     * En una cara vertical (X fijo en un borde) el desplazamiento
     * de textura recorre el eje Y de la celda; en una cara
     * horizontal recorre el eje X.
     */
    let texture_offset = if is_vertical_face {
        (impact_y / BLOCK_SIZE as f32).rem_euclid(1.0)
    } else {
        (impact_x / BLOCK_SIZE as f32).rem_euclid(1.0)
    };

    RayHit {
        distance: exact_distance,
        tile,
        position: Vector2::new(impact_x, impact_y),
        texture_offset,
    }
}

/// Construye un `RayHit` de reserva para los casos en los que el
/// rayo no encontró una celda de pared real (fuera del mapa o
/// distancia máxima agotada).
///
/// No existe una cara de pared real que muestrear, así que el
/// desplazamiento de textura se deriva de forma determinista del
/// eje dominante de la dirección del rayo, manteniendo siempre
/// `texture_offset` en `[0.0, 1.0)` y `position` consistente con
/// `distance`.
fn fallback_hit(player: &Player, direction_x: f32, direction_y: f32, distance: f32) -> RayHit {
    let impact_x = player.pos.x + direction_x * distance;

    let impact_y = player.pos.y + direction_y * distance;

    let texture_offset = if direction_x.abs() >= direction_y.abs() {
        (impact_y / BLOCK_SIZE as f32).rem_euclid(1.0)
    } else {
        (impact_x / BLOCK_SIZE as f32).rem_euclid(1.0)
    };

    RayHit {
        distance,
        tile: '#',
        position: Vector2::new(impact_x, impact_y),
        texture_offset,
    }
}
