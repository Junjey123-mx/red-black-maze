use crate::config::MOUSE_SENSITIVITY;
use crate::player::Player;
use crate::world::{Level, can_occupy};
use raylib::prelude::{KeyboardKey, RaylibHandle};
use std::f32::consts::TAU;

/// Velocidad del jugador medida en píxeles por segundo.
const MOVE_SPEED: f32 = 150.0;

/// Intención de movimiento del teclado para un único cuadro: avance/
/// retroceso (`forward`) y desplazamiento lateral (`strafe`), cada
/// uno en el rango `[-1.0, 1.0]` antes de normalizar.
///
/// Estructura de dominio puro (sin `RaylibHandle`): representa
/// ÚNICAMENTE el estado de las teclas W/A/S/D de este cuadro, sin
/// conocer nada sobre el mouse/cámara. Esto hace explícito, y
/// comprobable sin abrir una ventana de Raylib, que el movimiento de
/// teclado y la rotación de mouse son dos canales de entrada
/// independientes: `process_events` calcula y aplica AMBOS en la
/// misma llamada, nunca uno a costa del otro.
#[derive(Debug, Clone, Copy, PartialEq)]
struct MovementIntent {
    forward: f32,
    strafe: f32,
}

impl MovementIntent {
    /// `true` si ninguna tecla de movimiento relevante está
    /// presionada (o si teclas opuestas se cancelaron entre sí, por
    /// ejemplo W+S).
    fn is_zero(&self) -> bool {
        self.forward == 0.0 && self.strafe == 0.0
    }
}

/// Lee el estado ACTUAL de W/A/S/D y construye la intención de
/// movimiento de este cuadro. Pura lectura de teclado: no toca
/// `player`, no conoce el mouse, y no decide colisión.
fn read_movement_intent(window: &RaylibHandle) -> MovementIntent {
    let mut forward = 0.0;

    if window.is_key_down(KeyboardKey::KEY_W) {
        forward += 1.0;
    }

    if window.is_key_down(KeyboardKey::KEY_S) {
        forward -= 1.0;
    }

    let mut strafe = 0.0;

    if window.is_key_down(KeyboardKey::KEY_D) {
        strafe += 1.0;
    }

    if window.is_key_down(KeyboardKey::KEY_A) {
        strafe -= 1.0;
    }

    MovementIntent { forward, strafe }
}

/// Rota `angle` por el desplazamiento horizontal de mouse de este
/// cuadro (`mouse_delta_x * sensitivity`), normalizado al intervalo
/// `[0, TAU)`.
///
/// Función pura (sin `RaylibHandle`/`Player`): no depende de ninguna
/// tecla de movimiento, por lo que puede — y en `process_events`
/// SIEMPRE— se aplica incondicionalmente, independientemente de si
/// W/A/S/D está presionado este mismo cuadro.
fn rotate_angle(angle: f32, mouse_delta_x: f32, sensitivity: f32) -> f32 {
    (angle + mouse_delta_x * sensitivity).rem_euclid(TAU)
}

