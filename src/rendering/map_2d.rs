use super::framebuffer::Framebuffer;
use crate::config::BLOCK_SIZE;
use crate::player::Player;
use crate::raycasting::{RayHit, cast_ray};
use crate::world::Level;
use raylib::prelude::Color;

/// Calcula el tamaño de celda, en píxeles de PANTALLA, que usa el
/// mapa de depuración 2D para dibujar el nivel COMPLETO dentro del
/// framebuffer de resolución fija.
///
/// Esto es una escala de VISUALIZACIÓN, distinta de `BLOCK_SIZE`
/// (escala de MUNDO usada por colisión/raycasting/posición del
/// jugador, que nunca cambia). Para un nivel de 13×9 a 624×432 esto
/// retorna exactamente `BLOCK_SIZE` (48), preservando byte a byte la
/// apariencia histórica de Crimson Entrance/Black Club; para un
/// nivel más grande (por ejemplo House of Cards, 17×13) retorna un
/// valor menor para que el mapa completo quepa sin recortarse.
///
/// Retorna `0` de forma segura (sin panic ni división por cero) si
/// el framebuffer o el nivel tienen dimensiones inválidas; el
/// llamador debe tratar `0` como "no dibujar nada útil", igual que
/// hace `rendering::minimap` con su propio `Option::None`.
pub(crate) fn compute_display_cell_size(
    framebuffer_width: i32,
    framebuffer_height: i32,
    level_width: usize,
    level_height: usize,
    world_block_size: usize,
) -> usize {
    if framebuffer_width <= 0
        || framebuffer_height <= 0
        || level_width == 0
        || level_height == 0
        || world_block_size == 0
    {
        return 0;
    }

    let by_width = framebuffer_width as usize / level_width;

    let by_height = framebuffer_height as usize / level_height;

    world_block_size.min(by_width).min(by_height)
}

/// Factor de escala VISUALIZACIÓN/MUNDO: multiplica una coordenada
/// en píxeles de mundo (escala `world_block_size`) para obtener su
/// coordenada equivalente en píxeles de pantalla del mapa 2D (escala
/// `display_cell_size`). `world_block_size == 0` retorna `0.0` de
/// forma segura.
fn display_scale(display_cell_size: usize, world_block_size: usize) -> f32 {
    if world_block_size == 0 {
        return 0.0;
    }

    display_cell_size as f32 / world_block_size as f32
}

/// Rellena un rectángulo dentro del framebuffer.
fn fill_rectangle(
    framebuffer: &mut Framebuffer,
    x0: usize,
    y0: usize,
    width: usize,
    height: usize,
    color: Color,
) {
    framebuffer.set_current_color(color);

    let framebuffer_width = framebuffer.width() as usize;
    let framebuffer_height = framebuffer.height() as usize;

    let final_x = x0.saturating_add(width).min(framebuffer_width);

    let final_y = y0.saturating_add(height).min(framebuffer_height);

    for y in y0..final_y {
        for x in x0..final_x {
            framebuffer.point(x as i32, y as i32);
        }
    }
}

/// Dibuja una celda del laberinto.
pub fn draw_cell(
    framebuffer: &mut Framebuffer,
    x0: usize,
    y0: usize,
    block_size: usize,
    cell: char,
) {
    let wall_color = Color::new(48, 48, 82, 255);
    let floor_color = Color::new(255, 220, 220, 255);
    let goal_color = Color::new(210, 45, 55, 255);
    let invalid_color = Color::new(255, 0, 255, 255);

    match cell {
        // Paredes.
        '+' | '-' | '|' | '#' => {
            fill_rectangle(framebuffer, x0, y0, block_size, block_size, wall_color);
        }

        // Tanto el espacio como p representan una celda
        // transitable. El jugador se dibuja por separado.
        ' ' | 'p' => {
            fill_rectangle(framebuffer, x0, y0, block_size, block_size, floor_color);
        }

        // Meta.
        'g' => {
            fill_rectangle(framebuffer, x0, y0, block_size, block_size, floor_color);

            let margin = block_size / 4;
            let marker_size = block_size.saturating_sub(margin * 2);

            fill_rectangle(
                framebuffer,
                x0 + margin,
                y0 + margin,
                marker_size,
                marker_size,
                goal_color,
            );
        }

        // Carácter desconocido.
        _ => {
            fill_rectangle(framebuffer, x0, y0, block_size, block_size, invalid_color);
        }
    }
}

