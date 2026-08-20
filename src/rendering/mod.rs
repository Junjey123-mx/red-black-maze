mod background;
mod hud;
mod minimap;
mod palette;
mod sprites;
mod textures;
mod weapon;

pub mod framebuffer;
pub mod map_2d;
pub mod world_3d;

pub(crate) use hud::{render_fps, render_hud};
pub(crate) use minimap::render_minimap;
/// Tarea 41: expuesto fuera de `rendering` para que
/// `ui::level_select` pueda resolver el acento cromático de cada
/// fila del catálogo desde la MISMA fuente de verdad que ya usan
/// los renderers de gameplay — sin duplicar ningún literal de color.
pub(crate) use palette::palette_for_theme;
pub(crate) use sprites::render_world_sprites;
pub(crate) use textures::TextureManager;
pub(crate) use weapon::render_weapon;
