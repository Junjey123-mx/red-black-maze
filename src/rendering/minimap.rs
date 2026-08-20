use raylib::prelude::Color;

use super::framebuffer::Framebuffer;
use super::palette::palette_for_theme;
use crate::player::Player;
use crate::world::{Level, LevelTheme, Tile};

/// Margen exterior entre el borde del framebuffer y la caja del
/// minimapa.
const OUTER_MARGIN: f32 = 12.0;

/// Área de contenido (mapa, sin borde/padding) máxima permitida,
/// independiente de las dimensiones del nivel.
const MAX_CONTENT_WIDTH: f32 = 180.0;
const MAX_CONTENT_HEIGHT: f32 = 140.0;

/// Separación entre el borde de la caja y las celdas del mapa.
const PADDING: f32 = 6.0;

/// Grosor del borde de la caja del minimapa.
const BORDER_THICKNESS: f32 = 2.0;

/// Longitud, en píxeles de minimapa, del indicador de dirección.
const DIRECTION_LINE_LENGTH: f32 = 12.0;

/// Radio del marcador del jugador, en píxeles de minimapa.
const PLAYER_MARKER_RADIUS: i32 = 3;

/// Neutro, independiente del `LevelTheme` activo.
const BACKGROUND_COLOR: Color = Color::new(10, 10, 14, 255);

/// Neutro (marfil), independiente del `LevelTheme` activo: el
/// marcador del jugador no forma parte de la identidad de acento del
/// nivel.
const PLAYER_COLOR: Color = Color::new(235, 228, 210, 255);

/// Disposición geométrica ya resuelta del minimapa para un
/// framebuffer y un nivel concretos.
///
/// Cálculo puro, sin dependencias de Raylib/Framebuffer/Level: solo
/// primitivos, para poder probarse sin abrir una ventana.
struct MinimapLayout {
    box_left: i32,
    box_top: i32,
    box_right: i32,
    box_bottom: i32,
    map_left: f32,
    map_top: f32,
    cell_scale: f32,
}

/// Calcula la disposición del minimapa: caja anclada arriba-derecha
/// del framebuffer, con un área de contenido escalada uniformemente
/// (preservando aspecto) para que el nivel COMPLETO quepa dentro de
/// un máximo `MAX_CONTENT_WIDTH x MAX_CONTENT_HEIGHT`, recortado
/// además contra las dimensiones reales del framebuffer.
///
/// Retorna `None` si no hay espacio útil (framebuffer extremadamente
/// pequeño, o nivel sin dimensiones válidas), para que el llamador
/// pueda degradar de forma segura sin dibujar nada en vez de entrar
/// en pánico.
fn compute_layout(
    framebuffer_width: i32,
    framebuffer_height: i32,
    level_width: usize,
    level_height: usize,
) -> Option<MinimapLayout> {
    if level_width == 0 || level_height == 0 {
        return None;
    }

    let framebuffer_width = framebuffer_width as f32;
    let framebuffer_height = framebuffer_height as f32;

    let frame_margin = 2.0 * OUTER_MARGIN + 2.0 * PADDING + 2.0 * BORDER_THICKNESS;

    let available_content_width = MAX_CONTENT_WIDTH.min(framebuffer_width - frame_margin);

    let available_content_height = MAX_CONTENT_HEIGHT.min(framebuffer_height - frame_margin);

    if available_content_width <= 0.0 || available_content_height <= 0.0 {
        return None;
    }

    let scale_x = available_content_width / level_width as f32;

    let scale_y = available_content_height / level_height as f32;

    let cell_scale = scale_x.min(scale_y);

    if !(cell_scale > 0.0) {
        return None;
    }

    let content_width = level_width as f32 * cell_scale;

    let content_height = level_height as f32 * cell_scale;

    let box_width = content_width + 2.0 * PADDING + 2.0 * BORDER_THICKNESS;

    let box_height = content_height + 2.0 * PADDING + 2.0 * BORDER_THICKNESS;

    let box_right = framebuffer_width - OUTER_MARGIN;

    let box_left = box_right - box_width;

    let box_top = OUTER_MARGIN;

    let box_bottom = box_top + box_height;

    let map_left = box_left + BORDER_THICKNESS + PADDING;

    let map_top = box_top + BORDER_THICKNESS + PADDING;

    Some(MinimapLayout {
        box_left: box_left.round() as i32,
        box_top: box_top.round() as i32,
        box_right: box_right.round() as i32,
        box_bottom: box_bottom.round() as i32,
        map_left,
        map_top,
        cell_scale,
    })
}

