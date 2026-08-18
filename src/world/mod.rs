mod collision;
mod entity;
mod level;
mod level_manager;
mod tile;

/// `can_occupy` y `Level`/`LevelError` se re-exportan como `pub`
/// (Tarea 36) porque son la superficie de dominio mínima que las
/// pruebas de integración en `tests/` necesitan para verificar la
/// carga de niveles y las reglas de colisión sin abrir una ventana
/// de Raylib. El resto del módulo (`Entity`, `LevelManager`,
/// `Tile`, ...) permanece `pub(crate)`: ninguna prueba de
/// integración los requiere.
pub use collision::can_occupy;
pub(crate) use entity::{
    Entity, EntityDamageOutcome, EntitySprite, EntityState, EntityStateTransition,
};
pub use level::{Level, LevelError};
pub(crate) use level_manager::{LevelManager, LevelManagerError, LevelTheme};
pub(crate) use tile::Tile;
