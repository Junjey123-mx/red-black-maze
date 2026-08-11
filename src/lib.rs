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
// todavía dependen de estos módulos hasta su migración en tareas futuras.
mod player;
mod text_load;
