use crate::audio::AudioManager;
use crate::config::{BLOCK_SIZE, MAP_RAYS, TARGET_FPS};
use crate::game::{GameSession, GameState, ViewMode};
use crate::input::controller::process_events;
use crate::player::Player;
use crate::raycasting::{HitscanHit, HitscanTarget, cast_hitscan};
use crate::rendering::TextureManager;
use crate::rendering::framebuffer::Framebuffer;
use crate::rendering::map_2d::{render_fov_rays, render_maze, render_player};
use crate::rendering::world_3d::render_world;
use crate::rendering::{render_hud, render_minimap, render_weapon, render_world_sprites};
use crate::ui::{LevelSelectScreen, VictoryAction, VictoryScreen, WelcomeScreen};
use crate::world::{Level, LevelManager};
use raylib::prelude::*;

/// Coordina el estado de la aplicación y la sesión de juego activa.
///
/// El parámetro de vida `'aud` es el que impone `AudioManager`
/// (a través de `Music<'aud>`, atada por `raylib-rs` a la
/// `RaylibAudio` que la cargó en `run()`); `App` simplemente lo
/// propaga para poder poseer un único `AudioManager` centralizado.
pub(crate) struct App<'aud> {
    state: GameState,
    level_manager: LevelManager,
    session: GameSession,
    textures: TextureManager,
    welcome: WelcomeScreen,
    level_select: LevelSelectScreen,
    victory: VictoryScreen,
    audio: AudioManager<'aud>,
}

impl<'aud> App<'aud> {
    fn new(
        level_manager: LevelManager,
        session: GameSession,
        textures: TextureManager,
        welcome: WelcomeScreen,
        level_select: LevelSelectScreen,
        victory: VictoryScreen,
        audio: AudioManager<'aud>,
    ) -> Self {
        Self {
            state: GameState::Welcome,
            level_manager,
            session,
            textures,
            welcome,
            level_select,
            victory,
            audio,
        }
    }

    fn update(&mut self, window: &RaylibHandle) {
        /*
         * La música de fondo es independiente del `GameState`: se
         * actualiza exactamente una vez por cuadro, sin importar la
         * pantalla activa, para que el stream siga sonando a través
         * de Welcome/LevelSelect/Playing/Victory. No-op seguro si no
         * hay pista cargada.
         */
        self.audio.update();

        match self.state {
            GameState::Welcome => self.update_welcome(window),

            GameState::LevelSelect => self.update_level_select(window),

            GameState::Playing => self.update_playing(window),

            GameState::Victory => self.update_victory(window),
        }
    }

    /// Avanza únicamente la presentación de Bienvenida (su Juego de
    /// la Vida de fondo) y comprueba la activación de `PLAY`.
    ///
    /// NO ejecuta ninguna actualización de gameplay
    /// (`update_playing`, arma, entidades, cámara): la partida
    /// permanece completamente en pausa/oculta detrás de esta
    /// pantalla.
    fn update_welcome(&mut self, window: &RaylibHandle) {
        self.welcome.update(window.get_frame_time());

        /*
         * ENTER es la activación requerida; SPACE se admite también
         * porque es trivial y no introduce comportamiento de mouse
         * nuevo. Un disparo aceptado solo cambia el estado hacia
         * `LevelSelect` (Tarea 29 implementará esa pantalla); nunca
         * entra directamente a `Playing` ni recrea `GameSession`.
         */
        if window.is_key_pressed(KeyboardKey::KEY_ENTER)
            || window.is_key_pressed(KeyboardKey::KEY_SPACE)
        {
            self.state = GameState::LevelSelect;
        }
    }

    /// Avanza únicamente la presentación de Selección de Nivel (su
    /// propio Juego de la Vida de fondo, independiente del de
    /// Bienvenida) y procesa navegación/activación por teclado.
    ///
    /// Orden determinista de entrada: ESC se comprueba primero y
    /// retorna de inmediato (nunca puede coincidir con una
    /// activación de ENTER en el mismo cuadro); luego navegación;
    /// luego ENTER. NO ejecuta ninguna actualización de gameplay
    /// mientras este menú está activo.
    fn update_level_select(&mut self, window: &RaylibHandle) {
        self.level_select.update(window.get_frame_time());

        if window.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            self.state = GameState::Welcome;

            return;
        }

