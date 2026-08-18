use super::level::Level;

/// Radio utilizado para evitar que el jugador toque o atraviese paredes.
const COLLISION_RADIUS: f32 = 7.0;

/// Comprueba si una coordenada en píxeles corresponde
/// a una posición transitable dentro del laberinto.
fn is_point_walkable(level: &Level, x: f32, y: f32, block_size: usize) -> bool {
    if x < 0.0 || y < 0.0 {
        return false;
    }

    let column = (x / block_size as f32).floor() as usize;
    let row = (y / block_size as f32).floor() as usize;

    level.is_walkable(row, column)
}

/// Comprueba cuatro puntos alrededor del jugador.
///
/// Esto evita que solamente el centro del jugador sea considerado
/// y que sus bordes atraviesen una pared.
pub fn can_occupy(level: &Level, x: f32, y: f32, block_size: usize) -> bool {
    let collision_points = [
        (x - COLLISION_RADIUS, y - COLLISION_RADIUS),
        (x + COLLISION_RADIUS, y - COLLISION_RADIUS),
        (x - COLLISION_RADIUS, y + COLLISION_RADIUS),
        (x + COLLISION_RADIUS, y + COLLISION_RADIUS),
    ];

    collision_points
        .into_iter()
        .all(|(point_x, point_y)| is_point_walkable(level, point_x, point_y, block_size))
}
