use crate::config::{BLOCK_SIZE, MAP_RAYS, TARGET_FPS};
use crate::game::{GameSession, GameState, ViewMode};
use crate::input::controller::process_events;
use crate::player::Player;
use crate::rendering::TextureManager;
use crate::rendering::framebuffer::Framebuffer;
use crate::rendering::map_2d::{render_fov_rays, render_maze, render_player};
use crate::rendering::render_world_sprites;
use crate::rendering::world_3d::render_world;
use crate::world::LevelManager;
use raylib::prelude::*;

/// Coordina el estado de la aplicación y la sesión de juego activa.
pub(crate) struct App {
    state: GameState,
    level_manager: LevelManager,
    session: GameSession,
    textures: TextureManager,
}

impl App {
    fn new(level_manager: LevelManager, session: GameSession, textures: TextureManager) -> Self {
        Self {
            state: GameState::Playing,
            level_manager,
            session,
            textures,
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
            &self.session.level,
            BLOCK_SIZE,
        );

        /*
         * Avanza la animación de antorcha según el tiempo real
         * transcurrido. Esto es independiente del delta clamped
         * que usa el movimiento del jugador dentro de
         * process_events.
         */
        self.session.update_torch_animation(window.get_frame_time());

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
                render_maze(framebuffer, &self.session.level, BLOCK_SIZE);

                render_fov_rays(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    MAP_RAYS,
                );

                render_player(framebuffer, &self.session.player);
            }

            ViewMode::World3D => {
                /*
                 * Vista en primera persona.
                 */
                let wall_depth_buffer = render_world(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    BLOCK_SIZE,
                    &self.textures,
                );

                render_world_sprites(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    &self.textures,
                    BLOCK_SIZE,
                    self.session.torch_frame_index(),
                    &wall_depth_buffer,
                );
            }
        }
    }
}

/// Punto de entrada de la aplicación.
pub fn run() {
    let mut level_manager = LevelManager::new();

    let level = match level_manager.load(0) {
        Ok(level) => level,

        Err(error) => {
            eprintln!("Error al cargar el nivel inicial: {error}");
            return;
        }
    };

    let maze_height = level.height();
    let maze_width = level.width();

    let framebuffer_width =
        i32::try_from(maze_width * BLOCK_SIZE).expect("El ancho del laberinto es demasiado grande");

    let framebuffer_height = i32::try_from(maze_height * BLOCK_SIZE)
        .expect("La altura del laberinto es demasiado grande");

    let player = Player::from_level(&level, BLOCK_SIZE);

    let mut texture_manager = TextureManager::new();

    if let Err(error) = texture_manager.load_wall_textures() {
        eprintln!("Error al cargar texturas de paredes: {error}");
        return;
    }

    if let Err(error) = texture_manager.load_goal_texture() {
        eprintln!("Error al cargar la textura de la meta: {error}");
        return;
    }

    if let Err(error) = texture_manager.load_torch_textures() {
        eprintln!("Error al cargar las texturas de antorcha: {error}");
        return;
    }

    let (mut window, raylib_thread) = raylib::init()
        .size(framebuffer_width, framebuffer_height)
        .title("Red-Black Maze")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(TARGET_FPS);

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);

    framebuffer.set_background_color(Color::new(12, 12, 16, 255));

    let mut app = App::new(
        level_manager,
        GameSession::new(level, player),
        texture_manager,
    );

    while !window.window_should_close() {
        app.update(&window);

        framebuffer.clear();

        app.render(&mut framebuffer);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
