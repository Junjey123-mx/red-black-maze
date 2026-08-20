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

    /// Parada intermedia opcional del degradado de cielo (banda de
    /// humo). `None` preserva EXACTAMENTE el degradado de dos
    /// paradas que ya usaban Crimson Entrance/el fondo heredado: es
    /// literalmente la misma llamada a `lerp_color` de antes de
    /// Tarea 34, sin ninguna rama nueva para esos temas.
    ceiling_mid: Option<Color>,

    ceiling_horizon: Color,
    floor_near: Color,
    floor_far: Color,
    floor_accent: Color,

    /// Estrategia de dibujo del suelo. Pertenece EXCLUSIVAMENTE a
    /// esta capa de rendering (nunca a `world`/`LevelTheme`): es
    /// pura decisión de presentación.
    floor_pattern: FloorPattern,
}

/// Estrategia de dibujo del suelo de la vista 3D.
///
/// `Bands`: el mecanismo original de Tarea 33 (interpolación
/// horizonte->fondo + bandas horizontales dispersas de acento).
/// `GeometricMosaic`: patrón ornamental estático adicional para
/// House of Cards (Tarea 35), basado únicamente en coordenadas de
/// píxel de pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloorPattern {
    Bands,
    GeometricMosaic,
}

/// Traduce la identidad semántica de tema hacia su paleta concreta
/// de colores. Única correspondencia tema -> colores del proyecto.
fn palette_for_theme(theme: LevelTheme) -> BackgroundPalette {
    match theme {
        LevelTheme::CrimsonEntrance => BackgroundPalette {
            ceiling_top: Color::new(0x05, 0x05, 0x05, 255),
            ceiling_mid: None,
            ceiling_horizon: Color::new(0x3A, 0x07, 0x0A, 255),
            floor_near: Color::new(0x25, 0x25, 0x25, 255),
            floor_far: Color::new(0x1A, 0x1A, 0x1A, 255),
            floor_accent: Color::new(0x4A, 0x0B, 0x10, 255),
            floor_pattern: FloorPattern::Bands,
        },

        // Tarea 40: negro casi absoluto -> naranja quemado muy
        // oscuro (parada intermedia) -> naranja quemado de horizonte
        // — los tres valores de referencia exactos del Plan Maestro
        // (`#030303`/`#241000`/`#5A2000`). El techo sigue siendo
        // MAYORMENTE oscuro (la parada intermedia solo se alcanza a
        // mitad de la mitad superior de pantalla, y el naranja solo
        // domina cerca del horizonte), preservando la sensación
        // claustrofóbica de Black Club en vez de un cielo naranja
        // plano y brillante. El suelo permanece oscuro/carbón con un
        // matiz cálido sutil; el acento disperso de bandas usa el
        // mismo naranja oscuro `accent_dark` de `ThemePalette`
        // (`#A53600`), como las "líneas naranja" que pide la tarea,
        // en vez de un suelo naranja brillante uniforme.
        LevelTheme::BlackClub => BackgroundPalette {
            ceiling_top: Color::new(0x03, 0x03, 0x03, 255),
            ceiling_mid: Some(Color::new(0x24, 0x10, 0x00, 255)),
            ceiling_horizon: Color::new(0x5A, 0x20, 0x00, 255),
            floor_near: Color::new(0x28, 0x22, 0x1C, 255),
            floor_far: Color::new(0x16, 0x12, 0x0E, 255),
            floor_accent: Color::new(0xA5, 0x36, 0x00, 255),
            floor_pattern: FloorPattern::Bands,
        },

        // Tarea 41: casi negro -> violeta muy oscuro (parada
        // intermedia) -> violeta prominente en el horizonte — los
        // tres valores de referencia exactos del Plan Maestro
        // (`#050305`/`#260832`/`#52106A`, este último igual a
        // `accent_dark` de `ThemePalette`). El mosaico geométrico
        // ornamental (único tema que lo usa, sin cambios de patrón/
        // densidad/geometría) sigue reflejando que House of Cards es
        // visualmente más rico que Crimson Entrance/Black Club; solo
        // su familia cromática cambia a negro-violeta.
        LevelTheme::HouseOfCards => BackgroundPalette {
            ceiling_top: Color::new(0x05, 0x03, 0x05, 255),
            ceiling_mid: Some(Color::new(0x26, 0x08, 0x32, 255)),
            ceiling_horizon: Color::new(0x52, 0x10, 0x6A, 255),
            floor_near: Color::new(0x24, 0x1C, 0x28, 255),
            floor_far: Color::new(0x14, 0x10, 0x16, 255),
            floor_accent: Color::new(0x52, 0x10, 0x6A, 255),
            floor_pattern: FloorPattern::GeometricMosaic,
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

/// Separación, en píxeles de pantalla, de la retícula diagonal del
/// mosaico geométrico de House of Cards.
const MOSAIC_PERIOD: i32 = 24;

/// Patrón ornamental estático de House of Cards: marca únicamente
/// las intersecciones de dos familias de líneas diagonales
/// espaciadas `MOSAIC_PERIOD` píxeles entre sí, produciendo una
/// retícula dispersa de puntos (motivo tipo "diamante de carta") en
/// vez de una cuadrícula densa y brillante.
///
/// Función pura de coordenadas de PANTALLA únicamente: no depende de
/// la posición del jugador, del tiempo ni de ninguna fuente de
/// aleatoriedad, por lo que no anima ni parpadea.
fn is_mosaic_accent_pixel(x: i32, y: i32) -> bool {
    (x + y).rem_euclid(MOSAIC_PERIOD) == 0 && (x - y).rem_euclid(MOSAIC_PERIOD) == 0
}

/// Dibuja el cielo y el suelo de la vista 3D según la paleta
/// resuelta para `theme`.
///
/// El cielo interpola linealmente desde `ceiling_top` (borde
/// superior de pantalla) hasta `ceiling_horizon` (línea de
/// horizonte), con una parada intermedia opcional `ceiling_mid`; el
/// suelo interpola desde `floor_near` (horizonte) hacia `floor_far`
/// (borde inferior), con acento aplicado según `floor_pattern`:
/// bandas horizontales dispersas (`Bands`) o retícula diagonal
/// dispersa (`GeometricMosaic`, exclusiva de House of Cards).
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

        let color = match palette.ceiling_mid {
            /*
             * Degradado de tres paradas (0.0 -> 0.5 -> 1.0): negro
             * casi puro -> humo -> rojo vino casi negro. Es la misma
             * `lerp_color` pura de dos argumentos, aplicada dos
             * veces sobre la mitad correspondiente de `t`.
             */
            Some(mid) if t < 0.5 => lerp_color(palette.ceiling_top, mid, t / 0.5),

            Some(mid) => lerp_color(mid, palette.ceiling_horizon, (t - 0.5) / 0.5),

            /*
             * Sin parada intermedia: degradado clásico de dos
             * paradas, IDÉNTICO al que usaban Crimson Entrance y el
             * fondo heredado antes de Tarea 34.
             */
            None => lerp_color(palette.ceiling_top, palette.ceiling_horizon, t),
        };

        framebuffer.set_current_color(color);

        for x in 0..width {
            framebuffer.point(x, y);
        }
    }

    let floor_span = (height - half_height).max(1);

    for y in half_height..height {
        let t = (y - half_height) as f32 / floor_span as f32;

        match palette.floor_pattern {
            /*
             * Mecanismo ORIGINAL de Tarea 33, sin cambios: una única
             * llamada a `set_current_color` por fila.
             */
            FloorPattern::Bands => {
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

            /*
             * Mosaico geométrico: cada píxel puede requerir un color
             * distinto (base o acento disperso), así que el color se
             * decide dentro del bucle de columnas.
             */
            FloorPattern::GeometricMosaic => {
                let base_color = lerp_color(palette.floor_near, palette.floor_far, t);

                for x in 0..width {
                    let color = if is_mosaic_accent_pixel(x, y) {
                        palette.floor_accent
                    } else {
                        base_color
                    };

                    framebuffer.set_current_color(color);

                    framebuffer.point(x, y);
                }
            }
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
    fn house_of_cards_ceiling_matches_the_exact_planned_three_stop_gradient() {
        let palette = palette_for_theme(LevelTheme::HouseOfCards);

        assert_eq!(palette.ceiling_top, Color::new(0x05, 0x03, 0x05, 255));
        assert_eq!(palette.ceiling_mid, Some(Color::new(0x26, 0x08, 0x32, 255)));
        assert_eq!(palette.ceiling_horizon, Color::new(0x52, 0x10, 0x6A, 255));
    }

    #[test]
    fn house_of_cards_floor_palette_matches_the_plan_maestro_family() {
        let palette = palette_for_theme(LevelTheme::HouseOfCards);

        assert_eq!(palette.floor_near, Color::new(0x24, 0x1C, 0x28, 255));
        assert_eq!(palette.floor_far, Color::new(0x14, 0x10, 0x16, 255));
        assert_eq!(palette.floor_accent, Color::new(0x52, 0x10, 0x6A, 255));
    }

    #[test]
    fn house_of_cards_floor_uses_the_geometric_mosaic_pattern() {
        let palette = palette_for_theme(LevelTheme::HouseOfCards);

        assert_eq!(palette.floor_pattern, FloorPattern::GeometricMosaic);
    }

    #[test]
    fn house_of_cards_background_no_longer_uses_any_red_value() {
        let palette = palette_for_theme(LevelTheme::HouseOfCards);

        assert_ne!(palette.ceiling_horizon, Color::new(0x6B, 0x0D, 0x14, 255));
        assert_ne!(palette.floor_accent, Color::new(0x4C, 0x0B, 0x10, 255));

        // "Violeta": azul dominante sobre rojo, rojo por encima de
        // verde — la misma relación que la familia de acento de
        // `ThemePalette` para House of Cards.
        assert!(palette.ceiling_horizon.b > palette.ceiling_horizon.r);
        assert!(palette.ceiling_horizon.r > palette.ceiling_horizon.g);
        assert!(palette.floor_accent.b > palette.floor_accent.r);
        assert!(palette.floor_accent.r > palette.floor_accent.g);
    }

    #[test]
    fn house_of_cards_ceiling_stays_mostly_dark_before_the_horizon() {
        // Igual que Black Club: el techo NO debe volverse violeta
        // plano y brillante. A un cuarto del camino hacia la parada
        // intermedia, el color debe seguir siendo abrumadoramente
        // oscuro.
        let top = Color::new(0x05, 0x03, 0x05, 255);
        let mid = Color::new(0x26, 0x08, 0x32, 255);

        let quarter = lerp_color(top, mid, 0.25);

        assert!(quarter.r < 20);
        assert!(quarter.g < 10);
        assert!(quarter.b < 25);
    }

    #[test]
    fn crimson_and_black_club_floors_still_use_plain_bands() {
        assert_eq!(
            palette_for_theme(LevelTheme::CrimsonEntrance).floor_pattern,
            FloorPattern::Bands
        );

        assert_eq!(
            palette_for_theme(LevelTheme::BlackClub).floor_pattern,
            FloorPattern::Bands
        );
    }

    #[test]
    fn mosaic_accent_pixels_are_sparse_not_a_dense_grid() {
        let mut accent_count = 0;

        for y in 0..100 {
            for x in 0..100 {
                if is_mosaic_accent_pixel(x, y) {
                    accent_count += 1;
                }
            }
        }

        // 10 000 muestras: la retícula dispersa debe cubrir
        // claramente menos de una cuadrícula densa (que marcaría
        // ~2500+ píxeles con este mismo período en una sola familia
        // de diagonales); aquí exigimos varios órdenes de magnitud
        // menos para confirmar que es un acento disperso, no un
        // patrón dominante.
        assert!(accent_count > 0);
        assert!(accent_count < 200);
    }

    #[test]
    fn black_club_uses_the_neon_orange_background_after_task_40() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_eq!(palette.ceiling_top, Color::new(0x03, 0x03, 0x03, 255));
        assert_eq!(palette.ceiling_mid, Some(Color::new(0x24, 0x10, 0x00, 255)));
        assert_eq!(palette.ceiling_horizon, Color::new(0x5A, 0x20, 0x00, 255));
        assert_eq!(palette.floor_near, Color::new(0x28, 0x22, 0x1C, 255));
        assert_eq!(palette.floor_far, Color::new(0x16, 0x12, 0x0E, 255));
        assert_eq!(palette.floor_accent, Color::new(0xA5, 0x36, 0x00, 255));
    }

    #[test]
    fn black_club_background_no_longer_uses_any_red_value() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_ne!(palette.ceiling_horizon, Color::new(0x22, 0x04, 0x06, 255));
        assert_ne!(palette.floor_accent, Color::new(0x3E, 0x0A, 0x0F, 255));

        // "Naranja quemado": rojo dominante, azul en cero, verde
        // intermedio — la misma relación que la familia de acento de
        // `ThemePalette` para Black Club.
        assert_eq!(palette.ceiling_horizon.b, 0);
        assert!(palette.ceiling_horizon.r > palette.ceiling_horizon.g);
        assert_eq!(palette.floor_accent.b, 0);
        assert!(palette.floor_accent.r > palette.floor_accent.g);
    }

    #[test]
    fn crimson_entrance_palette_remains_exact_after_house_of_cards_changes() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(palette.ceiling_top, Color::new(0x05, 0x05, 0x05, 255));
        assert_eq!(palette.ceiling_mid, None);
        assert_eq!(palette.ceiling_horizon, Color::new(0x3A, 0x07, 0x0A, 255));
        assert_eq!(palette.floor_near, Color::new(0x25, 0x25, 0x25, 255));
        assert_eq!(palette.floor_far, Color::new(0x1A, 0x1A, 0x1A, 255));
        assert_eq!(palette.floor_accent, Color::new(0x4A, 0x0B, 0x10, 255));
    }

    #[test]
    fn crimson_entrance_ceiling_has_no_mid_stop() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(palette.ceiling_mid, None);
    }

    #[test]
    fn black_club_ceiling_matches_the_exact_planned_three_stop_gradient() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_eq!(palette.ceiling_top, Color::new(0x03, 0x03, 0x03, 255));
        assert_eq!(palette.ceiling_mid, Some(Color::new(0x24, 0x10, 0x00, 255)));
        assert_eq!(palette.ceiling_horizon, Color::new(0x5A, 0x20, 0x00, 255));
    }

    #[test]
    fn black_club_floor_palette_matches_the_plan_maestro_family() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_eq!(palette.floor_near, Color::new(0x28, 0x22, 0x1C, 255));
        assert_eq!(palette.floor_far, Color::new(0x16, 0x12, 0x0E, 255));
        assert_eq!(palette.floor_accent, Color::new(0xA5, 0x36, 0x00, 255));
    }

    #[test]
    fn black_club_ceiling_stays_mostly_dark_before_the_horizon() {
        // La tarea exige que el cielo NO se convierta en naranja
        // plano y brillante: la mayor parte de la mitad superior de
        // pantalla debe seguir siendo oscura, con el naranja
        // apareciendo progresivamente solo cerca del horizonte.
        let top = Color::new(0x03, 0x03, 0x03, 255);
        let mid = Color::new(0x24, 0x10, 0x00, 255);

        // A un cuarto del camino hacia la parada intermedia (t=0.125
        // sobre el primer tramo del degradado de tres paradas), el
        // color debe seguir siendo abrumadoramente oscuro: el canal
        // rojo, el más alto de los tres, no debe superar una
        // fracción pequeña de 255.
        let quarter = lerp_color(top, mid, 0.25);

        assert!(quarter.r < 20);
    }
}
