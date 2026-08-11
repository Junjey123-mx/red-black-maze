use crate::config::MOUSE_SENSITIVITY;
use crate::player::Player;
use crate::world::{Level, can_occupy};
use raylib::prelude::{KeyboardKey, RaylibHandle};
use std::f32::consts::TAU;

/// Velocidad del jugador medida en píxeles por segundo.
const MOVE_SPEED: f32 = 150.0;

/// Procesa el mouse y el teclado, actualizando el ángulo y la
/// posición del jugador.
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
     * El desplazamiento horizontal del mouse gira la cámara. El
     * delta del mouse ya representa el movimiento ocurrido durante
     * este cuadro, por lo que NO se multiplica por delta_time.
     */
    let mouse_delta = window.get_mouse_delta();

    player.a += mouse_delta.x * MOUSE_SENSITIVITY;

    // Mantener el ángulo dentro del intervalo 0 a 2π.
    player.a = player.a.rem_euclid(TAU);

    /*
     * MOVIMIENTO
     *
     * W/S avanzan o retroceden en la dirección de la cámara.
     * A/D se desplazan lateralmente (strafe) sin rotar.
     */
    let mut forward_input = 0.0;

    if window.is_key_down(KeyboardKey::KEY_W) {
        forward_input += 1.0;
    }

    if window.is_key_down(KeyboardKey::KEY_S) {
        forward_input -= 1.0;
    }

    let mut strafe_input = 0.0;

    if window.is_key_down(KeyboardKey::KEY_D) {
        strafe_input += 1.0;
    }

    if window.is_key_down(KeyboardKey::KEY_A) {
        strafe_input -= 1.0;
    }

    if forward_input == 0.0 && strafe_input == 0.0 {
        return;
    }

    /*
     * Vectores relativos a la cámara actual.
     */
    let forward_x = player.a.cos();

    let forward_y = player.a.sin();

    let right_x = -player.a.sin();

    let right_y = player.a.cos();

    let mut move_x = forward_x * forward_input + right_x * strafe_input;

    let mut move_y = forward_y * forward_input + right_y * strafe_input;

    /*
     * Normalizar el vector combinado evita que el movimiento
     * diagonal (por ejemplo W+D) sea más rápido que un solo eje.
     */
    let move_magnitude = move_x.hypot(move_y);

    if move_magnitude > 1.0 {
        move_x /= move_magnitude;
        move_y /= move_magnitude;
    }

    let movement_distance = MOVE_SPEED * delta_time;

    let movement_x = move_x * movement_distance;

    let movement_y = move_y * movement_distance;

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
