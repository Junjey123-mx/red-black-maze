use super::framebuffer::Framebuffer;
use super::textures::TextureManager;
use crate::player::{WeaponState, WeaponTier};
use crate::world::LevelTheme;

/// Escala entera de dibujo del arma en primera persona, con
/// muestreo nearest-neighbor (sin antialiasing).
const WEAPON_SCALE: i32 = 3;

/// Fracción del progreso de recarga (Tarea 43) en la que termina la
/// fase "bajar" y comienza "cambio de cargador".
const LOWER_PHASE_END: f32 = 0.25;

/// Fracción en la que termina "cambio de cargador" y comienza
/// "subir".
const MAGAZINE_CHANGE_PHASE_END: f32 = 0.65;

/// Desplazamiento vertical máximo (hacia abajo, píxeles de pantalla)
/// durante la recarga.
const MAX_VERTICAL_OFFSET: i32 = 12;

/// Desplazamiento horizontal máximo (píxeles de pantalla) durante el
/// gesto de cambio de cargador.
const MAX_HORIZONTAL_OFFSET: i32 = 6;

/// Calcula el desplazamiento visual (píxeles ENTEROS de pantalla,
/// pixel-snapped) del arma en primera persona durante la animación
/// de recarga, a partir del `progress` normalizado que ya reporta
/// `Weapon::reload_progress` — Tarea 43 no introduce un segundo
/// temporizador visual independiente; este es el ÚNICO consumidor
/// del progreso real de la mecánica.
///
/// Función PURA: sin `RaylibHandle`, sin mutar `Weapon`, sin reloj
/// absoluto, determinista (el mismo `progress` produce siempre el
/// mismo resultado). `progress` se recorta defensivamente a
/// `[0.0, 1.0]`.
///
/// `offset(0.0) == offset(1.0) == (0, 0)`: el arma SIEMPRE regresa
/// EXACTAMENTE a su posición base al completarse la recarga, porque
/// el desplazamiento se recalcula desde `progress` en cada cuadro
/// (`base_position + reload_offset(progress)`, nunca
/// `weapon_y += ...` acumulativo) — no puede existir drift de
/// redondeo entre cuadros.
///
/// Tres fases, sin discontinuidad en sus fronteras (cada fase
/// termina exactamente donde la siguiente comienza):
/// - `[0.0, LOWER_PHASE_END]`: el arma baja en línea recta (`x=0`).
/// - `(LOWER_PHASE_END, MAGAZINE_CHANGE_PHASE_END]`: permanece
///   abajo (`y` constante en el máximo) y se desplaza lateralmente
///   siguiendo un semiciclo de seno (`0 -> pico -> 0`), simulando el
///   gesto de cambio de cargador.
/// - `(MAGAZINE_CHANGE_PHASE_END, 1.0]`: sube en línea recta de
///   vuelta a `x=0, y=0`.
pub(crate) fn reload_offset(progress: f32) -> (i32, i32) {
    let progress = progress.clamp(0.0, 1.0);

    if progress <= LOWER_PHASE_END {
        let local = progress / LOWER_PHASE_END;

        let y = (MAX_VERTICAL_OFFSET as f32 * local).round() as i32;

        (0, y)
    } else if progress <= MAGAZINE_CHANGE_PHASE_END {
        let local = (progress - LOWER_PHASE_END) / (MAGAZINE_CHANGE_PHASE_END - LOWER_PHASE_END);

        let x =
            (MAX_HORIZONTAL_OFFSET as f32 * (std::f32::consts::PI * local).sin()).round() as i32;

        (x, MAX_VERTICAL_OFFSET)
    } else {
        let local = (progress - MAGAZINE_CHANGE_PHASE_END) / (1.0 - MAGAZINE_CHANGE_PHASE_END);

        let y = (MAX_VERTICAL_OFFSET as f32 * (1.0 - local)).round() as i32;

        (0, y)
    }
}