/// Recorre todas las filas y columnas del laberinto.
///
/// `display_cell_size` es la escala de PANTALLA (ver
/// `compute_display_cell_size`); para un nivel de 13×9 a 624×432
/// vale exactamente `BLOCK_SIZE`, por lo que el dibujo resultante es
/// idéntico al de antes de Tarea 35.
pub(crate) fn render_maze(framebuffer: &mut Framebuffer, level: &Level, display_cell_size: usize) {
    for row_index in 0..level.height() {
        for column_index in 0..level.width() {
            if let Some(cell) = level.cell_at(row_index, column_index) {
                let x0 = column_index * display_cell_size;
                let y0 = row_index * display_cell_size;

                draw_cell(framebuffer, x0, y0, display_cell_size, cell);
            }
        }
    }
}

/// Lanza múltiples rayos y dibuja su trayectoria para mostrar
/// el campo de visión en el mapa 2D.
///
/// `display_cell_size`/`world_block_size` transforman las
/// coordenadas de MUNDO que produce `cast_ray` (siempre en escala
/// `world_block_size`, sin cambios) a coordenadas de PANTALLA del
/// mapa 2D (escala `display_cell_size`). La geometría del raycaster
/// en sí no se modifica: solo se escala el resultado ya calculado
/// antes de dibujarlo.
pub(crate) fn render_fov_rays(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    number_of_rays: usize,
    display_cell_size: usize,
    world_block_size: usize,
) {
    if number_of_rays == 0 {
        return;
    }

    let scale = display_scale(display_cell_size, world_block_size);

    let start_angle = player.a - player.fov / 2.0;

    if number_of_rays == 1 {
        draw_ray_trajectory(framebuffer, level, player, player.a, scale);

        return;
    }

    let angle_step = player.fov / (number_of_rays - 1) as f32;

    for ray_index in 0..number_of_rays {
        let ray_angle = start_angle + ray_index as f32 * angle_step;

        draw_ray_trajectory(framebuffer, level, player, ray_angle, scale);
    }
}

/// Dibuja la trayectoria visible de un rayo hasta la distancia
/// calculada por el raycaster.
///
/// El renderer no decide colisiones: únicamente recorre, con el
/// mismo incremento que usaba el raycaster, el tramo entre el
/// jugador y `RayHit::distance`.
fn draw_ray_trajectory(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    ray_angle: f32,
    scale: f32,
) {
    const STEP_SIZE: f32 = 1.0;

    let ray_hit = cast_ray(level, player, ray_angle);

    framebuffer.set_current_color(Color::new(245, 245, 245, 255));

    let mut distance = 0.0;

    while distance < ray_hit.distance {
        let ray_x = player.pos.x + distance * ray_angle.cos();

        let ray_y = player.pos.y + distance * ray_angle.sin();

        framebuffer.point(
            (ray_x * scale).round() as i32,
            (ray_y * scale).round() as i32,
        );

        distance += STEP_SIZE;
    }

    if should_draw_boundary_sample(ray_angle, &ray_hit) {
        framebuffer.point(
            (ray_hit.position.x * scale).round() as i32,
            (ray_hit.position.y * scale).round() as i32,
        );
    }
}

/// Determina si el punto exacto de la frontera de celda coincide
/// con una muestra que el antiguo marcher por pasos de 1.0 habría
/// dibujado.
///
/// El antiguo marcher clasificaba cada muestra con `floor(x /
/// BLOCK_SIZE)`. Para una entrada por el eje negativo (rayo hacia
/// la izquierda o hacia arriba) la frontera exacta de la celda
/// golpeada cae, bajo esa regla de piso, dentro de la celda
/// ANTERIOR (transitable), así que si la distancia exacta coincide
/// con un paso entero de la cuadrícula de muestreo, esa muestra sí
/// se dibujaba. Para una entrada por el eje positivo (derecha o
/// abajo) la frontera cae directamente en la celda golpeada
/// (bloqueada), por lo que nunca se dibujaba.
///
/// Esto es una reconstrucción puramente geométrica para la
/// visualización de depuración; no consulta `Level` ni recalcula
/// colisiones.
fn should_draw_boundary_sample(ray_angle: f32, ray_hit: &RayHit) -> bool {
    /// Tolerancia para considerar que la distancia exacta cae
    /// sobre la cuadrícula de muestreo de 1.0 unidades.
    const LATTICE_EPSILON: f32 = 1e-3;

    /// Tolerancia para considerar que una coordenada de impacto
    /// cae exactamente sobre un borde de `BLOCK_SIZE`.
    const BOUNDARY_EPSILON: f32 = 1e-3;

    let nearest_step = ray_hit.distance.round();

    if (ray_hit.distance - nearest_step).abs() > LATTICE_EPSILON {
        return false;
    }

    let direction_x = ray_angle.cos();

    let direction_y = ray_angle.sin();

    let block_size = BLOCK_SIZE as f32;

    let x_grid = ray_hit.position.x / block_size;

    let y_grid = ray_hit.position.y / block_size;

    let x_on_boundary = (x_grid - x_grid.round()).abs() < BOUNDARY_EPSILON / block_size;

    let y_on_boundary = (y_grid - y_grid.round()).abs() < BOUNDARY_EPSILON / block_size;

    (x_on_boundary && direction_x < 0.0) || (y_on_boundary && direction_y < 0.0)
}

