use crate::config::{BLOCK_SIZE, MAP_RAYS, TARGET_FPS};
use crate::game::{GameSession, GameState, ViewMode};
use crate::input::controller::process_events;
use crate::player::Player;
use crate::raycasting::{HitscanTarget, cast_hitscan};
use crate::rendering::TextureManager;
use crate::rendering::framebuffer::Framebuffer;
use crate::rendering::map_2d::{render_fov_rays, render_maze, render_player};
use crate::rendering::world_3d::render_world;
use crate::rendering::{render_weapon, render_world_sprites};
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
         * Avanza la máquina de estados visual del arma ANTES de
         * procesar el clic de este cuadro, de modo que un disparo
         * aceptado ahora comience en `Fire` con tiempo cero y se
         * renderice como `Fire` en este mismo cuadro, en lugar de
         * consumir inmediatamente el delta_time del cuadro actual.
         */
        self.session.update_weapon(window.get_frame_time());

        /*
         * Clic izquierdo: evento PRESSED (no mantenido), para que
         * un solo clic físico dispare como máximo un intento de
         * disparo. `try_fire_weapon` es la única autoridad sobre si
         * el disparo se acepta (Idle + enfriamiento agotado); un
         * clic rechazado no debe producir ningún hitscan.
         *
         * Tarea 23 alimenta el hitscan con blancos geométricos
         * reales, uno por cada entidad activa de la sesión, en el
         * MISMO orden que `GameSession::entities()` para que
         * `target_index` siga correspondiendo 1:1 a esa lista. El
         * resultado se descarta deliberadamente: aplicar daño,
         * cambiar de estado o eliminar un Dealer pertenece a la
         * Tarea 24.
         */
        if window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
            && self.session.try_fire_weapon()
        {
            let targets: Vec<HitscanTarget> = self
                .session
                .entities()
                .iter()
                .map(|entity| HitscanTarget {
                    center: entity.position(),
                    radius: entity.hit_radius(),
                })
                .collect();

            let _shot_result = cast_hitscan(&self.session.level, &self.session.player, &targets);
        }

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
                    self.session.entities(),
                    &wall_depth_buffer,
                );

                /*
                 * El arma se dibuja SIEMPRE al final, como
                 * superposición en espacio de pantalla, para que
                 * nunca quede oculta por paredes ni sprites de
                 * mundo.
                 */
                render_weapon(framebuffer, &self.textures, self.session.weapon_state());
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

    if let Err(error) = texture_manager.load_weapon_textures() {
        eprintln!("Error al cargar las texturas del arma: {error}");
        return;
    }

    if let Err(error) = texture_manager.load_entity_textures() {
        eprintln!("Error al cargar las texturas de entidades: {error}");
        return;
    }

    let (mut window, raylib_thread) = raylib::init()
        .size(framebuffer_width, framebuffer_height)
        .title("Red-Black Maze")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(TARGET_FPS);

    /*
     * Captura y oculta el cursor para el control de cámara por
     * mouse. El estado actual (Playing) es el único estado en
     * ejecución real, por lo que basta con capturarlo una vez
     * antes del bucle de juego.
     */
    window.disable_cursor();

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);

    framebuffer.set_background_color(Color::new(12, 12, 16, 255));

    let mut app = App::new(
        level_manager,
        GameSession::new(level, player, BLOCK_SIZE),
        texture_manager,
    );

    while !window.window_should_close() {
        app.update(&window);

        framebuffer.clear();

        app.render(&mut framebuffer);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
