use raylib::prelude::Color;

use super::framebuffer::Framebuffer;

/// Rojo del dithering de daño. Identidad GLOBAL (no depende de
/// `LevelTheme`): el flash representa daño al JUGADOR, no la
/// identidad cromática del nivel activo, así que Crimson/Black
/// Club/House of Cards muestran exactamente el mismo rojo.
const FLASH_COLOR: Color = Color::new(200, 20, 30, 255);

/// Periodo (en píxeles) del patrón disperso: se pinta un píxel de
/// cada `SPARSE_PERIOD * SPARSE_PERIOD`, mucho menos denso que el
/// tablero de ajedrez de `ui::pause` (que oscurece al 50% porque
/// pausa oculta gameplay completo). El flash de daño debe ser
/// notorio pero breve, dejando el gameplay claramente visible
/// debajo.
const SPARSE_PERIOD: i32 = 3;

/// Dibuja el flash de daño al jugador (Tarea 45): un dithering rojo
/// DISPERSO sobre TODO lo ya dibujado en `framebuffer` (mundo/arma/
/// HUD/minimapa/FPS, ya renderizados por el llamador), en vez de
/// mezcla alfa/blur/pantalla roja opaca — misma familia de técnica
/// retro que `ui::pause::draw_dither_overlay`, pero deliberadamente
/// mucho menos densa (un píxel de cada nueve, no de cada dos), para
/// que la escena permanezca claramente visible durante el breve
/// destello.
///
/// Puramente de presentación: no lee ningún timer ni decide cuánto
/// debe durar el flash — el llamador (`App`) ya decidió que debe
/// dibujarse ESTE cuadro consultando `GameSession::is_hit_flash_active`.
pub(crate) fn render_hit_flash_overlay(framebuffer: &mut Framebuffer) {
    framebuffer.set_current_color(FLASH_COLOR);

    let width = framebuffer.width();

    let height = framebuffer.height();

    for y in 0..height {
        for x in 0..width {
            if x % SPARSE_PERIOD == 0 && y % SPARSE_PERIOD == 0 {
                framebuffer.point(x, y);
            }
        }
    }
}