/// Convierte una posición del jugador en espacio de mundo (píxeles,
/// según `block_size`) a coordenadas de pantalla dentro del área de
/// contenido del minimapa.
fn world_to_minimap(
    layout: &MinimapLayout,
    world_x: f32,
    world_y: f32,
    block_size: usize,
) -> (f32, f32) {
    let map_x = layout.map_left + (world_x / block_size as f32) * layout.cell_scale;

    let map_y = layout.map_top + (world_y / block_size as f32) * layout.cell_scale;

    (map_x, map_y)
}

/// Indica si esta clasificación semántica de celda debe dibujarse
/// como pared dentro del minimapa.
///
/// Reutiliza `Tile` como única fuente semántica; no reimplementa
/// coincidencia de caracteres crudos ni duplica la lógica de
/// colisión de `Level`/`collision.rs`.
fn is_minimap_wall(tile: Tile) -> bool {
    matches!(
        tile,
        Tile::HeartWall | Tile::DiamondWall | Tile::ClubWall | Tile::SpadeWall
    )
}

/// Rellena un rectángulo `[x0, x1) x [y0, y1)` dentro del
/// framebuffer.
///
/// Se apoya enteramente en `Framebuffer::point`, que ya recorta
/// coordenadas fuera de rango, por lo que no necesita convertir a
/// `usize` ni verificar límites por su cuenta: ninguna coordenada,
/// incluso negativa, puede producir una escritura fuera de rango.
fn fill_rect(framebuffer: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
    framebuffer.set_current_color(color);

    for y in y0..y1 {
        for x in x0..x1 {
            framebuffer.point(x, y);
        }
    }
}

/// Dibuja una línea corta mediante interpolación de paso fijo. Es
/// suficiente para el indicador de dirección (unos pocos píxeles de
/// longitud); no es un algoritmo genérico ni reutiliza el
/// raycasting para trazarla.
fn draw_line(framebuffer: &mut Framebuffer, x0: f32, y0: f32, x1: f32, y1: f32, color: Color) {
    framebuffer.set_current_color(color);

    let dx = x1 - x0;

    let dy = y1 - y0;

    let steps = dx.hypot(dy).ceil().max(1.0) as i32;

    for step in 0..=steps {
        let t = step as f32 / steps as f32;

        let x = x0 + dx * t;

        let y = y0 + dy * t;

        framebuffer.point(x.round() as i32, y.round() as i32);
    }
}

/// Dibuja al jugador como un pequeño marcador relleno, centrado en
/// `(center_x, center_y)`.
fn draw_player_marker(framebuffer: &mut Framebuffer, center_x: f32, center_y: f32) {
    framebuffer.set_current_color(PLAYER_COLOR);

    let cx = center_x.round() as i32;

    let cy = center_y.round() as i32;

    for offset_y in -PLAYER_MARKER_RADIUS..=PLAYER_MARKER_RADIUS {
        for offset_x in -PLAYER_MARKER_RADIUS..=PLAYER_MARKER_RADIUS {
            let distance_squared = offset_x * offset_x + offset_y * offset_y;

            if distance_squared <= PLAYER_MARKER_RADIUS * PLAYER_MARKER_RADIUS {
                framebuffer.point(cx + offset_x, cy + offset_y);
            }
        }
    }
}