/// Procesa el mouse y el teclado, actualizando el ángulo y la
/// posición del jugador.
///
/// La rotación (mouse) y el movimiento (teclado) son dos canales de
/// entrada INDEPENDIENTES: `rotate_angle` se aplica siempre, antes
/// de siquiera leer el teclado, y `read_movement_intent` se lee
/// siempre después, sin que ninguno de los dos dependa del estado
/// del otro. Mantener W/A/S/D presionado mientras se mueve el mouse
/// produce movimiento Y rotación en el mismo cuadro.
///
/// El delta horizontal viene de `RaylibHandle::get_mouse_delta`, que
/// asume el cursor ya capturado/oculto vía `disable_cursor` (ver
/// `App::sync_cursor_capture`) mientras `Playing` está activo. Tarea
/// 38.C.1: la causa raíz del "mouse look" que parecía congelarse al
/// sostener W/A/S/D no era esta ruta de entrada — era la función
/// "Desactivar mientras se escribe" de libinput/GNOME en el sistema
/// del usuario, confirmada y corregida como ajuste del sistema
/// operativo, fuera de este repositorio. Por eso esta función vuelve
/// a la ruta original de Raylib (`get_mouse_delta`), sin recentrado
/// manual ni lógica específica de plataforma.
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
     * Incondicional: no depende de qué teclas estén presionadas.
     */
    let mouse_delta = window.get_mouse_delta();

    player.a = rotate_angle(player.a, mouse_delta.x, MOUSE_SENSITIVITY);

    /*
     * MOVIMIENTO
     *
     * W/S avanzan o retroceden en la dirección de la cámara.
     * A/D se desplazan lateralmente (strafe) sin rotar. Incondicional
     * también: se lee siempre, independientemente del movimiento de
     * mouse ya aplicado arriba en este mismo cuadro.
     */
    let intent = read_movement_intent(window);

    if intent.is_zero() {
        return;
    }

    /*
     * Vectores relativos a la cámara actual.
     */
    let forward_x = player.a.cos();

    let forward_y = player.a.sin();

    let right_x = -player.a.sin();

    let right_y = player.a.cos();

    let mut move_x = forward_x * intent.forward + right_x * intent.strafe;

    let mut move_y = forward_y * intent.forward + right_y * intent.strafe;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_angle_adds_the_scaled_mouse_delta() {
        let result = rotate_angle(0.0, 100.0, 0.01);

        assert!((result - 1.0).abs() < 1e-5);
    }

    #[test]
    fn rotate_angle_normalizes_into_the_0_tau_range() {
        let result = rotate_angle(TAU - 0.05, 100.0, 0.01);

        assert!((0.0..TAU).contains(&result));
    }

    #[test]
    fn rotate_angle_handles_negative_delta_without_going_negative() {
        let result = rotate_angle(0.05, -100.0, 0.01);

        assert!((0.0..TAU).contains(&result));
    }

    #[test]
    fn rotate_angle_zero_delta_leaves_angle_unchanged() {
        let result = rotate_angle(1.2345, 0.0, MOUSE_SENSITIVITY);

        assert!((result - 1.2345).abs() < 1e-6);
    }

    #[test]
    fn movement_intent_w_plus_a_contains_both_components() {
        // Equivalente a W (forward=1.0) + A (strafe=-1.0) sostenidas
        // en el mismo cuadro.
        let intent = MovementIntent {
            forward: 1.0,
            strafe: -1.0,
        };

        assert!(!intent.is_zero());
        assert_eq!(intent.forward, 1.0);
        assert_eq!(intent.strafe, -1.0);
    }

    #[test]
    fn movement_intent_opposite_keys_cancel_to_zero() {
        // W+S sostenidas simultáneamente: forward_input = 1.0 - 1.0.
        let intent = MovementIntent {
            forward: 1.0 - 1.0,
            strafe: 0.0,
        };

        assert!(intent.is_zero());

        // A+D sostenidas simultáneamente.
        let intent = MovementIntent {
            forward: 0.0,
            strafe: 1.0 - 1.0,
        };

        assert!(intent.is_zero());
    }

    #[test]
    fn rotation_and_movement_intent_are_independent_pure_computations() {
        // Prueba estructural: `rotate_angle` y la construcción de
        // `MovementIntent` no comparten ningún parámetro ni estado —
        // son funciones puras sobre entradas disjuntas. Esto es
        // exactamente lo que permite que `process_events` aplique
        // AMBAS incondicionalmente en la misma llamada (rotación
        // primero, movimiento después), sin que ninguna dependa del
        // resultado de la otra.
        let rotated = rotate_angle(0.0, 50.0, MOUSE_SENSITIVITY);

        let intent = MovementIntent {
            forward: 1.0,
            strafe: 1.0,
        };

        assert_ne!(rotated, 0.0);
        assert!(!intent.is_zero());
    }
}
