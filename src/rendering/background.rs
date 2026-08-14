use super::framebuffer::Framebuffer;
use crate::world::LevelTheme;
use raylib::prelude::Color;

/// Paleta de cielo/suelo resuelta para un `LevelTheme` concreto.
///
/// Metadatos de la capa de RENDERING únicamente: `LevelTheme` (en
/// `world::level_manager`) no conoce `Color`; esta traducción vive
/// aquí, no en el dominio.
struct BackgroundPalette {
    ceiling_top: Color,
    ceiling_horizon: Color,
    floor_near: Color,
    floor_far: Color,
    floor_accent: Color,
}

/// Apariencia plana previa a Tarea 33, preservada EXACTAMENTE para
/// Black Club/House of Cards hasta que sus propias tareas (T34/T35)
/// definan su paleta final.
const LEGACY_CEILING: Color = Color::new(28, 20, 24, 255);
const LEGACY_FLOOR: Color = Color::new(12, 12, 16, 255);

/// Traduce la identidad semántica de tema hacia su paleta concreta
/// de colores. Única correspondencia tema -> colores del proyecto.
fn palette_for_theme(theme: LevelTheme) -> BackgroundPalette {
    match theme {
        LevelTheme::CrimsonEntrance => BackgroundPalette {
            ceiling_top: Color::new(0x05, 0x05, 0x05, 255),
            ceiling_horizon: Color::new(0x3A, 0x07, 0x0A, 255),
            floor_near: Color::new(0x25, 0x25, 0x25, 255),
            floor_far: Color::new(0x1A, 0x1A, 0x1A, 255),
            floor_accent: Color::new(0x4A, 0x0B, 0x10, 255),
        },

        // Black Club y House of Cards conservan la apariencia previa
        // a Tarea 33 sin cambios: `ceiling_top == ceiling_horizon` y
        // `floor_near == floor_far == floor_accent` colapsan la
        // interpolación/las bandas de acento a un color plano
        // idéntico al que ya dibujaba `draw_background` antes de
        // existir esta paleta.
        LevelTheme::BlackClub | LevelTheme::HouseOfCards => BackgroundPalette {
            ceiling_top: LEGACY_CEILING,
            ceiling_horizon: LEGACY_CEILING,
            floor_near: LEGACY_FLOOR,
            floor_far: LEGACY_FLOOR,
            floor_accent: LEGACY_FLOOR,
        },
    }
}

/// Interpola un único canal de color linealmente entre `a` y `b`.
///
/// `t` se recorta a `[0.0, 1.0]` de forma defensiva. `t == 0.0`
/// retorna exactamente `a`; `t == 1.0` retorna exactamente `b`
/// (sin arrastre de error de redondeo, porque `a + (b - a) * t`
/// se evalúa exactamente en esos extremos).
fn lerp_channel(a: u8, b: u8, t: f32) -> u8 {
    let t = t.clamp(0.0, 1.0);

    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

/// Interpola un color RGB completo (alfa fijo en 255) entre `a` y
/// `b`. Función pura, sin dependencia de `Framebuffer`/Raylib.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        lerp_channel(a.r, b.r, t),
        lerp_channel(a.g, b.g, t),
        lerp_channel(a.b, b.b, t),
        255,
    )
}

/// Separación, en filas de pantalla, entre bandas de acento del
/// suelo. Deliberadamente disperso (no es una cuadrícula regular
/// densa) y estático: no depende de la posición del jugador ni del
/// tiempo, por lo que no anima ni parpadea.
const FLOOR_ACCENT_ROW_PERIOD: i32 = 17;