/// Dibuja la superposición de minimapa: caja anclada arriba-derecha
/// sobre la vista `World3D`, con fondo oscuro, borde, paredes del
/// nivel completo escaladas de forma independiente a sus
/// dimensiones, y un marcador de jugador con indicador de
/// dirección.
///
/// Es puramente presentación: no modifica `Level`/`Player`, no
/// realiza colisión ni raycasting, no lee entidades, arma ni
/// entrada de teclado/mouse. El minimapa en sí NUNCA rota; solo el
/// indicador de dirección lo hace, siguiendo `player.a`.
pub(crate) fn render_minimap(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    block_size: usize,
    theme: LevelTheme,
) {
    let Some(layout) = compute_layout(
        framebuffer.width(),
        framebuffer.height(),
        level.width(),
        level.height(),
    ) else {
        return;
    };

    let palette = palette_for_theme(theme);

    fill_rect(
        framebuffer,
        layout.box_left,
        layout.box_top,
        layout.box_right,
        layout.box_bottom,
        palette.minimap_border_accent,
    );

    let border = BORDER_THICKNESS.round() as i32;

    fill_rect(
        framebuffer,
        layout.box_left + border,
        layout.box_top + border,
        layout.box_right - border,
        layout.box_bottom - border,
        BACKGROUND_COLOR,
    );

    for row in 0..level.height() {
        for column in 0..level.width() {
            let Some(tile) = level.tile_at(row, column) else {
                continue;
            };

            if !is_minimap_wall(tile) {
                continue;
            }

            let x0 = layout.map_left + column as f32 * layout.cell_scale;

            let x1 = layout.map_left + (column + 1) as f32 * layout.cell_scale;

            let y0 = layout.map_top + row as f32 * layout.cell_scale;

            let y1 = layout.map_top + (row + 1) as f32 * layout.cell_scale;

            fill_rect(
                framebuffer,
                x0.round() as i32,
                y0.round() as i32,
                x1.round() as i32,
                y1.round() as i32,
                palette.minimap_wall_accent,
            );
        }
    }

    let (player_x, player_y) = world_to_minimap(&layout, player.pos.x, player.pos.y, block_size);

    let direction_x = player_x + player.a.cos() * DIRECTION_LINE_LENGTH;

    let direction_y = player_y + player.a.sin() * DIRECTION_LINE_LENGTH;

    draw_line(
        framebuffer,
        player_x,
        player_y,
        direction_x,
        direction_y,
        PLAYER_COLOR,
    );

    draw_player_marker(framebuffer, player_x, player_y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_fits_within_framebuffer_bounds() {
        let layout = compute_layout(624, 432, 13, 9).expect("layout should be computed");

        assert!(layout.box_left >= 0);
        assert!(layout.box_top >= 0);
        assert!(layout.box_right <= 624);
        assert!(layout.box_bottom <= 432);
    }

    #[test]
    fn layout_is_anchored_top_right() {
        let layout = compute_layout(624, 432, 13, 9).expect("layout should be computed");

        // El borde derecho debe quedar cerca del borde derecho del
        // framebuffer (a OUTER_MARGIN de distancia), y el superior
        // cerca del borde superior.
        assert!((624 - layout.box_right) <= OUTER_MARGIN.round() as i32 + 1);
        assert!(layout.box_top <= OUTER_MARGIN.round() as i32 + 1);

        // No debe convertirse en un segundo viewport de ancho
        // completo: la caja debe ser claramente más angosta que el
        // framebuffer.
        assert!(layout.box_right - layout.box_left < 624 / 2);
    }

    #[test]
    fn small_and_large_levels_fit_the_configured_maximum() {
        let small = compute_layout(624, 432, 13, 9).expect("small level should fit");

        let large = compute_layout(624, 432, 50, 40).expect("large level should fit");

        let small_width = small.box_right - small.box_left;

        let large_width = large.box_right - large.box_left;

        assert!(
            small_width as f32 <= MAX_CONTENT_WIDTH + 2.0 * PADDING + 2.0 * BORDER_THICKNESS + 1.0
        );

        assert!(
            large_width as f32 <= MAX_CONTENT_WIDTH + 2.0 * PADDING + 2.0 * BORDER_THICKNESS + 1.0
        );
    }

    #[test]
    fn aspect_ratio_is_preserved_via_uniform_scale() {
        let layout = compute_layout(624, 432, 13, 9).expect("layout should be computed");

        let content_width = level_content_width(&layout, 13);

        let content_height = level_content_height(&layout, 9);

        let expected_ratio = 13.0 / 9.0;

        let actual_ratio = content_width / content_height;

        assert!((actual_ratio - expected_ratio).abs() < 0.01);
    }

    fn level_content_width(layout: &MinimapLayout, level_width: usize) -> f32 {
        level_width as f32 * layout.cell_scale
    }

    fn level_content_height(layout: &MinimapLayout, level_height: usize) -> f32 {
        level_height as f32 * layout.cell_scale
    }

    #[test]
    fn world_position_converts_to_expected_minimap_location() {
        let layout = compute_layout(624, 432, 13, 9).expect("layout should be computed");

        let (x, y) = world_to_minimap(&layout, 0.0, 0.0, 48);

        assert!((x - layout.map_left).abs() < 1e-4);
        assert!((y - layout.map_top).abs() < 1e-4);

        let (x, y) = world_to_minimap(&layout, 48.0, 48.0, 48);

        assert!((x - (layout.map_left + layout.cell_scale)).abs() < 1e-4);
        assert!((y - (layout.map_top + layout.cell_scale)).abs() < 1e-4);
    }

    #[test]
    fn tiny_framebuffer_degrades_safely_without_panicking() {
        let layout = compute_layout(4, 4, 13, 9);

        assert!(layout.is_none());
    }

    #[test]
    fn zero_sized_level_is_rejected_safely() {
        assert!(compute_layout(624, 432, 0, 9).is_none());
        assert!(compute_layout(624, 432, 13, 0).is_none());
    }
}
