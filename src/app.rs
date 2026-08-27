use crate::audio::{AudioManager, MusicTrack, SoundEffect, music_track_for_theme};
use crate::config::{BLOCK_SIZE, FRAMEBUFFER_HEIGHT, FRAMEBUFFER_WIDTH, MAP_RAYS, TARGET_FPS};
use crate::game::{GameSession, GameState, HandHudMessage, ViewMode};
use crate::input::controller::process_events;
use crate::player::Player;
use crate::raycasting::{HitscanHit, HitscanTarget, cast_hitscan};
use crate::rendering::TextureManager;
use crate::rendering::framebuffer::Framebuffer;
use crate::rendering::map_2d::{
    compute_display_cell_size, render_fov_rays, render_maze, render_player,
};
use crate::rendering::world_3d::render_world;
use crate::rendering::{
    render_fps, render_hand_message, render_hit_flash_overlay, render_hud, render_minimap,
    render_weapon, render_world_sprites,
};
use crate::ui::{
    DefeatMenuItem, DefeatScreen, LevelSelectScreen, PauseMenuItem, PauseScreen, VictoryAction,
    VictoryScreen, WelcomeScreen,
};
use crate::world::{EntityDamageOutcome, EntityState, Level, LevelManager};
use raylib::prelude::*;

/// Umbral de desplazamiento (al cuadrado, en píxeles^2) por encima
/// del cual un cuadro cuenta como "el jugador se movió realmente"
/// para efectos de pasos. Filtra el ruido de punto flotante de un
/// movimiento bloqueado por colisión (posición idéntica) sin exigir
/// una igualdad exacta.
const FOOTSTEP_MOVEMENT_EPSILON_SQUARED: f32 = 0.001;

