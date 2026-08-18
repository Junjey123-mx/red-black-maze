pub mod app;
pub mod audio;
pub mod config;
pub mod game;
pub mod input;
pub mod raycasting;
pub mod rendering;
pub mod ui;
pub mod world;

// Compatibilidad temporal: rendering::map_2d y rendering::world_3d
// todavía dependen de este módulo hasta su migración en tareas futuras.
//
// `pub` desde Tarea 37: `Player` es el tipo que `cast_ray`/
// `cast_hitscan` toman como parámetro; para que esas funciones
// públicas sean genuinamente invocables desde `tests/`, `Player`
// debe ser alcanzable por una ruta pública.
pub mod player;
