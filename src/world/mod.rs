mod ammo_pickup;
mod collision;
mod entity;
mod health_pickup;
mod level;
mod level_generator;
mod level_manager;
mod pathfinding;
mod rng;
mod royal_flush_pickup;
mod tile;

/// `can_occupy` y `Level`/`LevelError` se re-exportan como `pub`
/// (Tarea 36); `LevelManager`/`LevelManagerError` se suman como
/// `pub` en Tarea 37, porque son la superficie de dominio mínima que
/// las pruebas de integración en `tests/` necesitan para verificar
/// la carga de niveles, las reglas de colisión y la progresión
/// restart/next del catálogo sin abrir una ventana de Raylib. El
/// resto del módulo (`Entity`, `LevelTheme`, `Tile`, ...) permanece
/// `pub(crate)`: ninguna prueba de integración los requiere.
pub(crate) use ammo_pickup::AmmoPickup;
pub use collision::can_occupy;
pub(crate) use entity::{
    DEALER_ATTACK_RANGE_CELLS, EnemyKind, Entity, EntityDamageOutcome, EntitySprite, EntityState,
    EntityStateTransition,
};
pub(crate) use health_pickup::HealthPickup;
pub use level::{Level, LevelError};
pub(crate) use level_manager::{HordeHandConfig, LevelTheme};
pub use level_manager::{LevelManager, LevelManagerError};
pub(crate) use pathfinding::DistanceField;
pub(crate) use rng::Rng;
pub(crate) use royal_flush_pickup::RoyalFlushPickup;
pub(crate) use tile::Tile;
