use raylib::prelude::Vector2;
use std::f32::consts::{PI, TAU};

use super::framebuffer::Framebuffer;
use super::textures::{TextureAsset, TextureManager};
use crate::player::Player;
use crate::world::{AmmoPickup, Entity, HealthPickup, Level, LevelTheme};

/// Distancia mínima segura para evitar dividir por (casi) cero al
/// calcular el ángulo/dirección hacia el sprite.
const MIN_DISTANCE: f32 = 0.0001;

/// Profundidad mínima segura frente a la cámara. Un sprite con
/// profundidad menor o igual a este valor está detrás o
/// prácticamente sobre el plano de la cámara y no se dibuja.
const MIN_DEPTH: f32 = 0.0001;

/// Calcula la posición de un punto del mundo en el espacio de
/// cámara del jugador: desplazamiento lateral y profundidad
/// perpendicular ("hacia adelante").
///
/// Retorna `None` si el punto está demasiado cerca del jugador o
/// detrás/sobre el plano de la cámara, en cuyo caso no debe
/// proyectarse ni participar en el ordenamiento de dibujo.
///
/// Esta es la ÚNICA definición de este cálculo; tanto la
/// proyección (`draw_billboard`) como el ordenamiento far->near
/// (`render_world_sprites`) la reutilizan en lugar de duplicarla.
fn camera_space(player: &Player, world_position: Vector2) -> Option<(f32, f32)> {
    let dx = world_position.x - player.pos.x;

    let dy = world_position.y - player.pos.y;

    let distance = dx.hypot(dy);

    if distance < MIN_DISTANCE {
        return None;
    }

    let sprite_angle = dy.atan2(dx);

    /*
     * Ángulo relativo a la dirección del jugador, normalizado a
     * [-PI, PI).
     */
    let relative_angle = (sprite_angle - player.a + PI).rem_euclid(TAU) - PI;

    /*
     * Profundidad perpendicular/de cámara: el mismo tipo de valor
     * almacenado en wall_depth_buffer, usado tanto para proyectar
     * el sprite como para su oclusión contra paredes.
     */
    let depth = distance * relative_angle.cos();

    if depth <= MIN_DEPTH {
        return None;
    }

    let lateral = distance * relative_angle.sin();

    Some((lateral, depth))
}

/// Convierte una celda (fila, columna) al centro de esa celda en
/// coordenadas de mundo, con la misma convención de centrado que
/// usan la aparición del jugador y la meta.
fn cell_center(row: usize, column: usize, block_size: usize) -> Vector2 {
    let half_block = block_size as f32 / 2.0;

    Vector2::new(
        column as f32 * block_size as f32 + half_block,
        row as f32 * block_size as f32 + half_block,
    )
}

/// Dibuja un billboard genérico: una textura que siempre mira
/// hacia la cámara, proyectada en perspectiva desde su posición
/// en el mundo.
///
/// `world_size` es la altura mundial aproximada del sprite (en las
/// mismas unidades que `BLOCK_SIZE`); el ancho proyectado preserva
/// la relación de aspecto real de la textura.
///
/// `wall_depth_buffer` es el z-buffer de paredes de ESTE MISMO
/// cuadro, producido por `render_world`. Antes de dibujar cada
/// columna de pantalla del sprite se compara su profundidad de
/// cámara con la profundidad de pared de esa misma columna; si la
/// pared está igual o más cerca, esa columna del sprite se omite
/// por completo, permitiendo oclusión parcial.
fn draw_billboard(
    framebuffer: &mut Framebuffer,
    player: &Player,
    world_position: Vector2,
    texture: &TextureAsset,
    world_size: f32,
    wall_depth_buffer: &[f32],
) {
    let texture_width = texture.width();

    let texture_height = texture.height();

    if texture_width <= 0 || texture_height <= 0 || world_size <= 0.0 {
        return;
    }

    let Some((lateral, depth)) = camera_space(player, world_position) else {
        return;
    };

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
         * Oclusión por columna: si no hay profundidad de pared
         * registrada para esta columna, o la pared está igual o
         * más cerca que el sprite, esta columna no se dibuja.
         *
         * `screen_x` ya es no negativo gracias al recorte anterior
         * (`draw_left = ....max(0.0)`), por lo que la conversión a
         * `usize` es segura.
         */
        let Some(&wall_depth) = wall_depth_buffer.get(screen_x as usize) else {
            continue;
        };

        if depth >= wall_depth {
            continue;
        }

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

/// Un billboard preparado para dibujarse en el cuadro actual.
///
/// Esto es un ítem de renderizado LOCAL A LA LLAMADA de dibujo, no
/// una entidad de juego: no se almacena entre cuadros.
struct BillboardItem<'a> {
    world_position: Vector2,
    texture: &'a TextureAsset,
    world_size: f32,
}

/// Tamaño de mundo del billboard del pickup de munición (Tarea 44),
/// deliberadamente menor que `block_size` (el tamaño que usan meta/
/// antorcha/Dealer): el pickup debe leerse claramente como más
/// pequeño que The Dealer y el portal, sin necesitar una textura de
/// mayor resolución ni un mecanismo de escalado nuevo — el mismo
/// billboard existente ya acepta cualquier `world_size`.
const AMMO_PICKUP_WORLD_SIZE_FACTOR: f32 = 0.5;

