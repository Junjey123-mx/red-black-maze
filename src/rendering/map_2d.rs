use super::framebuffer::Framebuffer;
use crate::text_load::Maze;
use raylib::prelude::Color;

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
pub fn render_maze(framebuffer: &mut Framebuffer, maze: &Maze, block_size: usize) {
    for (row_index, row) in maze.iter().enumerate() {
        for (column_index, &cell) in row.iter().enumerate() {
            let x0 = column_index * block_size;
            let y0 = row_index * block_size;

            draw_cell(framebuffer, x0, y0, block_size, cell);
        }
    }
}