/// Traduce la posición actual del mouse (coordenadas reales de
/// ventana, las que expone `RaylibHandle`) a coordenadas lógicas del
/// framebuffer (`FRAMEBUFFER_WIDTH x FRAMEBUFFER_HEIGHT`), la MISMA
/// resolución que ya usan `compute_layout`/`render` de cada pantalla.
///
/// La ventana se dibuja al doble de la resolución lógica
/// (`WINDOW_SCALE` en `run()`) mediante `draw_texture_pro` mapeando
/// el framebuffer COMPLETO al área de pantalla completa (Tarea 38);
/// esta función invierte esa misma transformación a partir del
/// tamaño de pantalla real reportado por Raylib, en vez de asumir un
/// factor de escala fijo, para seguir siendo correcta aunque la
/// ventana cambiara de tamaño en el futuro.
fn mouse_position_in_framebuffer(
    window: &RaylibHandle,
    framebuffer_width: i32,
    framebuffer_height: i32,
) -> (i32, i32) {
    let screen_width = (window.get_screen_width().max(1)) as f32;
    let screen_height = (window.get_screen_height().max(1)) as f32;

    let mouse = window.get_mouse_position();

    let x = (mouse.x / screen_width * framebuffer_width as f32) as i32;
    let y = (mouse.y / screen_height * framebuffer_height as f32) as i32;

    (x, y)
}

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
    pause: PauseScreen,
    defeat: DefeatScreen,
    audio: AudioManager<'aud>,

    /// FPS real, leído de Raylib (`RaylibHandle::get_fps`) durante
    /// `update_playing` y solo DIBUJADO durante `render` — preserva
    /// la separación update -> render: el renderer nunca conoce
    /// `RaylibHandle` directamente. Raylib ya promedia esta lectura
    /// internamente (no es un valor instantáneo de un único cuadro),
    /// así que no se necesita suavizado adicional propio.
    current_fps: u32,
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
            pause: PauseScreen::new(),
            defeat: DefeatScreen::new(),
            audio,
            current_fps: 0,
        }
    }

    fn update(&mut self, window: &mut RaylibHandle) {
        /*
         * La música de fondo y los cooldowns anti-spam de audio son
         * independientes del `GameState`: se actualizan exactamente
         * una vez por cuadro, sin importar la pantalla activa, para
         * que el stream siga sonando a través de
         * Welcome/LevelSelect/Playing/Victory. No-op seguro si no
         * hay pista/efectos cargados.
         */
        self.audio.update(window.get_frame_time());

        let previous_state = self.state;

        match self.state {
            GameState::Welcome => self.update_welcome(window),

            GameState::LevelSelect => self.update_level_select(window),

            GameState::Playing => self.update_playing(window),

            GameState::Victory => self.update_victory(window),

            GameState::Paused => self.update_paused(window),

            GameState::Defeat => self.update_defeat(window),
        }

        self.sync_cursor_capture(window, previous_state);
    }

    /// Mantiene el ciclo de vida del cursor atado al `GameState`
    /// actual, en vez de capturarlo una única vez de forma global al
    /// arrancar la aplicación (comportamiento previo a Tarea 38.C,
    /// que dejaba el cursor permanentemente inutilizable en
    /// Welcome/LevelSelect/Victory).
    ///
    /// Al ENTRAR a `Playing` (transición detectada este mismo
    /// cuadro), captura el cursor para el control de cámara por
    /// mouse; al SALIR de `Playing` hacia cualquier otro estado, lo
    /// libera para que los menús vuelvan a ser utilizables.
    ///
    /// Mientras `Playing` permanece activo (sin transición), esta
    /// función reafirma la captura DEFENSIVAMENTE solo si
    /// `is_cursor_hidden()` reporta que en algún momento dejó de
    /// estarlo: un gestor de ventanas puede liberar la captura del
    /// cursor por eventos de foco (alt-tab, cambio de espacio de
    /// trabajo) sin que el estado interno de `raylib-rs` se entere
    /// por sí solo. Esto es una comprobación barata (un booleano) en
    /// el caso común, no una llamada a `disable_cursor` repetida sin
    /// necesidad cada cuadro.
    ///
    /// Tarea 38.C.1: un experimento posterior a la auditoría de T38.C
    /// reemplazó temporalmente esta ruta por recentrado manual
    /// (`hide_cursor`/`set_mouse_position` cada cuadro), sospechando
    /// que `disable_cursor` no activaba de forma fiable el modo
    /// relativo de la plataforma. La prueba manual del usuario
    /// confirmó que la causa raíz real era ajena al juego: la función
    /// "Desactivar mientras se escribe" de libinput/GNOME en el
    /// sistema del usuario, no esta ruta de entrada. Por eso esta
    /// función vuelve a `disable_cursor`/`enable_cursor`.
    ///
    /// Tarea 42: `GameState::Paused` reutiliza esta MISMA lógica sin
    /// ningún caso especial nuevo. `Paused != Playing`, así que
    /// `Playing -> Paused` ya cae en `left_playing` (libera el
    /// cursor) y `Paused -> Playing` ya cae en `entered_playing`
    /// (recaptura), incluyendo el recentrado interno que
    /// `disable_cursor` ya hace antes de bloquear el cursor — el
    /// mismo mecanismo que evita el salto de cámara al reanudar
    /// desde cualquier otro estado, sin un segundo sistema de
    /// captura. `Paused -> Welcome` (EXIT TO MENU) no activa ninguna
    /// rama (ni `entered_playing` ni `left_playing`, porque
    /// `previous_state` ya era `Paused`, no `Playing`), así que el
    /// cursor simplemente permanece liberado — correcto, porque ya
    /// se liberó en la transición `Playing -> Paused` anterior.
    fn sync_cursor_capture(&self, window: &mut RaylibHandle, previous_state: GameState) {
        let entered_playing = self.state == GameState::Playing && previous_state != self.state;

        let left_playing = self.state != GameState::Playing && previous_state == GameState::Playing;

        if entered_playing {
            window.disable_cursor();
        } else if left_playing {
            window.enable_cursor();
        } else if self.state == GameState::Playing && !window.is_cursor_hidden() {
            window.disable_cursor();
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
         * Mouse: hover resalta `PLAY` con el mismo acento de
         * "selección" que el resto de los menús (`set_hovered`, leído
         * únicamente por `render`); un clic izquierdo dentro de la
         * MISMA hitbox es una activación EQUIVALENTE a ENTER/SPACE,
         * nunca un segundo camino de transición — se combina en la
         * misma condición de abajo.
         */
        let (mouse_x, mouse_y) =
            mouse_position_in_framebuffer(window, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT);

        let hovering_play =
            self.welcome
                .hit_test(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, mouse_x, mouse_y);

        self.welcome.set_hovered(hovering_play);

        let clicked_play =
            hovering_play && window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        /*
         * ENTER es la activación requerida; SPACE se admite también
         * porque es trivial y no introduce comportamiento de mouse
         * nuevo. Un disparo aceptado solo cambia el estado hacia
         * `LevelSelect` (Tarea 29 implementará esa pantalla); nunca
         * entra directamente a `Playing` ni recrea `GameSession`.
         */
        if window.is_key_pressed(KeyboardKey::KEY_ENTER)
            || window.is_key_pressed(KeyboardKey::KEY_SPACE)
            || clicked_play
        {
            self.state = GameState::LevelSelect;

            self.audio.play_sound(SoundEffect::MenuSelect);
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
            let selection_before = self.level_select.selected_index();

            self.level_select
                .select_previous(self.level_manager.level_count());

            if self.level_select.selected_index() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_DOWN) || window.is_key_pressed(KeyboardKey::KEY_S)
        {
            let selection_before = self.level_select.selected_index();

            self.level_select
                .select_next(self.level_manager.level_count());

            if self.level_select.selected_index() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        /*
         * Izquierda/Derecha alternan el modo de juego (`GameMode`),
         * un eje de navegación completamente independiente de
         * Arriba/Abajo (nivel): ninguna de las dos teclas se usaba
         * dentro de este menú antes de esta tarea, así que no hay
         * conflicto que resolver con un "modo de foco" — ambos ejes
         * están siempre activos a la vez.
         */
        if window.is_key_pressed(KeyboardKey::KEY_LEFT) || window.is_key_pressed(KeyboardKey::KEY_A)
        {
            let mode_before = self.level_select.selected_mode();

            self.level_select.select_mode_previous();

            if self.level_select.selected_mode() != mode_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_RIGHT)
            || window.is_key_pressed(KeyboardKey::KEY_D)
        {
            let mode_before = self.level_select.selected_mode();

            self.level_select.select_mode_next();

            if self.level_select.selected_mode() != mode_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        /*
         * Mouse: mismo patrón exacto que `update_paused`/
         * `update_victory`/`update_defeat` — hover mueve la selección
         * (`set_selected_index`, la MISMA fuente de verdad que el
         * teclado) solo cuando el mouse realmente se movió este cuadro
         * o al hacer clic; clic izquierdo confirma la fila bajo el
         * cursor. `hit_test` ya se limita a `0..level_count`, así que
         * nunca puede seleccionar ni activar una fila fuera del
         * catálogo actual.
         */
        let level_count = self.level_manager.level_count();

        let mouse_delta = window.get_mouse_delta();

        let mouse_moved = mouse_delta.x != 0.0 || mouse_delta.y != 0.0;

        let (mouse_x, mouse_y) =
            mouse_position_in_framebuffer(window, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT);

        let hovered_index = self.level_select.hit_test(
            FRAMEBUFFER_WIDTH,
            FRAMEBUFFER_HEIGHT,
            mouse_x,
            mouse_y,
            level_count,
        );

        let left_clicked = window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        if let Some(index) = hovered_index {
            if (mouse_moved || left_clicked) && index != self.level_select.selected_index() {
                self.level_select.set_selected_index(index);

                if !left_clicked {
                    self.audio.play_sound(SoundEffect::MenuMove);
                }
            }
        }

        /*
         * Mismo patrón para las cajas `PORTAL`/`HORDE`: hover/clic
         * escriben la MISMA `selected_mode` que ←/→ ya escriben por
         * teclado (`set_mode`, nunca un campo paralelo). A diferencia
         * de una fila de nivel, un clic aquí SOLO cambia el modo —
         * nunca lanza la partida: confirmar sigue siendo
         * exclusivamente Enter o el clic sobre una fila de nivel
         * (abajo), reutilizando `start_selected_level` sin duplicar
         * esa transición.
         */
        let hovered_mode = self.level_select.hit_test_mode(
            FRAMEBUFFER_WIDTH,
            FRAMEBUFFER_HEIGHT,
            mouse_x,
            mouse_y,
        );

        if let Some(mode) = hovered_mode {
            if (mouse_moved || left_clicked) && mode != self.level_select.selected_mode() {
                self.level_select.set_mode(mode);

                if !left_clicked {
                    self.audio.play_sound(SoundEffect::MenuMove);
                }
            }
        }

        /*
         * La confirmación `MenuSelect` corresponde a la ACTIVACIÓN
         * del menú (la pulsación de ENTER en sí, o el clic
         * equivalente), no al éxito de la carga: incluso si
         * `start_selected_level` falla y reporta su propio error, el
         * usuario sí activó la acción.
         */
        if window.is_key_pressed(KeyboardKey::KEY_ENTER)
            || (left_clicked && hovered_index.is_some())
        {
            self.audio.play_sound(SoundEffect::MenuSelect);

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
         * FPS real de Raylib, leído una vez por cuadro aquí (nunca
         * en `render`, que no recibe `RaylibHandle`). `get_fps` ya
         * es un promedio de Raylib sobre una ventana de cuadros
         * recientes, no una lectura instantánea de un solo cuadro.
         */
        self.current_fps = window.get_fps();

        /*
         * Tarea 42: ESC pausa la partida. Comprobado ANTES de
         * cualquier otra lectura de entrada/actualización jugable de
         * este cuadro (mismo patrón que el ESC de
         * `update_level_select`) — un ESC este cuadro no debe además
         * mover al jugador, disparar, ni avanzar ningún timer. La
         * sesión (`self.session`) no se toca en absoluto: `Paused`
         * es la MISMA partida, solo con un `GameState` distinto y
         * `update_playing` dejando de invocarse mientras dure. Como
         * NINGÚN temporizador jugable (arma/recarga/entidades/
         * antorcha/pasos) usa reloj absoluto — todos avanzan
         * exclusivamente vía el `delta_time` que este mismo método
         * les entrega cuadro a cuadro — simplemente no llamar a este
         * método mientras `Paused` esté activo congela TODA la
         * simulación jugable sin necesitar un mecanismo de pausa por
         * subsistema.
         */
        if window.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            self.state = GameState::Paused;

            self.pause.on_enter();

            self.audio.pause_music();

            return;
        }

        /*
         * Tarea 46.B, sección 7: guardia defensiva. El flujo normal
         * de T45/T46 ya hace estructuralmente imposible reentrar a
         * `update_playing` con la vida ya en `0` (el cuadro exacto en
         * que la vida llega a `0` ya transiciona a `Defeat` más abajo
         * y retorna), pero esta comprobación mantiene la función
         * robusta por sí misma en vez de depender silenciosamente de
         * esa garantía externa: si de todas formas ocurriera, no se
         * permite movimiento/pickups/disparo/reload/Victoria este
         * cuadro, la sesión NO se toca en absoluto, y se entra a
         * `Defeat` de inmediato.
         */
        self.state = self
            .state
            .resolve_playing_terminal_state(self.session.player_health(), false);

        if self.state == GameState::Defeat {
            self.defeat.on_enter();

            self.audio.set_music(MusicTrack::Defeat);

            return;
        }

        /*
         * Movimiento y rotación del jugador. Se observa la posición
         * antes/después para alimentar la cadencia de pasos con
         * desplazamiento REAL: un jugador empujando contra una pared
         * no produce una posición distinta, así que no cuenta como
         * movimiento aunque W/A/S/D esté mantenido.
         */
        let position_before = self.session.player.pos;

        process_events(
            window,
            &mut self.session.player,
            &self.session.level,
            BLOCK_SIZE,
        );

        let position_after = self.session.player.pos;

        let dx = position_after.x - position_before.x;

        let dy = position_after.y - position_before.y;

        let moved = (dx * dx + dy * dy) > FOOTSTEP_MOVEMENT_EPSILON_SQUARED;

        self.audio.update_footsteps(moved, window.get_frame_time());

        /*
         * Tarea 46.B: la meta se COMPRUEBA aquí, justo después del
         * movimiento de este cuadro (misma posición que antes), pero
         * la transición YA NO se decide ni se ejecuta en este punto.
         * Antes de esta tarea, un `return` inmediato aquí permitía
         * que el jugador ganara la partida sin que el combate de
         * Dealer de este MISMO cuadro llegara siquiera a resolverse
         * — por lo que un golpe letal simultáneo nunca podía competir
         * con la Victoria. Ahora `reached_goal` se recuerda en una
         * variable local y la decisión real (con la prioridad
         * obligatoria de Defeat sobre Victory) se aplica DESPUÉS de
         * que el daño de Dealer de este cuadro (más abajo) ya se haya
         * aplicado, mediante `GameState::resolve_playing_terminal_state`.
         */
        let reached_goal = self.session.has_reached_goal(BLOCK_SIZE);

        /*
         * Avanza la animación de antorcha según el tiempo real
         * transcurrido. Esto es independiente del delta clamped
         * que usa el movimiento del jugador dentro de
         * process_events.
         */
        self.session.update_torch_animation(window.get_frame_time());

        /*
         * Tarea 44: recolección de munición cercana. Vive aquí,
         * dentro de `update_playing` (el ÚNICO llamador), para que
         * `App::update_paused` (Tarea 42) — que simplemente no
         * invoca este método mientras `GameState::Paused` esté
         * activo — congele la recolección automáticamente, sin
         * ningún caso especial nuevo. `collect_nearby_ammo_pickups`
         * es la única autoridad sobre qué pickup se consume y cuánta
         * reserva se añade; `App` no repite esa lógica.
         *
         * Tarea "Ammo Pickup SFX": el conteo retornado es el ÚNICO
         * evento semántico de "recolección exitosa" — se solicita
         * `SoundEffect::AmmoPickup` exactamente una vez POR PICKUP
         * consumido este cuadro (nunca por simple proximidad a uno
         * todavía activo, ni una segunda vez por el mismo pickup, que
         * `AmmoPickup::deactivate` ya deja inactivo de forma
         * permanente para esta sesión).
         */
        for _ in 0..self.session.collect_nearby_ammo_pickups() {
            self.audio.play_sound(SoundEffect::AmmoPickup);
        }

        /*
         * Health Pickup: mismo patrón exacto que la recolección de
         * munición de arriba — vive aquí, dentro de `update_playing`
         * (el ÚNICO llamador), para que Pause/Victory/Defeat lo
         * congelen automáticamente sin ningún caso especial nuevo.
         * `collect_nearby_health_pickups` ya decide por sí sola si
         * hay algo que curar (vida < máximo) y si el pickup se
         * consume; el conteo retornado es el ÚNICO evento semántico
         * de "curación exitosa".
         */
        for _ in 0..self.session.collect_nearby_health_pickups() {
            self.audio.play_sound(SoundEffect::HealthPickup);
        }

        /*
         * Emergency Ammo Respawn: anti-softlock, no regeneración
         * pasiva. Vive aquí (dentro de `update_playing`, el ÚNICO
         * llamador) por el mismo motivo que la recolección de
         * pickups de arriba — Pause/Victory/Defeat lo congelan
         * automáticamente sin ningún caso especial. A diferencia de
         * la recolección, esto NUNCA reproduce sonido: el spawn es
         * silencioso, `SoundEffect::AmmoPickup` sigue perteneciendo
         * exclusivamente a la recolección.
         */
        self.session.ensure_emergency_ammo(BLOCK_SIZE);

        /*
         * Avanza la máquina de estados visual del arma ANTES de
         * procesar el clic de este cuadro, de modo que un disparo
         * aceptado ahora comience en `Fire` con tiempo cero y se
         * renderice como `Fire` en este mismo cuadro, en lugar de
         * consumir inmediatamente el delta_time del cuadro actual.
         */
        self.session.update_weapon(window.get_frame_time());

        /*
         * Tecla R: evento PRESSED (no mantenido/`is_key_down`), para
         * que mantener R presionada no reinicie continuamente el
         * temporizador de recarga. Es un canal de entrada
         * INDEPENDIENTE del movimiento/rotación (ya procesados
         * arriba en `process_events`) y del clic de disparo (más
         * abajo): sostener W, mover el mouse, Y presionar R en el
         * mismo cuadro inician movimiento + rotación + recarga sin
         * que ninguno bloquee a los otros. `try_start_weapon_reload`
         * es la única autoridad sobre si la recarga se acepta
         * (`Idle`, cargador no lleno, reserva > 0); un intento
         * rechazado no altera ningún estado.
         *
         * Tarea 43: `SoundEffect::Reload` suena EXACTAMENTE cuando
         * `try_start_weapon_reload` retorna `true` — el mismo booleano
         * que ya decidía la mecánica, sin una segunda comprobación
         * manual de `magazine < capacity && reserve > 0` aquí. Un
         * `R` rechazado (cargador lleno, reserva agotada, o ya
         * recargando) no produce sonido; mantener `WeaponState::Reload`
         * en cuadros posteriores tampoco, porque este bloque solo se
         * evalúa en el cuadro exacto de la pulsación (`is_key_pressed`,
         * no un chequeo continuo del estado).
         */
        if window.is_key_pressed(KeyboardKey::KEY_R) && self.session.try_start_weapon_reload() {
            self.audio.play_sound(SoundEffect::Reload);
        }

        /*
         * Avanza el temporizador de `Hit` y la reevaluación de
         * proximidad `Idle`/`Alert` de cada Dealer ANTES de procesar
         * el clic de este cuadro, por la misma razón que el arma: un
         * golpe aceptado este cuadro debe comenzar en `Hit` con su
         * temporizador completo y renderizarse como `Hit` en este
         * mismo cuadro, sin que la reevaluación de este mismo cuadro
         * lo consuma primero.
         */
        /*
         * `update_entities` reporta solo las transiciones de estado
         * que REALMENTE ocurrieron este cuadro (dominio puro, sin
         * vocabulario de audio). Aquí, y únicamente aquí, se traduce
         * cada transición de reconocimiento/recuperación a su efecto
         * de sonido: `-> Alert` sí, `-> Idle` sí; `-> Hit`/`-> Dead`
         * NO se mapean desde esta vía porque esos ya se resuelven
         * como resultado explícito de `damage_entity` más abajo.
         */
        for transition in self
            .session
            .update_entities(window.get_frame_time(), BLOCK_SIZE)
        {
            match transition.to {
                EntityState::Alert => self.audio.play_sound(SoundEffect::EnemyAlert),

                EntityState::Idle => self.audio.play_sound(SoundEffect::EnemyIdle),

                EntityState::Hit | EntityState::Dead => {}
            }
        }

        /*
         * Tarea 45: ataques de Dealer. El temporizador del flash se
         * avanza PRIMERO (con el tiempo real de este cuadro, antes
         * de que un golpe de este MISMO cuadro pueda reiniciarlo),
         * y luego se resuelven los ataques de TODOS los Dealers.
         * `process_dealer_attacks` es la única autoridad sobre
         * cuánta vida se resta y sobre cuándo reinicia el flash
         * (`GameSession::hit_flash`); `App` solo decide, a partir
         * del daño TOTAL agregado que retorna, si reproducir
         * `SoundEffect::PlayerHit` — como mucho una vez por cuadro,
         * sin importar cuántos Dealers golpearon. Un total de `0`
         * (ningún Dealer atacó, o la vida ya estaba en `0`) no
         * reproduce nada.
         */
        self.session.update_hit_flash(window.get_frame_time());

        let player_damage_this_frame = self
            .session
            .process_dealer_attacks(window.get_frame_time(), BLOCK_SIZE);

        if player_damage_this_frame > 0 {
            self.audio.play_sound(SoundEffect::PlayerHit);
        }

        /*
         * Dealer Hands: countdown de "The House is reloading" y
         * spawn de la siguiente Hand cuando corresponda. Llamado
         * exclusivamente aquí, dentro de `update_playing` (mismo
         * patrón que `process_dealer_attacks`/
         * `collect_nearby_ammo_pickups`), para que Pause/Victory/
         * Defeat lo congelen automáticamente sin ningún caso
         * especial: esos estados simplemente no vuelven a invocar
         * `update_playing`. `level_cap`/`use_clusters` son identidad
         * del NIVEL (LevelManager), no de la sesión — `GameSession`
         * los recibe como parámetros y no conoce `LevelTheme`.
         */
        self.session.update_hand_state(
            window.get_frame_time(),
            BLOCK_SIZE,
            self.level_manager.current_dealer_cap(),
            self.level_manager.current_is_procedural(),
        );

        /*
         * Tarea 46.B: resolución terminal ÚNICA de este cuadro, ahora
         * que tanto `reached_goal` (recordado arriba, antes de
         * combate) como el daño de Dealer de ESTE MISMO cuadro (justo
         * arriba) ya están disponibles. `resolve_playing_terminal_state`
         * es la ÚNICA regla de decisión — nunca dos comprobaciones
         * independientes con un `return` intermedio entre ellas — y
         * garantiza `Defeat` sobre `Victory` cuando ambas condiciones
         * se cumplen en el mismo cuadro. El `return` detiene aquí
         * cualquier acción jugable adicional de este cuadro (disparo,
         * reload, alternar vista) sin necesidad de banderas de estado
         * nuevas.
         */
        let previous_state = self.state;

        self.state = previous_state
            .resolve_playing_terminal_state(self.session.player_health(), reached_goal);

        if self.state != previous_state {
            match self.state {
                GameState::Defeat => {
                    /*
                     * Tarea 46, sección 22: sin SFX nuevo para
                     * derrota. El `player_hit.wav` de Tarea 45 ya
                     * sonó (arriba) si el golpe que causó la muerte
                     * era daño real; esta transición en sí NO
                     * reproduce ningún SFX adicional.
                     *
                     * Tarea "Victory/Defeat music": la música del
                     * nivel se detiene por completo y `defeat.mp3`
                     * ("The House Always Wins") arranca desde el
                     * principio — `set_music` ya se encarga de parar
                     * la pista anterior antes de empezar la nueva, y
                     * como el nivel nunca deja `current_track` en
                     * `Defeat` fuera de esta pantalla, siempre
                     * arranca limpia, nunca a mitad de reproducción.
                     */
                    self.defeat.on_enter();

                    self.audio.set_music(MusicTrack::Defeat);
                }

                GameState::Victory => {
                    /*
                     * Solo se alcanza aquí cuando `resolve_playing_terminal_state`
                     * decidió Victory — es decir, la vida seguía > 0
                     * este cuadro (Tarea 46.B, sección 12): un golpe
                     * letal simultáneo ya se resolvió como `Defeat`
                     * arriba y nunca llega a esta rama.
                     *
                     * Tarea "Victory/Defeat music": la música del
                     * nivel se detiene y `victory.mp3` ("The House
                     * Has Fallen") arranca desde el principio, igual
                     * en las CUATRO victorias del juego (Crimson/
                     * Black/House/True Maze todas entran a este MISMO
                     * `GameState::Victory` — la única diferencia entre
                     * ellas es si `NEXT LEVEL` queda habilitado). El
                     * SFX de Victoria (`SoundEffect::Victory`) sigue
                     * siendo una responsabilidad totalmente separada
                     * de la música y no se toca.
                     */
                    self.victory.on_enter(self.level_manager.has_next());

                    self.audio.set_music(MusicTrack::Victory);

                    self.audio.play_sound(SoundEffect::Victory);
                }

                _ => {}
            }

            return;
        }

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
            /*
             * `Shoot` representa el disparo del arma en sí, ya
             * aceptado por `try_fire_weapon` (cooldown agotado y
             * arma `Idle`). Suena exactamente una vez aquí,
             * independientemente de qué resuelva después el
             * hitscan: un disparo aceptado con impacto de pared o de
             * Dealer todavía produce exactamente un `Shoot`.
             */
            self.audio.play_sound(SoundEffect::Shoot);

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
             * Un impacto de pared produce `WallHit` y ningún daño de
             * entidad. Un impacto de blanco resuelve exactamente un
             * Dealer: el hitscan ya decidió cuál es el más cercano
             * antes de la pared, así que aquí solo se traduce ese
             * índice filtrado de vuelta al índice real y se aplica
             * el daño controlado a través de `GameSession`, cuyo
             * resultado semántico (`EntityDamageOutcome`) decide
             * `EnemyHit` (no letal) o `EnemyDeath` (letal) sin
             * inferirlo del `EntityState` resultante. Un golpe letal
             * nunca también reproduce `EnemyHit`.
             */
            match shot_result {
                HitscanHit::Target { target_index, .. } => {
                    if let Some(&entity_index) = target_entity_indices.get(target_index) {
                        match self.session.damage_entity(entity_index) {
                            EntityDamageOutcome::Hit => {
                                self.audio.play_sound(SoundEffect::EnemyHit);
                            }

                            EntityDamageOutcome::Killed => {
                                self.audio.play_sound(SoundEffect::EnemyDeath);
                            }

                            EntityDamageOutcome::None => {}
                        }
                    }
                }

                HitscanHit::Wall(_) => {
                    self.audio.play_sound(SoundEffect::WallHit);
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

    /// Procesa el menú de pausa: ESC reanuda incondicionalmente
    /// (sin importar qué opción esté resaltada); arriba/abajo (flecha
    /// o W/S, misma convención que Selección de Nivel/Victoria)
    /// mueve la selección entre `CONTINUE`/`EXIT TO MENU`; ENTER
    /// ejecuta la opción resaltada.
    ///
    /// NO llama a `update_playing` ni a ningún método de
    /// `GameSession`/`Weapon`/`Entity`: `self.session` permanece
    /// exactamente como estaba en el cuadro en que se pausó. Tampoco
    /// avanza `self.audio.update_footsteps` (solo se llama dentro de
    /// `update_playing`), por lo que ningún paso nuevo puede sonar
    /// mientras la pausa está activa.
    fn update_paused(&mut self, window: &RaylibHandle) {
        if window.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            self.state = GameState::Playing;

            self.audio.play_music();

            return;
        }

        if window.is_key_pressed(KeyboardKey::KEY_UP) || window.is_key_pressed(KeyboardKey::KEY_W) {
            let selection_before = self.pause.selected_item();

            self.pause.select_previous();

            if self.pause.selected_item() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_DOWN) || window.is_key_pressed(KeyboardKey::KEY_S)
        {
            let selection_before = self.pause.selected_item();

            self.pause.select_next();

            if self.pause.selected_item() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        /*
         * Mouse: hover mueve la selección y clic izquierdo confirma
         * la opción bajo el cursor. Fuente de verdad ÚNICA con el
         * teclado (`self.pause.selected`, vía `set_selected`); nunca
         * un segundo índice paralelo. El hover solo mueve la
         * selección cuando el mouse REALMENTE se desplazó este cuadro
         * (o al hacer clic, una intención explícita incluso con el
         * cursor quieto): así, mantener el cursor inmóvil sobre una
         * fila nunca le "gana" a `↑`/`↓` en cuadros posteriores.
         */
        let mouse_delta = window.get_mouse_delta();

        let mouse_moved = mouse_delta.x != 0.0 || mouse_delta.y != 0.0;

        let (mouse_x, mouse_y) =
            mouse_position_in_framebuffer(window, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT);

        let hovered_item =
            self.pause
                .hit_test(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, mouse_x, mouse_y);

        let left_clicked = window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        if let Some(item) = hovered_item {
            if (mouse_moved || left_clicked) && item != self.pause.selected_item() {
                self.pause.set_selected(item);

                if !left_clicked {
                    self.audio.play_sound(SoundEffect::MenuMove);
                }
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_ENTER) || (left_clicked && hovered_item.is_some())
        {
            self.audio.play_sound(SoundEffect::MenuSelect);

            match self.pause.selected_item() {
                PauseMenuItem::Continue => {
                    self.state = GameState::Playing;

                    self.audio.play_music();
                }

                /*
                 * `self.session` NO se destruye ni se reinicia aquí:
                 * exactamente el mismo lifecycle que `MainMenu` desde
                 * Victoria (`perform_victory_action`), que tampoco
                 * toca la sesión — simplemente dejar de estar en
                 * `Playing`/`Paused` es suficiente para que
                 * `update`/`render` dejen de tocarla, y la próxima
                 * partida (`start_selected_level`/`next`/`restart`)
                 * la reemplaza atómicamente de todas formas.
                 *
                 * Tarea 46.5, sección 4: la pista del nivel se
                 * detiene (nunca queda sonando detrás del menú) y
                 * `Menu Music` arranca en loop.
                 */
                PauseMenuItem::ExitToMenu => {
                    self.state = GameState::Welcome;

                    self.audio.set_music(MusicTrack::Menu);
                }
            }
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
            let selection_before = self.victory.selected_index();

            self.victory.select_previous(has_next_level);

            if self.victory.selected_index() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_DOWN) || window.is_key_pressed(KeyboardKey::KEY_S)
        {
            let selection_before = self.victory.selected_index();

            self.victory.select_next(has_next_level);

            if self.victory.selected_index() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        /*
         * Mouse: mismo patrón exacto que `update_paused` — hover
         * mueve la selección (`set_selected_index`, la MISMA fuente
         * de verdad que el teclado) solo cuando el mouse realmente se
         * movió este cuadro o al hacer clic; clic izquierdo confirma
         * la fila bajo el cursor. `hit_test` ya excluye `NEXT LEVEL`
         * cuando `has_next_level` es `false`, así que esa fila nunca
         * puede seleccionarse ni activarse por mouse mientras está
         * deshabilitada.
         */
        let mouse_delta = window.get_mouse_delta();

        let mouse_moved = mouse_delta.x != 0.0 || mouse_delta.y != 0.0;

        let (mouse_x, mouse_y) =
            mouse_position_in_framebuffer(window, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT);

        let hovered_index = self.victory.hit_test(
            FRAMEBUFFER_WIDTH,
            FRAMEBUFFER_HEIGHT,
            mouse_x,
            mouse_y,
            has_next_level,
        );

        let left_clicked = window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        if let Some(index) = hovered_index {
            if (mouse_moved || left_clicked) && index != self.victory.selected_index() {
                self.victory.set_selected_index(index);

                if !left_clicked {
                    self.audio.play_sound(SoundEffect::MenuMove);
                }
            }
        }

        /*
         * `MenuSelect` solo suena para una acción EJECUTABLE:
         * `selected_action` ya retorna `None` para `NEXT LEVEL`
         * deshabilitado en el nivel final, así que esa fila nunca
         * llega a reproducir el sonido.
         */
        if window.is_key_pressed(KeyboardKey::KEY_ENTER)
            || (left_clicked && hovered_index.is_some())
        {
            if let Some(action) = self.victory.selected_action(has_next_level) {
                self.audio.play_sound(SoundEffect::MenuSelect);

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

                self.audio.set_music(MusicTrack::Menu);
            }
        }
    }

    /// Procesa navegación/activación por teclado de la pantalla de
    /// Derrota (Tarea 46).
    ///
    /// Mismo orden determinista que `update_victory`: navegación
    /// primero, luego ENTER; como máximo una acción se ejecuta por
    /// llamada. NO ejecuta ninguna actualización de gameplay
    /// mientras esta pantalla está activa (nunca llama
    /// `update_playing`): la sesión muerta permanece congelada/oculta
    /// detrás de ella, exactamente como la partida completada detrás
    /// de Victoria.
    fn update_defeat(&mut self, window: &RaylibHandle) {
        if window.is_key_pressed(KeyboardKey::KEY_UP) || window.is_key_pressed(KeyboardKey::KEY_W) {
            let selection_before = self.defeat.selected_item();

            self.defeat.select_previous();

            if self.defeat.selected_item() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_DOWN) || window.is_key_pressed(KeyboardKey::KEY_S)
        {
            let selection_before = self.defeat.selected_item();

            self.defeat.select_next();

            if self.defeat.selected_item() != selection_before {
                self.audio.play_sound(SoundEffect::MenuMove);
            }
        }

        /*
         * Mouse: mismo patrón exacto que `update_paused`/
         * `update_victory` — hover mueve la selección (`set_selected`,
         * la MISMA fuente de verdad que el teclado) solo cuando el
         * mouse realmente se movió este cuadro o al hacer clic; clic
         * izquierdo confirma la fila bajo el cursor.
         */
        let mouse_delta = window.get_mouse_delta();

        let mouse_moved = mouse_delta.x != 0.0 || mouse_delta.y != 0.0;

        let (mouse_x, mouse_y) =
            mouse_position_in_framebuffer(window, FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT);

        let hovered_item =
            self.defeat
                .hit_test(FRAMEBUFFER_WIDTH, FRAMEBUFFER_HEIGHT, mouse_x, mouse_y);

        let left_clicked = window.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);

        if let Some(item) = hovered_item {
            if (mouse_moved || left_clicked) && item != self.defeat.selected_item() {
                self.defeat.set_selected(item);

                if !left_clicked {
                    self.audio.play_sound(SoundEffect::MenuMove);
                }
            }
        }

        if window.is_key_pressed(KeyboardKey::KEY_ENTER) || (left_clicked && hovered_item.is_some())
        {
            let action = self.defeat.selected_item();

            self.audio.play_sound(SoundEffect::MenuSelect);

            self.perform_defeat_action(action);
        }
    }

    /// Ejecuta la acción de Derrota ya resuelta por
    /// `DefeatScreen::selected_item`.
    ///
    /// `Retry` reutiliza EXACTAMENTE el mismo lifecycle que
    /// `VictoryAction::Retry`: `LevelManager::restart` ya resuelve
    /// "el mismo nivel donde ocurrió la derrota" (fuente de verdad
    /// única — nunca la fila actualmente resaltada en Level Select,
    /// que podría haber divergido), y `replace_session_with_level`
    /// construye una `GameSession` enteramente NUEVA (vida/arma/
    /// Dealers/pickups/antorcha/hit-flash — ver
    /// `GameSession::new` — todos reinician limpios sin que esta
    /// función repare ningún campo manualmente). `MainMenu` deja la
    /// sesión muerta intacta en memoria sin actualizarla, igual que
    /// `VictoryAction::MainMenu`.
    fn perform_defeat_action(&mut self, action: DefeatMenuItem) {
        match action {
            DefeatMenuItem::Retry => match self.level_manager.restart() {
                Ok(level) => self.replace_session_with_level(level),

                Err(error) => {
                    eprintln!("Error al reiniciar el nivel: {error}");
                }
            },

            DefeatMenuItem::MainMenu => {
                self.state = GameState::Welcome;

                self.audio.set_music(MusicTrack::Menu);
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
    ///
    /// Tarea 46.5, sección 12: también es el único punto que
    /// selecciona la música del nivel, a través de
    /// `LevelManager::current_theme`/`current_is_procedural` — la
    /// fuente de verdad real de qué nivel se acaba de cargar
    /// (`load`/`next`/`restart` ya actualizaron el estado interno de
    /// `LevelManager` antes de llegar aquí) — nunca a partir de la
    /// posición resaltada en Level Select. `set_music` decide por sí
    /// mismo si eso implica reiniciar la pista desde el principio: la
    /// pista activa en Retry/Next Level siempre es `Victory`/`Defeat`
    /// (distinta de la del nivel), así que `set_music` la detiene y
    /// arranca la del nivel limpia, nunca desde la posición donde el
    /// jugador ganó/murió.
    ///
    /// Tarea 48: `The Dealer's True Maze` es el único nivel cuya
    /// música NO se deriva de `LevelTheme` (su identidad visual es
    /// aleatoria en cada generación, pero su pista es siempre la
    /// misma) — de ahí la rama explícita antes de caer al mapeo
    /// tema->pista que ya usan los tres niveles estáticos.
    fn replace_session_with_level(&mut self, level: Level) {
        let player = Player::from_level(&level, BLOCK_SIZE);

        let hand_seed = self.level_manager.current_hand_seed();

        self.session = GameSession::new(level, player, BLOCK_SIZE, hand_seed);

        self.state = GameState::Playing;

        let music_track = if self.level_manager.current_is_procedural() {
            if let Some(seed) = self.level_manager.current_procedural_seed() {
                eprintln!("Entrando a The Dealer's True Maze — Seed activa: {seed}");
            }

            MusicTrack::TheDealersTrueMaze
        } else {
            let theme = self
                .level_manager
                .current_theme()
                .expect("un nivel estático siempre resuelve un LevelTheme");

            music_track_for_theme(theme)
        };

        self.audio.set_music(music_track);
    }

    fn render(&self, framebuffer: &mut Framebuffer) {
        match self.state {
            GameState::Welcome => self.welcome.render(framebuffer),

            GameState::LevelSelect => self.level_select.render(framebuffer, &self.level_manager),

            GameState::Playing => self.render_playing(framebuffer),

            GameState::Victory => self
                .victory
                .render(framebuffer, self.level_manager.has_next()),

            /*
             * Tarea 42: el mundo congelado se dibuja EXACTAMENTE
             * igual que en `Playing` (misma `render_playing`, sin
             * duplicar ningún renderer) — la sesión no cambió, así
             * que su render tampoco debe hacerlo — y el overlay de
             * pausa se dibuja ENCIMA, en una segunda pasada.
             */
            GameState::Paused => {
                self.render_playing(framebuffer);

                self.pause.render(framebuffer);
            }

            /*
             * Tarea 46: a diferencia de `Paused`, Derrota es una
             * pantalla COMPLETA — no dibuja el mundo congelado
             * detrás. `level_manager.current_theme()` sigue siendo,
             * en este punto, el nivel donde el jugador murió (Retry/
             * Main Menu son las ÚNICAS acciones que pueden cambiarlo,
             * y ninguna se ejecuta todavía mientras se está
             * renderizando este cuadro), así que el acento cromático
             * discreto de la pantalla siempre corresponde al nivel
             * correcto, nunca a una selección obsoleta de Level
             * Select.
             */
            GameState::Defeat => self.defeat.render(
                framebuffer,
                self.level_manager
                    .current_theme()
                    .expect("Defeat solo se alcanza tras generar/cargar un nivel con tema"),
            ),
        }
    }

    fn render_playing(&self, framebuffer: &mut Framebuffer) {
        match self.session.view_mode {
            ViewMode::Map2D => {
                /*
                 * Vista superior. `display_cell_size` (Tarea 35)
                 * escala el nivel COMPLETO para que quepa dentro de
                 * la resolución lógica fija del framebuffer,
                 * independientemente de sus dimensiones reales
                 * (13×9 sigue usando 48px/celda tal como antes;
                 * House of Cards, 17×13, usa una celda más pequeña).
                 * `BLOCK_SIZE` sigue siendo la única escala de MUNDO
                 * usada por colisión/raycasting/posición del
                 * jugador.
                 */
                let display_cell_size = compute_display_cell_size(
                    framebuffer.width(),
                    framebuffer.height(),
                    self.session.level.width(),
                    self.session.level.height(),
                    BLOCK_SIZE,
                );

                render_maze(framebuffer, &self.session.level, display_cell_size);

                render_fov_rays(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    MAP_RAYS,
                    display_cell_size,
                    BLOCK_SIZE,
                );

                render_player(
                    framebuffer,
                    &self.session.player,
                    display_cell_size,
                    BLOCK_SIZE,
                );
            }

            ViewMode::World3D => {
                /*
                 * Vista en primera persona. El tema visual (cielo/
                 * suelo, paredes con textura, arma, Dealer,
                 * antorchas, meta, HUD, minimapa — Tarea 39.B)
                 * proviene del catálogo de `LevelManager`, único
                 * dueño de qué `LevelTheme` corresponde al nivel
                 * activo; no se duplica en `GameSession`. Resuelto
                 * UNA VEZ por cuadro aquí y propagado a cada
                 * renderer, en vez de que cada uno vuelva a leer
                 * `self.level_manager.current().theme` por su cuenta.
                 */
                let theme = self
                    .level_manager
                    .current_theme()
                    .expect("Playing solo se alcanza tras generar/cargar un nivel con tema");

                let wall_depth_buffer = render_world(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    BLOCK_SIZE,
                    &self.textures,
                    theme,
                );

                render_world_sprites(
                    framebuffer,
                    &self.session.level,
                    &self.session.player,
                    &self.textures,
                    BLOCK_SIZE,
                    self.session.torch_frame_index(),
                    self.session.entities(),
                    self.session.ammo_pickups(),
                    self.session.health_pickups(),
                    &wall_depth_buffer,
                    theme,
                );

                /*
                 * El arma se dibuja SIEMPRE al final, como
                 * superposición en espacio de pantalla, para que
                 * nunca quede oculta por paredes ni sprites de
                 * mundo. `weapon_reload_progress` (Tarea 43) es
                 * `None` fuera de `WeaponState::Reload`, dejando el
                 * arma en su posición base sin desplazamiento.
                 */
                render_weapon(
                    framebuffer,
                    &self.textures,
                    self.session.weapon_state(),
                    theme,
                    self.session.weapon_reload_progress(),
                );

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
                    theme,
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
                    self.session.weapon_reserve_ammo(),
                    theme,
                );
            }
        }

        /*
         * Contador de FPS: arriba-izquierda, dibujado al final para
         * quedar SIEMPRE por encima del contenido de la escena
         * (fondo/laberinto/mapa ya rellenaron el framebuffer
         * completo más arriba). Visible en ambas vistas
         * (World3D/Map2D) porque se dibuja fuera del `match`, sin
         * duplicar la llamada en cada rama. No sustituye ni se
         * solapa con el HUD (abajo-izquierda) ni el minimapa
         * (arriba-derecha, solo en World3D).
         */
        render_fps(framebuffer, self.current_fps);

        /*
         * Dealer Hands: "THE HOUSE IS RELOADING...", cuenta
         * regresiva, y el banner breve "HAND N". Visible en ambas
         * vistas (World3D/Map2D), como el contador de FPS, pero SOLO
         * mientras la sesión sigue activa detrás (Playing/Paused) —
         * nunca en Welcome/LevelSelect/Victory/Defeat, donde
         * `self.session` puede seguir conteniendo el `HandState`
         * congelado de la última partida. `App` resuelve el texto
         * exacto a partir de `HandHudMessage` (dominio puro, sin
         * vocabulario de presentación) — `rendering::hud` solo dibuja
         * la cadena ya resuelta.
         */
        if matches!(self.state, GameState::Playing | GameState::Paused) {
            if let Some(message) = hand_message_text(self.session.hand_hud_message()) {
                render_hand_message(framebuffer, &message);
            }
        }

        /*
         * Tarea 45: flash de daño al jugador, dibujado AL FINAL de
         * todo lo demás (mundo/arma/HUD/minimapa/FPS) para quedar
         * siempre por encima, igual que el contador de FPS. Se
         * dibuja en las DOS vistas (World3D/Map2D) porque también
         * está fuera del `match` de arriba. Puramente de lectura:
         * `is_hit_flash_active` no muta ningún estado de sesión.
         */
        if self.session.is_hit_flash_active() {
            render_hit_flash_overlay(framebuffer);
        }
    }
}

/// Traduce el mensaje de dominio puro del sistema de Hands
/// (`HandHudMessage`, sin vocabulario de presentación) al texto EXACTO
/// que debe mostrarse — únicamente el lenguaje de la casa (sección
/// 30): nunca "WAVE"/"ENEMIES RESPAWNING". `None` cuando no hay nada
/// que mostrar este cuadro.
fn hand_message_text(message: HandHudMessage) -> Option<String> {
    match message {
        HandHudMessage::None => None,

        HandHudMessage::HouseIsReloading => Some("THE HOUSE IS RELOADING...".to_string()),

        HandHudMessage::NextHandIn(remaining) => Some(format!("NEXT HAND IN {remaining}...")),

        HandHudMessage::HandBanner(hand_number) => {
            Some(format!("HAND {}", roman_numeral(hand_number)))
        }
    }
}

/// Numeral romano de `value` (`1` -> "I", `2` -> "II", ...),
/// puramente visual — pertenece a esta pantalla, no a `GameSession`/
/// `HandState`, mismo principio que las etiquetas romanas privadas de
/// `ui::level_select`. Notación subtractiva estándar; `0` produce una
/// cadena vacía (no debería alcanzar `HandBanner` con `hand_number ==
/// 0` en la práctica, `HandState` siempre arranca en `1`).
fn roman_numeral(mut value: usize) -> String {
    const NUMERALS: [(usize, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];

    let mut result = String::new();

    for &(magnitude, symbol) in &NUMERALS {
        while value >= magnitude {
            result.push_str(symbol);

            value -= magnitude;
        }
    }

    result
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

    /*
     * Resolución lógica FIJA (Tarea 35): independiente de las
     * dimensiones de `level`. Antes se derivaba de
     * `level.width()/height() * BLOCK_SIZE`, acoplando el tamaño de
     * ventana al nivel inicial; con niveles de tamaños distintos
     * (Crimson/Black Club en 13×9, House of Cards en 17×13) ese
     * acoplamiento ya no tiene sentido. `Map2D` (que sí necesita
     * encajar el nivel completo en esta resolución fija) resuelve su
     * propia escala de visualización por separado; el framebuffer en
     * sí nunca cambia de tamaño según el nivel activo.
     */
    let framebuffer_width = FRAMEBUFFER_WIDTH;

    let framebuffer_height = FRAMEBUFFER_HEIGHT;

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

    if let Err(error) = texture_manager.load_ammo_pickup_texture() {
        eprintln!("Error al cargar la textura del pickup de munición: {error}");
        return;
    }

    if let Err(error) = texture_manager.load_health_pickup_texture() {
        eprintln!("Error al cargar la textura del pickup de vida: {error}");
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
     * El cursor NO se captura aquí. Su ciclo de vida ahora sigue al
     * `GameState` activo (`App::sync_cursor_capture`, Tarea 38.C):
     * permanece visible/utilizable en Welcome/LevelSelect/Victory, y
     * se captura únicamente al entrar a `Playing`, liberándose de
     * nuevo al salir. El estado inicial es `Welcome`, así que el
     * cursor arranca visible, como corresponde a un menú.
     */

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

    /*
     * Tarea 38: la textura de presentación GPU persistente se crea
     * aquí, UNA sola vez, junto con el framebuffer lógico; cada
     * cuadro del bucle principal solo actualiza sus píxeles
     * (`Framebuffer::swap_buffers`), sin volver a crear/destruir
     * ningún recurso GPU.
     */
    let mut framebuffer = match Framebuffer::new(
        framebuffer_width,
        framebuffer_height,
        &mut window,
        &raylib_thread,
    ) {
        Ok(framebuffer) => framebuffer,

        Err(error) => {
            eprintln!("Error al crear el framebuffer: {error}");
            return;
        }
    };

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
        GameSession::new(level, player, BLOCK_SIZE, 0),
        texture_manager,
        welcome,
        level_select,
        victory,
        audio,
    );

    while !window.window_should_close() {
        app.update(&mut window);

        framebuffer.clear();

        app.render(&mut framebuffer);

        framebuffer.swap_buffers(&mut window, &raylib_thread);
    }
}