/// Tamaño de mundo del billboard del pickup de vida (Health Pickup),
/// misma proporción que `AMMO_PICKUP_WORLD_SIZE_FACTOR` (sección 7:
/// "escala de los demás objetos") — un corazón del mismo tamaño
/// aparente que el diamante de munición, sin introducir una segunda
/// escala arbitraria.
const HEALTH_PICKUP_WORLD_SIZE_FACTOR: f32 = 0.5;

/// Dibuja todos los sprites billboard de la escena actual (meta y
/// antorchas), ordenados de más lejano a más cercano y ocluidos
/// contra `wall_depth_buffer` de este mismo cuadro.
///
/// `torch_frame_index` es el cuadro de animación de antorcha
/// decidido por `GameSession`; este renderer solo LEE ese índice
/// para seleccionar la textura correspondiente, nunca lo avanza.
///
/// `entities` son los Dealers (y futuras entidades) activos de la
/// sesión actual; este renderer solo LEE su posición, identidad
/// visual y estado para decidir si/cómo dibujarlos, nunca los
/// muta.
pub(crate) fn render_world_sprites(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    textures: &TextureManager,
    block_size: usize,
    torch_frame_index: usize,
    entities: &[Entity],
    ammo_pickups: &[AmmoPickup],
    health_pickups: &[HealthPickup],
    wall_depth_buffer: &[f32],
    theme: LevelTheme,
) {
    let mut items: Vec<BillboardItem> = Vec::new();

    if let Some(texture) = textures.themed_goal_texture(theme) {
        let (row, column) = level.goal();

        items.push(BillboardItem {
            world_position: cell_center(row, column, block_size),
            texture,
            world_size: block_size as f32,
        });
    }

    if let Some(texture) = textures.themed_torch_texture(torch_frame_index, theme) {
        for &(row, column) in level.torch_spawns() {
            items.push(BillboardItem {
                world_position: cell_center(row, column, block_size),
                texture,
                world_size: block_size as f32,
            });
        }
    }

    /*
     * Las cuatro combinaciones identidad+estado (incluida `Dead`)
     * tienen una textura determinista propia desde Tarea 24: un
     * Dealer muerto sigue siendo un billboard visible (cadáver),
     * simplemente con otra textura, y participa del MISMO pipeline
     * de proyección/ordenamiento/oclusión que cualquier otro
     * sprite.
     */
    for entity in entities {
        if let Some(texture) =
            textures.themed_entity_texture(entity.sprite(), entity.state(), theme)
        {
            items.push(BillboardItem {
                world_position: entity.position(),
                texture,
                world_size: block_size as f32,
            });
        }
    }

    /*
     * Tarea 44: solo los pickups ACTIVOS (todavía no recogidos en
     * esta sesión) entran al pipeline de dibujo — `GameSession` es
     * quien decide cuándo desactivar uno (`collect_nearby_ammo_pickups`,
     * llamado desde el update jugable, nunca desde aquí). Reutiliza
     * el MISMO pipeline de proyección/orden/oclusión que meta/
     * antorcha/Dealer; la única diferencia es un `world_size` menor.
     */
    if let Some(texture) = textures.themed_ammo_pickup_texture(theme) {
        for pickup in ammo_pickups {
            if !pickup.is_active() {
                continue;
            }

            items.push(BillboardItem {
                world_position: pickup.position(),
                texture,
                world_size: block_size as f32 * AMMO_PICKUP_WORLD_SIZE_FACTOR,
            });
        }
    }

    /*
     * Health Pickup: mismo pipeline que el pickup de munición
     * (proyección/orden/oclusión), con una diferencia deliberada —
     * `health_pickup_texture` NUNCA se resuelve por tema (sección 8):
     * el corazón conserva siempre su único color rojo/crimson en los
     * tres temas visuales.
     */
    if let Some(texture) = textures.health_pickup_texture() {
        for pickup in health_pickups {
            if !pickup.is_active() {
                continue;
            }

            items.push(BillboardItem {
                world_position: pickup.position(),
                texture,
                world_size: block_size as f32 * HEALTH_PICKUP_WORLD_SIZE_FACTOR,
            });
        }
    }

    /*
     * Orden de pintor: de más lejano a más cercano, para que un
     * sprite más cercano sobrescriba a uno más lejano si se
     * superponen en pantalla. Un ítem sin profundidad de cámara
     * válida (detrás/sobre el plano de la cámara) se ordena al
     * frente; `draw_billboard` lo descartará de todas formas.
     */
    let depths: Vec<f32> = items
        .iter()
        .map(|item| {
            camera_space(player, item.world_position).map_or(f32::INFINITY, |(_, depth)| depth)
        })
        .collect();

    let mut order: Vec<usize> = (0..items.len()).collect();

    order.sort_by(|&a, &b| depths[b].total_cmp(&depths[a]));

    for index in order {
        let item = &items[index];

        draw_billboard(
            framebuffer,
            player,
            item.world_position,
            item.texture,
            item.world_size,
            wall_depth_buffer,
        );
    }
}