/// Dibuja el cielo y el suelo de la vista 3D según la paleta
/// resuelta para `theme`.
///
/// El cielo interpola linealmente desde `ceiling_top` (borde
/// superior de pantalla) hasta `ceiling_horizon` (línea de
/// horizonte); el suelo interpola desde `floor_near` (horizonte)
/// hacia `floor_far` (borde inferior), con bandas horizontales
/// estáticas y dispersas de `floor_accent`. Para Black Club/House of
/// Cards esto colapsa exactamente al color plano previo a Tarea 33
/// (§`palette_for_theme`).
pub(super) fn draw_background(framebuffer: &mut Framebuffer, theme: LevelTheme) {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let half_height = height / 2;

    let palette = palette_for_theme(theme);

    for y in 0..half_height {
        let t = if half_height > 0 {
            y as f32 / half_height as f32
        } else {
            0.0
        };

        let color = lerp_color(palette.ceiling_top, palette.ceiling_horizon, t);

        framebuffer.set_current_color(color);

        for x in 0..width {
            framebuffer.point(x, y);
        }
    }

    let floor_span = (height - half_height).max(1);

    for y in half_height..height {
        let t = (y - half_height) as f32 / floor_span as f32;

        let color = if y % FLOOR_ACCENT_ROW_PERIOD == 0 {
            palette.floor_accent
        } else {
            lerp_color(palette.floor_near, palette.floor_far, t)
        };

        framebuffer.set_current_color(color);

        for x in 0..width {
            framebuffer.point(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_endpoint_zero_returns_the_first_color_exactly() {
        let a = Color::new(5, 5, 5, 255);
        let b = Color::new(58, 7, 10, 255);

        assert_eq!(lerp_color(a, b, 0.0), a);
    }

    #[test]
    fn lerp_endpoint_one_returns_the_second_color_exactly() {
        let a = Color::new(5, 5, 5, 255);
        let b = Color::new(58, 7, 10, 255);

        assert_eq!(lerp_color(a, b, 1.0), b);
    }

    #[test]
    fn lerp_midpoint_lies_between_endpoints_componentwise() {
        let a = Color::new(5, 5, 5, 255);
        let b = Color::new(58, 7, 10, 255);

        let mid = lerp_color(a, b, 0.5);

        assert!(mid.r >= a.r.min(b.r) && mid.r <= a.r.max(b.r));
        assert!(mid.g >= a.g.min(b.g) && mid.g <= a.g.max(b.g));
        assert!(mid.b >= a.b.min(b.b) && mid.b <= a.b.max(b.b));
    }

    #[test]
    fn crimson_entrance_ceiling_top_is_the_exact_planned_color() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(palette.ceiling_top, Color::new(0x05, 0x05, 0x05, 255));
    }

    #[test]
    fn crimson_entrance_ceiling_horizon_is_the_exact_planned_color() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(palette.ceiling_horizon, Color::new(0x3A, 0x07, 0x0A, 255));
    }

    #[test]
    fn crimson_entrance_floor_palette_matches_the_plan_maestro_family() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(palette.floor_near, Color::new(0x25, 0x25, 0x25, 255));
        assert_eq!(palette.floor_far, Color::new(0x1A, 0x1A, 0x1A, 255));
        assert_eq!(palette.floor_accent, Color::new(0x4A, 0x0B, 0x10, 255));
    }

    #[test]
    fn black_club_still_selects_the_legacy_flat_background() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_eq!(palette.ceiling_top, LEGACY_CEILING);
        assert_eq!(palette.ceiling_horizon, LEGACY_CEILING);
        assert_eq!(palette.floor_near, LEGACY_FLOOR);
        assert_eq!(palette.floor_far, LEGACY_FLOOR);
        assert_eq!(palette.floor_accent, LEGACY_FLOOR);
    }

    #[test]
    fn house_of_cards_still_selects_the_legacy_flat_background() {
        let palette = palette_for_theme(LevelTheme::HouseOfCards);

        assert_eq!(palette.ceiling_top, LEGACY_CEILING);
        assert_eq!(palette.ceiling_horizon, LEGACY_CEILING);
        assert_eq!(palette.floor_near, LEGACY_FLOOR);
        assert_eq!(palette.floor_far, LEGACY_FLOOR);
        assert_eq!(palette.floor_accent, LEGACY_FLOOR);
    }
}