/// Dibuja al jugador como un pequeño círculo.
///
/// `display_cell_size`/`world_block_size`: misma transformación de
/// escala MUNDO -> PANTALLA que `render_fov_rays`, para que el
/// marcador del jugador quede alineado con el laberinto ya escalado
/// dibujado por `render_maze`. El radio del marcador permanece fijo
/// en píxeles de pantalla (no se escala): para 13×9 la escala es
/// 1.0, así que el resultado es idéntico al de antes de Tarea 35.
pub fn render_player(
    framebuffer: &mut Framebuffer,
    player: &Player,
    display_cell_size: usize,
    world_block_size: usize,
) {
    const PLAYER_RADIUS: i32 = 5;

    framebuffer.set_current_color(Color::new(50, 220, 160, 255));

    let scale = display_scale(display_cell_size, world_block_size);

    let center_x = (player.pos.x * scale).round() as i32;
    let center_y = (player.pos.y * scale).round() as i32;

    for offset_y in -PLAYER_RADIUS..=PLAYER_RADIUS {
        for offset_x in -PLAYER_RADIUS..=PLAYER_RADIUS {
            let distance_squared = offset_x * offset_x + offset_y * offset_y;

            if distance_squared <= PLAYER_RADIUS * PLAYER_RADIUS {
                framebuffer.point(center_x + offset_x, center_y + offset_y);
            }
        }
    }
}

#[cfg(test)]
mod display_scale_tests {
    use super::*;

    #[test]
    fn thirteen_by_nine_at_reference_framebuffer_retains_historical_forty_eight() {
        assert_eq!(compute_display_cell_size(624, 432, 13, 9, 48), 48);
    }

    #[test]
    fn seventeen_by_thirteen_at_reference_framebuffer_selects_a_smaller_positive_cell_size() {
        let cell_size = compute_display_cell_size(624, 432, 17, 13, 48);

        assert!(cell_size > 0);
        assert!(cell_size < 48);
    }

    #[test]
    fn seventeen_by_thirteen_complete_pixel_footprint_fits_the_framebuffer() {
        let cell_size = compute_display_cell_size(624, 432, 17, 13, 48);

        assert!(17 * cell_size <= 624);
        assert!(13 * cell_size <= 432);
    }

    #[test]
    fn zero_level_dimensions_are_safe() {
        assert_eq!(compute_display_cell_size(624, 432, 0, 9, 48), 0);
        assert_eq!(compute_display_cell_size(624, 432, 13, 0, 48), 0);
    }

    #[test]
    fn zero_framebuffer_dimensions_are_safe() {
        assert_eq!(compute_display_cell_size(0, 432, 13, 9, 48), 0);
        assert_eq!(compute_display_cell_size(624, 0, 13, 9, 48), 0);
    }

    #[test]
    fn zero_world_block_size_is_safe() {
        assert_eq!(compute_display_cell_size(624, 432, 13, 9, 0), 0);
    }

    #[test]
    fn display_scale_is_one_when_display_matches_world_block_size() {
        assert_eq!(display_scale(48, 48), 1.0);
    }

    #[test]
    fn display_scale_is_less_than_one_for_a_smaller_display_cell_size() {
        assert!(display_scale(33, 48) < 1.0);
        assert!(display_scale(33, 48) > 0.0);
    }

    #[test]
    fn display_scale_zero_world_block_size_is_safe() {
        assert_eq!(display_scale(48, 0), 0.0);
    }
}
