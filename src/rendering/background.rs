use super::framebuffer::Framebuffer;
use raylib::prelude::Color;

/// Dibuja el cielo y el suelo de la vista 3D.
pub(super) fn draw_background(framebuffer: &mut Framebuffer) {
    let width = framebuffer.width();
    let height = framebuffer.height();
    let half_height = height / 2;

    let ceiling_color = Color::new(28, 20, 24, 255);

    let floor_color = Color::new(12, 12, 16, 255);

    for y in 0..height {
        if y < half_height {
            framebuffer.set_current_color(ceiling_color);
        } else {
            framebuffer.set_current_color(floor_color);
        }

        for x in 0..width {
            framebuffer.point(x, y);
        }
    }
}
