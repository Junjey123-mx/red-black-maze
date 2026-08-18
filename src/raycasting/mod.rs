mod caster;
mod fov;
mod hit;
mod hitscan;

/// `cast_ray`/`RayHit`/`cast_hitscan`/`HitscanHit`/`HitscanTarget`/
/// `ray_angle_for_column` se re-exportan como `pub` (Tarea 37):
/// son la superficie mínima de matemática de dominio (geometría de
/// rayos, hitscan, distribución de FOV) que las pruebas de
/// integración en `tests/` necesitan para verificar distancias/
/// impactos/ángulos conocidos sin abrir una ventana de Raylib.
pub use caster::cast_ray;
pub use fov::ray_angle_for_column;
pub use hit::RayHit;
pub use hitscan::{HitscanHit, HitscanTarget, cast_hitscan};
