use super::background::draw_background;
use super::framebuffer::Framebuffer;
use super::textures::{TextureAsset, TextureManager};
use crate::player::Player;
use crate::raycasting::{cast_ray, ray_angle_for_column};
use crate::world::{Level, LevelTheme};
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

/// Deriva la coordenada de textura horizontal (tx) a partir del
/// desplazamiento normalizado geométrico calculado por el
/// raycaster.
///
/// `texture_offset` está garantizado por el raycaster a estar en
/// `[0.0, 1.0)`. El resultado se recorta defensivamente al rango
/// válido de píxeles de la textura ante cualquier imprecisión de
/// punto flotante.
fn calculate_tx(texture_offset: f32, texture_width: i32) -> i32 {
    if texture_width <= 0 {
        return 0;
    }

    let tx = (texture_offset * texture_width as f32).floor() as i32;

    tx.clamp(0, texture_width - 1)
}

/// Deriva la coordenada de textura vertical (ty) a partir de la
/// posición de un píxel de pantalla dentro del STAKE PROYECTADO
/// COMPLETO (sin recortar contra los límites de la pantalla).
///
/// `projected_top` y `stake_height` deben ser los valores de
/// proyección SIN recortar; usar el rango de dibujo ya recortado
/// aquí estiraría la textura para llenar únicamente la porción
/// visible de una pared cercana, lo cual es incorrecto.
fn calculate_ty(screen_y: i32, projected_top: f32, stake_height: f32, texture_height: i32) -> i32 {
    if texture_height <= 0 || stake_height <= 0.0 {
        return 0;
    }

    let relative_y = screen_y as f32 - projected_top;

    let texture_v = relative_y / stake_height;

    let ty = (texture_v * texture_height as f32).floor() as i32;

    ty.clamp(0, texture_height - 1)
}

/// Dibuja una columna de pared muestreando la textura suministrada.
///
/// `draw_top`/`draw_bottom` son el rango de píxeles de pantalla ya
/// recortado que realmente se dibuja. `projected_top`/
/// `stake_height` describen la proyección COMPLETA sin recortar,
/// usada únicamente para calcular `ty` correctamente.
///
/// Retorna `false` sin dibujar nada si la textura tiene
/// dimensiones inválidas, dejando que el llamador recurra a un
/// respaldo (por ejemplo, el color plano existente).
fn draw_textured_column(
    framebuffer: &mut Framebuffer,
    screen_x: i32,
    draw_top: i32,
    draw_bottom: i32,
    projected_top: f32,
    stake_height: f32,
    texture: &TextureAsset,
    texture_offset: f32,
) -> bool {
    let texture_width = texture.width();

    let texture_height = texture.height();

    if texture_width <= 0 || texture_height <= 0 || stake_height <= 0.0 {
        return false;
    }

    let tx = calculate_tx(texture_offset, texture_width);

    for screen_y in draw_top..=draw_bottom {
        let ty = calculate_ty(screen_y, projected_top, stake_height, texture_height);

        if let Some(color) = texture.pixel_at(tx, ty) {
            framebuffer.set_current_color(color);

            framebuffer.point(screen_x, screen_y);
        }
    }

    true
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
    textures: &TextureManager,
    theme: LevelTheme,
) -> Vec<f32> {
    let screen_width = framebuffer.width().max(1);

    let screen_height = framebuffer.height().max(1);

    let mut wall_depth_buffer = Vec::with_capacity(screen_width as usize);

    draw_background(framebuffer, theme);

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
        /*
         * Distribuye el rayo de esta columna a lo largo del abanico
         * de FOV, desde `a - fov/2` hasta `a + fov/2`
         * (`raycasting::ray_angle_for_column`, extraída en Tarea 37
         * a partir de esta misma fórmula, sin cambio de
         * comportamiento).
         */
        let ray_angle = ray_angle_for_column(
            player.a,
            screen_x as usize,
            screen_width as usize,
            player.fov,
        );

        let ray_hit = cast_ray(level, player, ray_angle);

        /*
         * Corregir el efecto ojo de pez.
         *
         * Sin esta corrección, las paredes rectas
         * parecerían curvas.
         */
        let corrected_distance = ray_hit.distance * (ray_angle - player.a).cos();

        let corrected_distance = corrected_distance.max(0.0001);

        wall_depth_buffer.push(corrected_distance);

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
         *
         * `projected_top` es la proyección COMPLETA sin recortar;
         * `stake_top`/`stake_bottom` son el rango ya recortado
         * contra los límites de la pantalla que realmente se
         * dibuja.
         */
        let projected_top = half_height - stake_height / 2.0;

        let stake_top = projected_top.floor().max(0.0) as i32;

        let stake_bottom = (half_height + stake_height / 2.0)
            .ceil()
            .min(screen_height as f32 - 1.0) as i32;

        /*
         * Si el carácter de pared golpeado tiene una textura
         * registrada, dibujar la columna muestreándola; de lo
         * contrario, usar el color plano existente.
         */
        let textured = textures.wall_texture(ray_hit.tile).is_some_and(|texture| {
            draw_textured_column(
                framebuffer,
                screen_x,
                stake_top,
                stake_bottom,
                projected_top,
                stake_height,
                texture,
                ray_hit.texture_offset,
            )
        });

        if !textured {
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

    wall_depth_buffer
}
