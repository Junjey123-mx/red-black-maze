use raylib::prelude::Vector2;
use std::f32::consts::{PI, TAU};

use super::framebuffer::Framebuffer;
use super::textures::{TextureAsset, TextureManager};
use crate::player::Player;
use crate::world::Level;

/// Distancia mínima segura para evitar dividir por (casi) cero al
/// calcular el ángulo/dirección hacia el sprite.
const MIN_DISTANCE: f32 = 0.0001;

/// Profundidad mínima segura frente a la cámara. Un sprite con
/// profundidad menor o igual a este valor está detrás o
/// prácticamente sobre el plano de la cámara y no se dibuja.
const MIN_DEPTH: f32 = 0.0001;

/// Dibuja un billboard genérico: una textura que siempre mira
/// hacia la cámara, proyectada en perspectiva desde su posición
/// en el mundo.
///
/// `world_size` es la altura mundial aproximada del sprite (en las
/// mismas unidades que `BLOCK_SIZE`); el ancho proyectado preserva
/// la relación de aspecto real de la textura.
///
/// Esta función es puramente de proyección/dibujo: no compara
/// contra el z-buffer de paredes (eso pertenece a la Tarea 18), por
/// lo que el sprite puede dibujarse incluso si geométricamente está
/// detrás de una pared.
fn draw_billboard(
    framebuffer: &mut Framebuffer,
    player: &Player,
    world_position: Vector2,
    texture: &TextureAsset,
    world_size: f32,
) {
    let texture_width = texture.width();

    let texture_height = texture.height();

    if texture_width <= 0 || texture_height <= 0 || world_size <= 0.0 {
        return;
    }

    /*
     * Vector del jugador hacia el sprite.
     */
    let dx = world_position.x - player.pos.x;

    let dy = world_position.y - player.pos.y;

    let distance = dx.hypot(dy);

    if distance < MIN_DISTANCE {
        return;
    }

    let sprite_angle = dy.atan2(dx);

    /*
     * Ángulo relativo a la dirección del jugador, normalizado a
     * [-PI, PI).
     */
    let relative_angle = (sprite_angle - player.a + PI).rem_euclid(TAU) - PI;

    /*
     * Profundidad perpendicular/de cámara, del mismo tipo que la
     * futura Tarea 18 comparará contra wall_depth_buffer. Aquí
     * SOLO se usa para proyectar el sprite, no para ocluirlo.
     */
    let depth = distance * relative_angle.cos();

    if depth <= MIN_DEPTH {
        return;
    }

    let lateral = distance * relative_angle.sin();

    let screen_width = framebuffer.width().max(1);

    let screen_height = framebuffer.height().max(1);

    let half_width = screen_width as f32 / 2.0;

    let half_height = screen_height as f32 / 2.0;

    /*
     * Mismo modelo de plano de proyección que usa world_3d para
     * las paredes.
     */
    let distance_to_projection_plane = half_width / (player.fov / 2.0).tan();

    let screen_center_x = half_width + lateral * distance_to_projection_plane / depth;

    let sprite_height = world_size * distance_to_projection_plane / depth;

    let sprite_width = sprite_height * texture_width as f32 / texture_height as f32;

    let projected_top = half_height - sprite_height / 2.0;

    let projected_bottom = half_height + sprite_height / 2.0;

    let projected_left = screen_center_x - sprite_width / 2.0;

    let projected_right = screen_center_x + sprite_width / 2.0;

    /*
     * Rango de dibujo recortado contra los límites de la pantalla.
     */
    let draw_left = projected_left.floor().max(0.0) as i32;

    let draw_right = projected_right.ceil().min(screen_width as f32 - 1.0) as i32;

    let draw_top = projected_top.floor().max(0.0) as i32;

    let draw_bottom = projected_bottom.ceil().min(screen_height as f32 - 1.0) as i32;

    if draw_left > draw_right || draw_top > draw_bottom {
        return;
    }

    for screen_x in draw_left..=draw_right {
        /*
         * u/v se calculan sobre el rectángulo proyectado COMPLETO
         * (sin recortar), para que un sprite parcialmente fuera de
         * pantalla no estire la textura hacia la porción visible.
         */
        let u = (screen_x as f32 - projected_left) / sprite_width;

        let tx = (u * texture_width as f32).floor() as i32;

        let tx = tx.clamp(0, texture_width - 1);

        for screen_y in draw_top..=draw_bottom {
            let v = (screen_y as f32 - projected_top) / sprite_height;

            let ty = (v * texture_height as f32).floor() as i32;

            let ty = ty.clamp(0, texture_height - 1);

            let Some(color) = texture.pixel_at(tx, ty) else {
                continue;
            };

            /*
             * Alfa binaria: los píxeles totalmente transparentes
             * no se escriben, dejando visible lo ya renderizado
             * detrás del sprite.
             */
            if color.a == 0 {
                continue;
            }

            framebuffer.set_current_color(color);

            framebuffer.point(screen_x, screen_y);
        }
    }
}

/// Dibuja el sprite de la meta del nivel activo, si su textura ya
/// fue cargada por el `TextureManager`.
///
/// Convierte la posición de celda de `Level::goal()` al centro de
/// esa celda en coordenadas de mundo y delega la proyección al
/// billboard genérico.
pub(crate) fn render_goal_sprite(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    textures: &TextureManager,
    block_size: usize,
) {
    let Some(texture) = textures.goal_texture() else {
        return;
    };

    let (row, column) = level.goal();

    let half_block = block_size as f32 / 2.0;

    let x = column as f32 * block_size as f32 + half_block;

    let y = row as f32 * block_size as f32 + half_block;

    draw_billboard(
        framebuffer,
        player,
        Vector2::new(x, y),
        texture,
        block_size as f32,
    );
}
