use crate::player::Player;
use crate::rendering::framebuffer::Framebuffer;
use crate::world::Level;
use raylib::prelude::Color;

/// Información obtenida cuando un rayo impacta una pared.
#[derive(Debug, Clone, Copy)]
pub struct Intersect {
    /// Distancia desde el jugador hasta la pared.
    pub distance: f32,

    /// Carácter de la pared golpeada.
    pub impact: char,
}

/// Indica si una celda puede ser atravesada por el jugador
/// y por los rayos.
fn is_walkable(cell: char) -> bool {
    matches!(cell, ' ' | 'p' | 'g')
}

/// Lanza un único rayo desde el jugador.
///
/// `ray_angle` representa la dirección específica del rayo.
///
/// Cuando `draw_line` es verdadero, el recorrido se dibuja
/// sobre el mapa 2D. En la vista 3D será falso.
pub fn cast_ray(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    ray_angle: f32,
    block_size: usize,
    draw_line: bool,
) -> Intersect {
    const STEP_SIZE: f32 = 1.0;

    let map_width = level.width() as f32 * block_size as f32;

    let map_height = level.height() as f32 * block_size as f32;

    let max_distance = (map_width * map_width + map_height * map_height).sqrt();

    let mut distance = 0.0;

    if draw_line {
        framebuffer.set_current_color(Color::new(245, 245, 245, 255));
    }

    while distance <= max_distance {
        /*
         * Posición actual del rayo:
         *
         * x = jugador.x + distancia × cos(ángulo)
         * y = jugador.y + distancia × sin(ángulo)
         */
        let ray_x = player.pos.x + distance * ray_angle.cos();

        let ray_y = player.pos.y + distance * ray_angle.sin();

        /*
         * Detenerse si el rayo sale del mapa.
         */
        if ray_x < 0.0 || ray_y < 0.0 || ray_x >= map_width || ray_y >= map_height {
            return Intersect {
                distance,
                impact: '#',
            };
        }

        /*
         * Convertir la posición en píxeles a una posición
         * dentro de la matriz del laberinto.
         */
        let column = (ray_x / block_size as f32).floor() as usize;

        let row = (ray_y / block_size as f32).floor() as usize;

        let Some(cell) = level.cell_at(row, column) else {
            return Intersect {
                distance,
                impact: '#',
            };
        };

        /*
         * Si no es una celda transitable, el rayo
         * encontró una pared.
         */
        if !is_walkable(cell) {
            return Intersect {
                distance,
                impact: cell,
            };
        }

        /*
         * En el mapa 2D se dibuja la trayectoria.
         * En el mundo 3D únicamente necesitamos la distancia.
         */
        if draw_line {
            framebuffer.point(ray_x.round() as i32, ray_y.round() as i32);
        }

        distance += STEP_SIZE;
    }

    Intersect {
        distance: max_distance,
        impact: '#',
    }
}

/// Lanza múltiples rayos para mostrar el campo de visión
/// en el mapa 2D.
pub fn cast_fov(
    framebuffer: &mut Framebuffer,
    level: &Level,
    player: &Player,
    block_size: usize,
    number_of_rays: usize,
) {
    if number_of_rays == 0 {
        return;
    }

    let start_angle = player.a - player.fov / 2.0;

    if number_of_rays == 1 {
        cast_ray(framebuffer, level, player, player.a, block_size, true);

        return;
    }

    let angle_step = player.fov / (number_of_rays - 1) as f32;

    for ray_index in 0..number_of_rays {
        let ray_angle = start_angle + ray_index as f32 * angle_step;

        cast_ray(framebuffer, level, player, ray_angle, block_size, true);
    }
}
