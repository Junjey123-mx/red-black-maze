use crate::config::{BLOCK_SIZE, MAP_RAYS, TARGET_FPS};
use crate::game::{GameSession, GameState, ViewMode};
use crate::input::controller::process_events;
use crate::player::Player;
use crate::raycasting::cast_fov;
use crate::rendering::framebuffer::Framebuffer;
use crate::rendering::map_2d::{render_maze, render_player};
use crate::rendering::world_3d::render_world;
use crate::text_load::{Maze, load_maze, validate_maze};
use raylib::prelude::*;

/// Coordina el estado de la aplicación y la sesión de juego activa.
pub(crate) struct App {
    state: GameState,
    session: GameSession,
}

impl App {
    fn new(session: GameSession) -> Self {
        Self {
            state: GameState::Playing,
            session,
        }
    }

    fn update(&mut self, window: &RaylibHandle) {
        match self.state {
            GameState::Playing => self.update_playing(window),

            GameState::Welcome | GameState::LevelSelect | GameState::Victory => {}
        }
    }

    fn update_playing(&mut self, window: &RaylibHandle) {
        /*
         * Movimiento y rotación del jugador.
         */
        process_events(
            window,
            &mut self.session.player,
            &self.session.maze,
            BLOCK_SIZE,
        );

        /*
         * M cambia entre la vista 2D y la vista 3D.
         *
         * Se utiliza is_key_pressed para que cambie
         * solamente una vez por pulsación.
         */
        if window.is_key_pressed(KeyboardKey::KEY_M) {
            self.session.view_mode = match self.session.view_mode {
                ViewMode::Map2D => ViewMode::World3D,

                ViewMode::World3D => ViewMode::Map2D,
            };
        }
    }

    fn render(&self, framebuffer: &mut Framebuffer) {
        match self.state {
            GameState::Playing => self.render_playing(framebuffer),

            GameState::Welcome | GameState::LevelSelect | GameState::Victory => {}
        }
    }

    fn render_playing(&self, framebuffer: &mut Framebuffer) {
        match self.session.view_mode {
            ViewMode::Map2D => {
                /*
                 * Vista superior.
                 */
                render_maze(framebuffer, &self.session.maze, BLOCK_SIZE);

                cast_fov(
                    framebuffer,
                    &self.session.maze,
                    &self.session.player,
                    BLOCK_SIZE,
                    MAP_RAYS,
                );

                render_player(framebuffer, &self.session.player);
            }

            ViewMode::World3D => {
                /*
                 * Vista en primera persona.
                 */
                render_world(
                    framebuffer,
                    &self.session.maze,
                    &self.session.player,
                    BLOCK_SIZE,
                );
            }
        }
    }
}

/// Punto de entrada de la aplicación.
pub fn run() {
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

    let player = match Player::from_maze(&maze, BLOCK_SIZE) {
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

    window.set_target_fps(TARGET_FPS);

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);

    framebuffer.set_background_color(Color::new(12, 12, 16, 255));

    let mut app = App::new(GameSession::new(maze, player));

    while !window.window_should_close() {
        app.update(&window);

        framebuffer.clear();

        app.render(&mut framebuffer);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