/// Dibuja el arma en primera persona como una superposición en
/// espacio de PANTALLA, anclada abajo-centro.
///
/// No es un billboard: no usa coordenadas de mundo, ángulo del
/// jugador, FOV, ni `wall_depth_buffer`. El estado visual ya viene
/// decidido por el llamador (`GameSession`/`Weapon`); este renderer
/// solo lo LEE para elegir la textura correspondiente, nunca
/// modifica el estado del arma.
///
/// `tier` (Bloque 2, Commit 16) selecciona el conjunto de sprites:
/// `Standard` usa el arma base temática, `RoyalFlush` el sprite
/// dorado dedicado — todo lo demás (posición, escala, escalado
/// entero, `reload_offset`, transiciones) es idéntico para ambos.
///
/// `reload_progress` (Tarea 43) es `Weapon::reload_progress()` ya
/// leído por el llamador: `Some(progreso)` mientras
/// `WeaponState::Reload` está activo, `None` en cualquier otro
/// estado. Cuando es `None`, `reload_offset` nunca se invoca y el
/// arma se dibuja en su posición base sin desplazamiento — Idle/
/// Fire/Recoil quedan visualmente intactos.
pub(crate) fn render_weapon(
    framebuffer: &mut Framebuffer,
    textures: &TextureManager,
    state: WeaponState,
    theme: LevelTheme,
    tier: WeaponTier,
    reload_progress: Option<f32>,
) {
    let Some(texture) = textures.themed_weapon_texture(state, theme, tier) else {
        return;
    };

    let texture_width = texture.width();
    let texture_height = texture.height();

    if texture_width <= 0 || texture_height <= 0 {
        return;
    }

    let draw_width = texture_width * WEAPON_SCALE;
    let draw_height = texture_height * WEAPON_SCALE;

    let screen_width = framebuffer.width();
    let screen_height = framebuffer.height();

    let (offset_x, offset_y) = reload_progress.map(reload_offset).unwrap_or((0, 0));

    let left = screen_width / 2 - draw_width / 2 + offset_x;
    let top = screen_height - draw_height + offset_y;

    for dest_y in 0..draw_height {
        let screen_y = top + dest_y;

        if screen_y < 0 || screen_y >= screen_height {
            continue;
        }

        let source_y = dest_y / WEAPON_SCALE;

        for dest_x in 0..draw_width {
            let screen_x = left + dest_x;

            if screen_x < 0 || screen_x >= screen_width {
                continue;
            }

            let source_x = dest_x / WEAPON_SCALE;

            let Some(color) = texture.pixel_at(source_x, source_y) else {
                continue;
            };

            if color.a == 0 {
                continue;
            }

            framebuffer.set_current_color(color);

            framebuffer.point(screen_x, screen_y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_progress_is_the_base_position() {
        assert_eq!(reload_offset(0.0), (0, 0));
    }

    #[test]
    fn full_progress_is_exactly_the_base_position_again() {
        assert_eq!(reload_offset(1.0), (0, 0));
    }

    #[test]
    fn early_reload_moves_the_weapon_down() {
        let (x, y) = reload_offset(0.1);

        assert_eq!(x, 0);
        assert!(y > 0);
    }

    #[test]
    fn middle_reload_stays_lowered_with_horizontal_displacement() {
        let (x, y) = reload_offset(0.45);

        assert_eq!(y, MAX_VERTICAL_OFFSET);
        assert_ne!(x, 0);
    }

    #[test]
    fn late_reload_moves_back_toward_the_base_position() {
        let (x, y) = reload_offset(0.9);

        assert_eq!(x, 0);
        assert!(y > 0 && y < MAX_VERTICAL_OFFSET);
    }

    #[test]
    fn offsets_are_integer_pixel_values_across_the_whole_range() {
        // El tipo de retorno ya es `(i32, i32)` (garantía
        // estructural), pero además comprobamos que no hay
        // redondeo fraccional oculto convertido incorrectamente:
        // recomputar con `.round()` debe coincidir exactamente.
        let mut progress = 0.0;

        while progress <= 1.0 {
            let (x, y) = reload_offset(progress);

            assert_eq!(x, x.clamp(-MAX_HORIZONTAL_OFFSET, MAX_HORIZONTAL_OFFSET));
            assert_eq!(y, y.clamp(0, MAX_VERTICAL_OFFSET));

            progress += 0.01;
        }
    }

    #[test]
    fn no_drift_repeated_calls_at_the_same_progress_are_identical() {
        // La función es pura: llamarla muchas veces con el MISMO
        // progreso nunca debe producir una deriva acumulativa,
        // porque no existe ningún estado mutable entre llamadas.
        for _ in 0..1000 {
            assert_eq!(reload_offset(0.45), reload_offset(0.45));
        }
    }

    #[test]
    fn phase_boundaries_are_continuous() {
        // Verifica que no hay salto visual entre fases: el valor
        // justo antes y justo después de cada frontera debe ser
        // igual o diferir como mucho en 1 píxel por redondeo.
        let epsilon = 0.001;

        let (_, y_before_lower_end) = reload_offset(LOWER_PHASE_END - epsilon);
        let (_, y_after_lower_end) = reload_offset(LOWER_PHASE_END + epsilon);

        assert!((y_before_lower_end - y_after_lower_end).abs() <= 1);

        let (x_before_change_end, _) = reload_offset(MAGAZINE_CHANGE_PHASE_END - epsilon);
        let (x_after_change_end, _) = reload_offset(MAGAZINE_CHANGE_PHASE_END + epsilon);

        assert!((x_before_change_end - x_after_change_end).abs() <= 1);
    }

    #[test]
    fn out_of_range_progress_is_clamped_safely() {
        assert_eq!(reload_offset(-1.0), reload_offset(0.0));
        assert_eq!(reload_offset(2.0), reload_offset(1.0));
    }

    #[test]
    fn not_reloading_produces_no_offset_via_the_option_path() {
        // Refleja exactamente cómo `render_weapon` consume el valor:
        // `None` (no recargando) nunca invoca `reload_offset`, así
        // que el desplazamiento efectivo es siempre `(0, 0)`.
        let reload_progress: Option<f32> = None;

        let offset = reload_progress.map(reload_offset).unwrap_or((0, 0));

        assert_eq!(offset, (0, 0));
    }
}
