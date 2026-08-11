use crate::player::Player;
use crate::world::{Level, can_occupy};
use raylib::prelude::{KeyboardKey, RaylibHandle};
use std::f32::consts::{PI, TAU};

/// Velocidad del jugador medida en píxeles por segundo.
const MOVE_SPEED: f32 = 150.0;

/// Velocidad de rotación medida en radianes por segundo.
const ROTATION_SPEED: f32 = PI;

/// Procesa el teclado y actualiza la posición y el ángulo
/// del jugador.
pub fn process_events(
    window: &RaylibHandle,
    player: &mut Player,
    level: &Level,
    block_size: usize,
) {
    // Limitar el delta evita saltos grandes si la ventana
    // se congela momentáneamente.
    let delta_time = window.get_frame_time().clamp(0.0, 0.05);

    /*
     * ROTACIÓN
     *
     * Izquierda/A disminuyen el ángulo.
     * Derecha/D aumentan el ángulo.
     */
    let mut rotation_direction = 0.0;

    if window.is_key_down(KeyboardKey::KEY_LEFT) || window.is_key_down(KeyboardKey::KEY_A) {
        rotation_direction -= 1.0;
    }

    if window.is_key_down(KeyboardKey::KEY_RIGHT) || window.is_key_down(KeyboardKey::KEY_D) {
        rotation_direction += 1.0;
    }

    player.a += rotation_direction * ROTATION_SPEED * delta_time;

    // Mantener el ángulo dentro del intervalo 0 a 2π.
    player.a = player.a.rem_euclid(TAU);

    /*
     * MOVIMIENTO
     *
     * Arriba/W avanzan.
     * Abajo/S retroceden.
     */
    let mut movement_direction = 0.0;

    if window.is_key_down(KeyboardKey::KEY_UP) || window.is_key_down(KeyboardKey::KEY_W) {
        movement_direction += 1.0;
    }

    if window.is_key_down(KeyboardKey::KEY_DOWN) || window.is_key_down(KeyboardKey::KEY_S) {
        movement_direction -= 1.0;
    }

    if movement_direction == 0.0 {
        return;
    }

    let movement_distance = MOVE_SPEED * movement_direction * delta_time;

    /*
     * El coseno controla el movimiento horizontal.
     * El seno controla el movimiento vertical.
     */
    let movement_x = player.a.cos() * movement_distance;

    let movement_y = player.a.sin() * movement_distance;

    let proposed_x = player.pos.x + movement_x;
    let proposed_y = player.pos.y + movement_y;

    /*
     * Comprobamos X y Y por separado.
     *
     * Esto permite que el jugador se deslice a lo largo
     * de una pared en lugar de quedarse completamente detenido.
     */
    if can_occupy(level, proposed_x, player.pos.y, block_size) {
        player.pos.x = proposed_x;
    }

    if can_occupy(level, player.pos.x, proposed_y, block_size) {
        player.pos.y = proposed_y;
    }
}
