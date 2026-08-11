use super::background::draw_background;
use super::framebuffer::Framebuffer;
use crate::player::Player;
use crate::raycasting::cast_ray;
use crate::world::Level;
use raylib::prelude::Color;

/// Obtiene el color de una pared según el carácter
/// golpeado y la distancia.
///
/// Las paredes más lejanas se muestran más oscuras.
fn wall_color(impact: char, distance: f32) -> Color {
    let brightness = (255.0 / (1.0 + distance * 0.009)).clamp(45.0, 255.0);

    let bright = brightness as u8;

    let medium = (brightness * 0.82).clamp(0.0, 255.0) as u8;

    let dark = (brightness * 0.65).clamp(0.0, 255.0) as u8;

    match impact {
        // Pared vertical.
        '|' => Color::new(bright, 25, 30, 255),

        // Pared horizontal.
        '-' => Color::new(medium, 18, 24, 255),

        // Esquina.
        '+' => Color::new(dark, 12, 18, 255),

        // Carácter alternativo de pared.
        '#' => Color::new(medium, 20, 25, 255),

        // Carácter desconocido.
        _ => Color::new(bright, 0, bright, 255),
    }
}

/// Renderiza el laberinto utilizando proyección 3D.
///
/// Se lanza un rayo por cada columna de la pantalla.
/// La distancia de cada rayo determina la altura
/// de la columna vertical.
pub(crate) fn render_world(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    block_size: usize,
) {
    let screen_width = framebuffer.width().max(1);

    let screen_height = framebuffer.height().max(1);

    draw_background(framebuffer);

    let half_width = screen_width as f32 / 2.0;

    let half_height = screen_height as f32 / 2.0;

    /*
     * Distancia entre el jugador y el plano de proyección.
     *
     * Este valor depende del ancho de la pantalla y
     * del campo de visión.
     */
    let distance_to_projection_plane = half_width / (player.fov / 2.0).tan();

    /*
     * Cada columna de la pantalla representa un rayo.
     */
    for screen_x in 0..screen_width {
        let ray_fraction = (screen_x as f32 + 0.5) / screen_width as f32;

        /*
         * Distribuir el rayo desde:
         *
         * a - fov/2
         *
         * hasta:
         *
         * a + fov/2
         */
        let ray_angle = player.a - player.fov / 2.0 + player.fov * ray_fraction;

        let ray_hit = cast_ray(level, player, ray_angle);

        /*
         * Corregir el efecto ojo de pez.
         *
         * Sin esta corrección, las paredes rectas
         * parecerían curvas.
         */
        let corrected_distance = ray_hit.distance * (ray_angle - player.a).cos();

        let corrected_distance = corrected_distance.max(0.0001);

        /*
         * Altura de la columna:
         *
         * altura =
         *     tamaño_pared × distancia_proyección
         *     -----------------------------------
         *             distancia_a_pared
         */
        let stake_height = block_size as f32 * distance_to_projection_plane / corrected_distance;

        /*
         * Centrar la columna vertical.
         */
        let stake_top = (half_height - stake_height / 2.0).floor().max(0.0) as i32;

        let stake_bottom = (half_height + stake_height / 2.0)
            .ceil()
            .min(screen_height as f32 - 1.0) as i32;

        let color = wall_color(ray_hit.tile, corrected_distance);

        framebuffer.set_current_color(color);

        /*
         * Dibujar la columna vertical.
         */
        for y in stake_top..=stake_bottom {
            framebuffer.point(screen_x, y);
        }
    }
}
