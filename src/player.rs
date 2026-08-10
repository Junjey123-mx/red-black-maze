use crate::rendering::framebuffer::Framebuffer;
use crate::text_load::Maze;
use raylib::prelude::{Color, Vector2};
use std::f32::consts::PI;

/// Representa al jugador dentro del laberinto.
pub struct Player {
    /// Posición del jugador expresada en píxeles.
    pub pos: Vector2,

    /// Dirección central hacia donde mira el jugador.
    pub a: f32,

    /// Campo de visión total expresado en radianes.
    pub fov: f32,
}

impl Player {
    /// Busca el carácter `p` en el laberinto y coloca
    /// al jugador en el centro de esa celda.
    pub fn from_maze(maze: &Maze, block_size: usize) -> Result<Self, String> {
        for (row_index, row) in maze.iter().enumerate() {
            for (column_index, &cell) in row.iter().enumerate() {
                if cell == 'p' {
                    let half_block = block_size as f32 / 2.0;

                    let x = column_index as f32 * block_size as f32 + half_block;

                    let y = row_index as f32 * block_size as f32 + half_block;

                    return Ok(Self {
                        pos: Vector2::new(x, y),

                        // Dirección inicial de 60 grados.
                        a: PI / 3.0,

                        // Campo de visión total de 60 grados.
                        fov: PI / 3.0,
                    });
                }
            }
        }

        Err("No se encontró el carácter 'p' en el laberinto".to_string())
    }
}

/// Dibuja al jugador como un pequeño círculo.
pub fn render_player(framebuffer: &mut Framebuffer, player: &Player) {
    const PLAYER_RADIUS: i32 = 5;

    framebuffer.set_current_color(Color::new(50, 220, 160, 255));

    let center_x = player.pos.x.round() as i32;
    let center_y = player.pos.y.round() as i32;

    for offset_y in -PLAYER_RADIUS..=PLAYER_RADIUS {
        for offset_x in -PLAYER_RADIUS..=PLAYER_RADIUS {
            let distance_squared = offset_x * offset_x + offset_y * offset_y;

            if distance_squared <= PLAYER_RADIUS * PLAYER_RADIUS {
                framebuffer.point(center_x + offset_x, center_y + offset_y);
            }
        }
    }
}
