mod caster;
mod controller;
mod framebuffer;
mod maze_renderer;
mod player;
mod text_load;
mod world_renderer;

use caster::cast_fov;
use controller::process_events;
use framebuffer::Framebuffer;
use maze_renderer::render_maze;
use player::{Player, render_player};
use raylib::prelude::*;
use text_load::{Maze, load_maze, validate_maze};
use world_renderer::render_world;

/// Modos de visualización disponibles.
#[derive(Debug, Clone, Copy)]
enum ViewMode {
    Map2D,
    World3D,
}

fn main() {
    const BLOCK_SIZE: usize = 48;

    /*
     * Cantidad de rayos utilizados únicamente para
     * mostrar el abanico en el mapa 2D.
     */
    const MAP_RAYS: usize = 180;

    let maze: Maze = load_maze("./maze.txt");

    if let Err(error) = validate_maze(&maze) {
        eprintln!("Error en maze.txt: {error}");
        return;
    }

    let maze_height = maze.len();
    let maze_width = maze[0].len();

    let framebuffer_width =
        i32::try_from(maze_width * BLOCK_SIZE).expect("El ancho del laberinto es demasiado grande");

    let framebuffer_height = i32::try_from(maze_height * BLOCK_SIZE)
        .expect("La altura del laberinto es demasiado grande");

    let mut player = match Player::from_maze(&maze, BLOCK_SIZE) {
        Ok(player) => player,

        Err(error) => {
            eprintln!("Error al crear al jugador: {error}");
            return;
        }
    };

    let (mut window, raylib_thread) = raylib::init()
        .size(framebuffer_width, framebuffer_height)
        .title("Red-Black Maze")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(60);

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);

    framebuffer.set_background_color(Color::new(12, 12, 16, 255));

    /*
     * Iniciar mostrando el mapa 2D.
     */
    let mut view_mode = ViewMode::Map2D;

    while !window.window_should_close() {
        /*
         * Movimiento y rotación del jugador.
         */
        process_events(&window, &mut player, &maze, BLOCK_SIZE);

        /*
         * M cambia entre la vista 2D y la vista 3D.
         *
         * Se utiliza is_key_pressed para que cambie
         * solamente una vez por pulsación.
         */
        if window.is_key_pressed(KeyboardKey::KEY_M) {
            view_mode = match view_mode {
                ViewMode::Map2D => ViewMode::World3D,

                ViewMode::World3D => ViewMode::Map2D,
            };
        }

        framebuffer.clear();

        match view_mode {
            ViewMode::Map2D => {
                /*
                 * Vista superior.
                 */
                render_maze(&mut framebuffer, &maze, BLOCK_SIZE);

                cast_fov(&mut framebuffer, &maze, &player, BLOCK_SIZE, MAP_RAYS);

                render_player(&mut framebuffer, &player);
            }

            ViewMode::World3D => {
                /*
                 * Vista en primera persona.
                 */
                render_world(&mut framebuffer, &maze, &player, BLOCK_SIZE);
            }
        }

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