        if window.is_key_pressed(KeyboardKey::KEY_UP) || window.is_key_pressed(KeyboardKey::KEY_W) {
            self.level_select
                .select_previous(self.level_manager.level_count());
        }

        if window.is_key_pressed(KeyboardKey::KEY_DOWN) || window.is_key_pressed(KeyboardKey::KEY_S)
        {
            self.level_select
                .select_next(self.level_manager.level_count());
        }

        if window.is_key_pressed(KeyboardKey::KEY_ENTER) {
            self.start_selected_level();
        }
    }

    /// Carga el nivel actualmente seleccionado a través de
    /// `LevelManager::load` (única autoridad de rutas de archivo;
    /// esta pantalla nunca conoce una ruta), y solo si la carga
    /// tiene éxito construye un `Player`/`GameSession` COMPLETAMENTE
    /// nuevos y reemplaza `self.session` de forma atómica antes de
    /// entrar a `Playing`.
    ///
    /// `Player::from_level` es infalible en la arquitectura actual
    /// (solo lee el spawn ya validado por `Level::load`), por lo que
    /// no existe un segundo punto de fallo tras la carga del nivel.
    /// Si la carga falla, se reporta el error, `self.session` NO se
    /// toca y el estado permanece en `LevelSelect`.
    fn start_selected_level(&mut self) {
        let index = self.level_select.selected_index();

        let level = match self.level_manager.load(index) {
            Ok(level) => level,

            Err(error) => {
                eprintln!("Error al cargar el nivel seleccionado: {error}");

                return;
            }
        };

        self.replace_session_with_level(level);
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
         * Comprobación de meta DESPUÉS del movimiento de este
         * cuadro: si el jugador acaba de entrar a la celda de meta,
         * la partida termina aquí mismo. `on_enter` reinicia la
         * selección de Victoria según si existe un nivel siguiente
         * (Tarea 30), y el `return` inmediato evita que el resto de
         * esta función (arma, entidades, hitscan, alternar vista)
         * siga ejecutando gameplay sobre un nivel ya completado.
         */
        if self.session.has_reached_goal(BLOCK_SIZE) {
            self.victory.on_enter(self.level_manager.has_next());

            self.state = GameState::Victory;

            return;
        }

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
         * Avanza el temporizador de `Hit` y la reevaluación de
         * proximidad `Idle`/`Alert` de cada Dealer ANTES de procesar
         * el clic de este cuadro, por la misma razón que el arma: un
         * golpe aceptado este cuadro debe comenzar en `Hit` con su
         * temporizador completo y renderizarse como `Hit` en este
         * mismo cuadro, sin que la reevaluación de este mismo cuadro
         * lo consuma primero.
         */
        self.session
            .update_entities(window.get_frame_time(), BLOCK_SIZE);

        /*
         * Clic izquierdo: evento PRESSED (no mantenido), para que
         * un solo clic físico dispare como máximo un intento de
         * disparo. `try_fire_weapon` es la única autoridad sobre si
         * el disparo se acepta (Idle + enfriamiento agotado); un
         * clic rechazado no debe producir ningún hitscan.
         *
         * Tarea 24 alimenta el hitscan únicamente con Dealers VIVOS.
         * Como los `Dead` se filtran, el índice dentro de `targets`
         * ya no coincide necesariamente con el índice dentro de
         * `GameSession.entities()`; `target_entity_indices` conserva
         * esa correspondencia explícita en el mismo orden de
         * iteración (sin ordenar/invertir/deduplicar), de modo que
         * `target_entity_indices[target_index]` siempre resuelve al
         * índice real de la entidad.
         */
        if window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT)
            && self.session.try_fire_weapon()
        {
            let mut targets: Vec<HitscanTarget> = Vec::new();

            let mut target_entity_indices: Vec<usize> = Vec::new();

            for (entity_index, entity) in self.session.entities().iter().enumerate() {
                if entity.is_dead() {
                    continue;
                }

                target_entity_indices.push(entity_index);

                targets.push(HitscanTarget {
                    center: entity.position(),
                    radius: entity.hit_radius(),
                });
            }

            let shot_result = cast_hitscan(&self.session.level, &self.session.player, &targets);

            /*
             * Un impacto de pared/fallo no produce daño de entidad.
             * Un impacto de blanco resuelve exactamente un Dealer:
             * el hitscan ya decidió cuál es el más cercano antes de
             * la pared, así que aquí solo se traduce ese índice
             * filtrado de vuelta al índice real y se aplica el daño
             * controlado a través de `GameSession`.
             */
            if let HitscanHit::Target { target_index, .. } = shot_result {
                if let Some(&entity_index) = target_entity_indices.get(target_index) {
                    self.session.damage_entity(entity_index);
                }
            }
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

    /// Avanza únicamente la presentación de Victoria (su propio
    /// Juego de la Vida de fondo, independiente de las otras
    /// pantallas) y procesa navegación/activación por teclado.
    ///
    /// Orden determinista de entrada: navegación primero, luego
    /// ENTER; como máximo una acción se ejecuta por llamada. NO
    /// ejecuta ninguna actualización de gameplay mientras esta
    /// pantalla está activa: la partida completada permanece
    /// congelada/oculta detrás de ella.
    fn update_victory(&mut self, window: &RaylibHandle) {
        self.victory.update(window.get_frame_time());

        let has_next_level = self.level_manager.has_next();

        if window.is_key_pressed(KeyboardKey::KEY_UP) || window.is_key_pressed(KeyboardKey::KEY_W) {
            self.victory.select_previous(has_next_level);
        }

        if window.is_key_pressed(KeyboardKey::KEY_DOWN) || window.is_key_pressed(KeyboardKey::KEY_S)
        {
            self.victory.select_next(has_next_level);
        }

        if window.is_key_pressed(KeyboardKey::KEY_ENTER) {
            if let Some(action) = self.victory.selected_action(has_next_level) {
                self.perform_victory_action(action);
            }
        }
    }

    /// Ejecuta la acción de Victoria ya resuelta por
    /// `VictoryScreen::selected_action` (nunca `NextLevel` cuando no
    /// hay nivel siguiente: eso ya es `None` antes de llegar aquí).
    fn perform_victory_action(&mut self, action: VictoryAction) {
        match action {
            VictoryAction::NextLevel => match self.level_manager.next() {
                Ok(Some(level)) => self.replace_session_with_level(level),

                // Nivel final: no existe ambigüedad porque la UI ya
                // deshabilita esta acción, pero se maneja de forma
                // segura de todas formas: permanece en Victoria, sin
                // reemplazar la sesión, sin envolver al nivel 1.
                Ok(None) => {}

                Err(error) => {
                    eprintln!("Error al cargar el siguiente nivel: {error}");
                }
            },

            VictoryAction::Retry => match self.level_manager.restart() {
                Ok(level) => self.replace_session_with_level(level),

                Err(error) => {
                    eprintln!("Error al reiniciar el nivel: {error}");
                }
            },

            VictoryAction::MainMenu => {
                self.state = GameState::Welcome;
            }
        }
    }

    /// Construye un `Player`/`GameSession` completamente nuevos a
    /// partir de `level` ya cargado con éxito, reemplaza
    /// `self.session` de forma atómica, y entra a `Playing`.
    ///
    /// Único punto compartido por Selección de Nivel (Tarea 29),
    /// `NEXT LEVEL` y `RETRY` (Tarea 30): los tres solo difieren en
    /// CÓMO obtuvieron `level` (`LevelManager::load`/`next`/
    /// `restart`), nunca en cómo se construye la sesión resultante.
    fn replace_session_with_level(&mut self, level: Level) {
        let player = Player::from_level(&level, BLOCK_SIZE);

        self.session = GameSession::new(level, player, BLOCK_SIZE);

        self.state = GameState::Playing;
    }

    fn render(&self, framebuffer: &mut Framebuffer) {
        match self.state {
            GameState::Welcome => self.welcome.render(framebuffer),

            GameState::LevelSelect => self.level_select.render(framebuffer, &self.level_manager),

            GameState::Playing => self.render_playing(framebuffer),

            GameState::Victory => self
                .victory
                .render(framebuffer, self.level_manager.has_next()),
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

                /*
                 * El minimapa se dibuja al final como superposición
                 * arriba-derecha sobre la vista 3D ya completa; no
                 * es un segundo viewport y no reduce el tamaño del
                 * mundo/framebuffer/proyección.
                 */
                render_minimap(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    BLOCK_SIZE,
                );

                /*
                 * El HUD (vida/munición) se dibuja al final,
                 * abajo-izquierda, leyendo instantáneas primitivas
                 * de estado real ya existente en GameSession; no
                 * posee ni modifica ese estado.
                 */
                render_hud(
                    framebuffer,
                    self.session.player_health(),
                    self.session.weapon_ammo(),
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

    if let Err(error) = texture_manager.load_weapon_textures() {
        eprintln!("Error al cargar las texturas del arma: {error}");
        return;
    }

    if let Err(error) = texture_manager.load_entity_textures() {
        eprintln!("Error al cargar las texturas de entidades: {error}");
        return;
    }

    /*
     * La ventana se muestra al doble de la resolución lógica del
     * framebuffer. `Framebuffer::swap_buffers` ya dibuja su textura
     * escalada al tamaño real de pantalla (`draw_texture_pro`), así
     * que duplicar solo las dimensiones de la ventana no afecta la
     * resolución lógica usada por el raycasting ni por el mapa 2D.
     */
    const WINDOW_SCALE: i32 = 2;

    let (mut window, raylib_thread) = raylib::init()
        .size(
            framebuffer_width * WINDOW_SCALE,
            framebuffer_height * WINDOW_SCALE,
        )
        .title("Red-Black Maze")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    window.set_target_fps(TARGET_FPS);

    /*
     * Captura y oculta el cursor para el control de cámara por
     * mouse. Se captura una única vez antes del bucle de juego,
     * independientemente del estado inicial (`Welcome`): la pantalla
     * de Bienvenida se activa solo por teclado (ENTER/SPACE), por lo
     * que no necesita el cursor visible, y esto preserva el
     * comportamiento de cámara exacto que `Playing` ya tenía.
     */
    window.disable_cursor();

    /*
     * Tarea 29 (requerido): por defecto, Raylib trata ESC como
     * tecla de salida de la ventana (`window_should_close()`
     * reporta true tanto por el ícono de cierre como por ESC). Eso
     * haría que ESC en `LevelSelect` cerrara la aplicación en vez
     * de volver a `Welcome`. `set_exit_key(None)` desactiva
     * únicamente esa asociación ESC->salida; el cierre por el
     * ícono/botón de la ventana no depende de la tecla de salida y
     * sigue funcionando exactamente igual.
     */
    window.set_exit_key(None);

    let mut framebuffer = Framebuffer::new(framebuffer_width, framebuffer_height);

    framebuffer.set_background_color(Color::new(12, 12, 16, 255));

    let welcome = WelcomeScreen::new(framebuffer_width, framebuffer_height);

    let level_select = LevelSelectScreen::new(framebuffer_width, framebuffer_height);

    let victory = VictoryScreen::new(framebuffer_width, framebuffer_height);

    /*
     * Inicialización del dispositivo de audio EXACTAMENTE una vez
     * por ejecución. Si falla (drivers ausentes, dispositivo
     * ocupado, etc.), se reporta una única advertencia y la
     * aplicación continúa sin música: `raylib_audio` queda en `Err`,
     * `audio_device` en `None`, y `AudioManager::new` recibe `None`,
     * degradando todas sus operaciones a no-ops seguros.
     */
    let raylib_audio = RaylibAudio::init_audio_device();

    if let Err(error) = &raylib_audio {
        eprintln!("Error al inicializar el dispositivo de audio: {error}");
    }

    let audio_device = raylib_audio.as_ref().ok();

    let audio = AudioManager::new(audio_device);

    let mut app = App::new(
        level_manager,
        GameSession::new(level, player, BLOCK_SIZE),
        texture_manager,
        welcome,
        level_select,
        victory,
        audio,
    );

    while !window.window_should_close() {
        app.update(&window);

        framebuffer.clear();

        app.render(&mut framebuffer);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
