use super::framebuffer::Framebuffer;
use super::textures::TextureManager;
use crate::player::WeaponState;
use crate::world::LevelTheme;

/// Escala entera de dibujo del arma en primera persona, con
/// muestreo nearest-neighbor (sin antialiasing).
const WEAPON_SCALE: i32 = 3;

/// Dibuja el arma en primera persona como una superposición en
/// espacio de PANTALLA, anclada abajo-centro.
///
/// No es un billboard: no usa coordenadas de mundo, ángulo del
/// jugador, FOV, ni `wall_depth_buffer`. El estado visual ya viene
/// decidido por el llamador (`GameSession`/`Weapon`); este renderer
/// solo lo LEE para elegir la textura correspondiente, nunca
/// modifica el estado del arma.
pub(crate) fn render_weapon(
    framebuffer: &mut Framebuffer,
    textures: &TextureManager,
    state: WeaponState,
    theme: LevelTheme,
) {
    let Some(texture) = textures.themed_weapon_texture(state, theme) else {
        return;
    };

    let texture_width = texture.width();
    let texture_height = texture.height();

    if texture_width <= 0 || texture_height <= 0 {
        return;
    }

    let draw_width = texture_width * WEAPON_SCALE;
    let draw_height = texture_height * WEAPON_SCALE;

    let screen_width = framebuffer.width();
    let screen_height = framebuffer.height();

    let left = screen_width / 2 - draw_width / 2;
    let top = screen_height - draw_height;

    for dest_y in 0..draw_height {
        let screen_y = top + dest_y;

        if screen_y < 0 || screen_y >= screen_height {
            continue;
        }

        let source_y = dest_y / WEAPON_SCALE;

        for dest_x in 0..draw_width {
            let screen_x = left + dest_x;

            if screen_x < 0 || screen_x >= screen_width {
                continue;
            }

            let source_x = dest_x / WEAPON_SCALE;

            let Some(color) = texture.pixel_at(source_x, source_y) else {
                continue;
            };

            if color.a == 0 {
                continue;
            }

            framebuffer.set_current_color(color);

            framebuffer.point(screen_x, screen_y);
        }
    }
}
