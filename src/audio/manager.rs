use std::collections::HashMap;
use std::path::Path;

use raylib::prelude::*;

use crate::world::LevelTheme;

/// Identidad tipada de una de las cuatro pistas de música de fondo
/// (Tarea 46.5). `App` selecciona la pista activa por este valor
/// (`AudioManager::set_music(MusicTrack::Menu)`), nunca por ruta de
/// archivo ni por una cadena de texto suelta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MusicTrack {
    Menu,
    CrimsonEntrance,
    BlackClub,
    HouseOfCards,

    /// Tarea 48: pista exclusiva de `The Dealer's True Maze`
    /// (nivel 4, procedural). A diferencia de las otras tres, esta
    /// pista NUNCA se deriva de un `LevelTheme` — el nivel elige su
    /// identidad visual al azar entre los tres temas existentes en
    /// cada generación, pero su música es siempre esta, sin
    /// excepción. Por eso no participa de `music_track_for_theme`:
    /// `App` la selecciona explícitamente cuando el nivel activo es
    /// el procedural, nunca a través del tema.
    TheDealersTrueMaze,
}

/// Enumeración completa de `MusicTrack`, usada para cargar el
/// catálogo completo y para las pruebas puras de cobertura del
/// catálogo. Mantener en sincronía con la definición del enum.
const ALL_MUSIC_TRACKS: [MusicTrack; 5] = [
    MusicTrack::Menu,
    MusicTrack::CrimsonEntrance,
    MusicTrack::BlackClub,
    MusicTrack::HouseOfCards,
    MusicTrack::TheDealersTrueMaze,
];

/// Única ubicación del catálogo ruta<->pista. Ningún otro módulo
/// conoce estas rutas.
fn music_path(track: MusicTrack) -> &'static str {
    match track {
        MusicTrack::Menu => "assets/audio/music/menu.mp3",
        MusicTrack::CrimsonEntrance => "assets/audio/music/crimson_entrance.mp3",
        MusicTrack::BlackClub => "assets/audio/music/black_club.mp3",
        MusicTrack::HouseOfCards => "assets/audio/music/house_of_cards.mp3",
        MusicTrack::TheDealersTrueMaze => "assets/audio/music/the_dealers_true_maze.mp3",
    }
}

/// Única ubicación de la asociación `LevelTheme -> MusicTrack`
/// (Tarea 46.5, sección 18): ningún otro módulo duplica esta
/// correspondencia. `App` la usa exclusivamente a través del
/// `LevelTheme` ya resuelto por `LevelManager` (la fuente de verdad
/// real de qué nivel está activo) — nunca infiere la pista a partir
/// de la posición seleccionada en Level Select.
///
/// Deliberadamente TOTAL sobre los tres temas visuales únicamente
/// (`CrimsonEntrance`/`BlackClub`/`HouseOfCards`): `MusicTrack::
/// TheDealersTrueMaze` no tiene tema propio y por lo tanto no
/// aparece aquí — ver su documentación en la definición del enum.
pub(crate) fn music_track_for_theme(theme: LevelTheme) -> MusicTrack {
    match theme {
        LevelTheme::CrimsonEntrance => MusicTrack::CrimsonEntrance,
        LevelTheme::BlackClub => MusicTrack::BlackClub,
        LevelTheme::HouseOfCards => MusicTrack::HouseOfCards,
    }
}

/// Identidad semántica de un efecto de sonido de partida/UI. `App`
/// solicita reproducción por este valor (`play_sound(SoundEffect::Shoot)`),
/// nunca por ruta de archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SoundEffect {
    Shoot,
    WallHit,
    EnemyIdle,
    EnemyAlert,
    EnemyHit,
    EnemyDeath,
    Footstep,
    MenuMove,
    MenuSelect,
    Victory,

    /// Tarea 43: recarga de arma ACEPTADA (`Weapon::try_start_reload`
    /// -> `true`). Nunca se dispara por una solicitud rechazada
    /// (cargador lleno, reserva agotada, ya recargando) ni por
    /// mantener `WeaponState::Reload` en cuadros posteriores — ver
    /// `App::update_playing`, el único llamador.
    Reload,

    /// Tarea 45: el jugador recibió daño REAL de al menos un Dealer
    /// en este cuadro (`GameSession::process_dealer_attacks`
    /// reportó una cantidad total > 0). Nunca se dispara si ningún
    /// Dealer atacó, si el daño aplicado terminó siendo `0` (salud
    /// ya en `0`), ni una segunda vez por múltiples Dealers
    /// impactando en el mismo cuadro — `App` lo solicita como
    /// máximo una vez por cuadro, sin importar cuántos ataques se
    /// agregaron en el total.
    PlayerHit,
}

/// Enumeración completa de `SoundEffect`, usada para cargar el
/// catálogo completo y para las pruebas puras de cobertura del
/// catálogo. Mantener en sincronía con la definición del enum.
const ALL_SOUND_EFFECTS: [SoundEffect; 12] = [
    SoundEffect::Shoot,
    SoundEffect::WallHit,
    SoundEffect::EnemyIdle,
    SoundEffect::EnemyAlert,
    SoundEffect::EnemyHit,
    SoundEffect::EnemyDeath,
    SoundEffect::Footstep,
    SoundEffect::MenuMove,
    SoundEffect::MenuSelect,
    SoundEffect::Victory,
    SoundEffect::Reload,
    SoundEffect::PlayerHit,
];

/// Única ubicación del catálogo ruta<->efecto. Ningún otro módulo
/// conoce estas rutas.
fn sfx_path(effect: SoundEffect) -> &'static str {
    match effect {
        SoundEffect::Shoot => "assets/audio/sfx/shoot.wav",
        SoundEffect::WallHit => "assets/audio/sfx/wall_hit.wav",
        SoundEffect::EnemyIdle => "assets/audio/sfx/enemy_idle.wav",
        SoundEffect::EnemyAlert => "assets/audio/sfx/enemy_alert.wav",
        SoundEffect::EnemyHit => "assets/audio/sfx/enemy_hit.wav",
        SoundEffect::EnemyDeath => "assets/audio/sfx/enemy_death.wav",
        SoundEffect::Footstep => "assets/audio/sfx/footstep.wav",
        SoundEffect::MenuMove => "assets/audio/sfx/menu_move.wav",
        SoundEffect::MenuSelect => "assets/audio/sfx/menu_select.wav",
        SoundEffect::Victory => "assets/audio/sfx/victory.wav",
        SoundEffect::Reload => "assets/audio/sfx/reload.wav",
        SoundEffect::PlayerHit => "assets/audio/sfx/player_hit.wav",
    }
}

/// Cooldown anti-spam para `EnemyIdle`: varios Dealers pueden entrar
/// a `Idle` en el mismo cuadro o alternar cerca del umbral de
/// alerta; esto colapsa la señal ambiental repetida sin descartar la
/// transición de dominio en sí (`GameSession` sigue reportándolas
/// todas).
const ENEMY_IDLE_COOLDOWN_SECONDS: f32 = 0.6;

/// Cooldown anti-spam para `EnemyAlert`, mismo motivo que
/// `ENEMY_IDLE_COOLDOWN_SECONDS`.
const ENEMY_ALERT_COOLDOWN_SECONDS: f32 = 0.35;

/// Intervalo de cadencia entre pasos consecutivos mientras el
/// jugador se desplaza realmente.
const FOOTSTEP_INTERVAL_SECONDS: f32 = 0.3;

/// Temporizador puro de cadencia de pasos: no conoce `Player`,
/// `Framebuffer` ni ningún tipo de audio; solo sabe "hubo
/// desplazamiento real este cuadro" y "cuánto tiempo pasó". Vive en
/// la capa de audio porque la CADENCIA es política de audio, no de
/// gameplay.
#[derive(Debug, Clone, Copy)]
struct FootstepCadence {
    time_since_last_step: f32,
    was_moving: bool,
}

impl FootstepCadence {
    fn new() -> Self {
        Self {
            time_since_last_step: 0.0,
            was_moving: false,
        }
    }

    /// Retorna `true` si un paso debe sonar en este cuadro.
    ///
    /// `delta_time` no finito o no positivo se ignora sin corromper
    /// el temporizador (no produce paso, no altera el estado
    /// `was_moving`/acumulado). Al detenerse el movimiento la
    /// cadencia se reinicia por completo, de modo que el siguiente
    /// desplazamiento produce un paso pronto en vez de heredar tiempo
    /// acumulado antiguo.
    fn update(&mut self, is_moving: bool, delta_time: f32) -> bool {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return false;
        }

        if !is_moving {
            self.was_moving = false;
            self.time_since_last_step = 0.0;

            return false;
        }

        if !self.was_moving {
            self.was_moving = true;
            self.time_since_last_step = 0.0;

            return true;
        }

        self.time_since_last_step += delta_time;

        if self.time_since_last_step >= FOOTSTEP_INTERVAL_SECONDS {
            self.time_since_last_step -= FOOTSTEP_INTERVAL_SECONDS;

            true
        } else {
            false
        }
    }
}

/// Gestor centralizado de la música de fondo de la aplicación.
///
/// `Music<'aud>` está atada por vida a la `RaylibAudio` que la cargó
/// (impuesto por el propio `raylib-rs`), por lo que este manager toma
/// prestada esa `RaylibAudio` en lugar de poseerla: la posesión del
/// dispositivo de audio vive en `run()`, y este struct únicamente
/// vive tan tiempo como esa referencia. No hay auto-referencia, ni
/// `unsafe`, ni `transmute`: es el modelo de propiedad más simple que
/// el API segura de `raylib-rs` permite.
///
/// Tarea 46.5: `music` pasa de una única pista (`Option<Music>`) a un
/// catálogo de hasta cuatro (`MusicTrack -> Music`), pero SOLO una
/// puede sonar a la vez — `current_track` es la única fuente de
/// verdad de cuál. Una pista sin archivo (faltante, fallo de
/// decodificación, o dispositivo de audio no disponible) simplemente
/// no tiene entrada en el mapa; todas las operaciones son no-op
/// seguras para esa pista en particular, sin afectar a las demás.
///
/// `sounds` sigue el mismo principio para los doce efectos (Tarea
/// 32 en adelante): un `SoundEffect` sin entrada en el mapa (archivo faltante o
/// fallo de carga) hace que `play_sound` sea no-op seguro para ese
/// efecto en particular, sin afectar a los demás.
pub(crate) struct AudioManager<'aud> {
    music: HashMap<MusicTrack, Music<'aud>>,

    /// Pista actualmente seleccionada (sonando o en pausa). `None`
    /// únicamente cuando ninguna pista ha sido seleccionada aún, o
    /// tras un estado terminal (Victoria/Derrota) que detiene la
    /// música por completo (Tarea 46.5, secciones 5/8) — la próxima
    /// llamada a `set_music` arranca esa pista desde el principio,
    /// nunca reanuda una posición vieja.
    current_track: Option<MusicTrack>,

    sounds: HashMap<SoundEffect, Sound<'aud>>,
    enemy_idle_cooldown: f32,
    enemy_alert_cooldown: f32,
    footstep_cadence: FootstepCadence,
}

impl<'aud> AudioManager<'aud> {
    /// Construye el manager e intenta cargar las cuatro pistas de
    /// música y los doce efectos de sonido, cada uno EXACTAMENTE una
    /// vez. Termina seleccionando `MusicTrack::Menu` como pista
    /// activa a través de `set_music` (Tarea 46.5): la reproducción
    /// del menú comienza de inmediato, sin requerir entrada de
    /// teclado, y sin duplicar la lógica de "iniciar una pista desde
    /// el principio" que `set_music` ya centraliza.
    ///
    /// `audio` es `None` cuando el dispositivo de audio no pudo
    /// inicializarse; en ese caso el manager queda completamente
    /// deshabilitado (música y SFX). Una pista ausente/fallida NO
    /// impide que las demás o los SFX se carguen: son intentos
    /// independientes sobre el mismo `RaylibAudio`.
    pub(crate) fn new(audio: Option<&'aud RaylibAudio>) -> Self {
        let music = audio.map(Self::load_music_tracks).unwrap_or_default();
        let sounds = audio.map(Self::load_sound_effects).unwrap_or_default();

        let mut manager = Self {
            music,
            current_track: None,
            sounds,
            enemy_idle_cooldown: 0.0,
            enemy_alert_cooldown: 0.0,
            footstep_cadence: FootstepCadence::new(),
        };

        manager.set_music(MusicTrack::Menu);

        manager
    }

    /// Intenta cargar cada una de las cuatro pistas de música
    /// EXACTAMENTE una vez. Una pista cuyo archivo falte o cuya
    /// carga falle se reporta con una única advertencia y se omite
    /// del mapa resultante; las demás pistas cargan con normalidad.
    /// Todas las que sí cargan quedan en loop nativo (Tarea 46.5,
    /// sección 14): ninguna pista se reinicia por temporizador o
    /// duración adivinada manualmente.
    fn load_music_tracks(audio: &'aud RaylibAudio) -> HashMap<MusicTrack, Music<'aud>> {
        let mut music = HashMap::new();

        for track in ALL_MUSIC_TRACKS {
            let path = music_path(track);

            if !Path::new(path).exists() {
                eprintln!("Pista de música '{track:?}' no encontrada en '{path}'; se omite.");

                continue;
            }

            match audio.new_music(path) {
                Ok(mut stream) => {
                    stream.set_looping(true);

                    music.insert(track, stream);
                }

                Err(error) => {
                    eprintln!("Error al cargar la pista de música '{path}': {error}");
                }
            }
        }

        music
    }

    /// Intenta cargar cada uno de los doce efectos de sonido
    /// EXACTAMENTE una vez. Un efecto cuyo archivo falte o cuya
    /// carga falle se reporta con una única advertencia y se omite
    /// del mapa resultante; los demás efectos cargan con normalidad.
    fn load_sound_effects(audio: &'aud RaylibAudio) -> HashMap<SoundEffect, Sound<'aud>> {
        let mut sounds = HashMap::new();

        for effect in ALL_SOUND_EFFECTS {
            let path = sfx_path(effect);

            if !Path::new(path).exists() {
                eprintln!("Efecto de sonido '{effect:?}' no encontrado en '{path}'; se omite.");

                continue;
            }

            match audio.new_sound(path) {
                Ok(sound) => {
                    sounds.insert(effect, sound);
                }

                Err(error) => {
                    eprintln!("Error al cargar el efecto de sonido '{path}': {error}");
                }
            }
        }

        sounds
    }

    /// Debe llamarse exactamente una vez por iteración del bucle
    /// principal, independientemente del `GameState` activo: avanza
    /// el stream de la ÚNICA pista actualmente activa (no-op seguro
    /// si no hay ninguna, o si esa pista en particular no cargó) y
    /// decrementa los cooldowns anti-spam de `EnemyIdle`/`EnemyAlert`.
    /// Es la única responsabilidad de actualización del stream de
    /// música de todo el proyecto — ningún `GameState` individual
    /// duplica esta llamada.
    ///
    /// `delta_time` no finito o no positivo se ignora para los
    /// cooldowns, sin corromper su estado.
    pub(crate) fn update(&mut self, delta_time: f32) {
        if let Some(music) = self.current_music() {
            music.update_stream();
        }

        if delta_time.is_finite() && delta_time > 0.0 {
            self.enemy_idle_cooldown = (self.enemy_idle_cooldown - delta_time).max(0.0);
            self.enemy_alert_cooldown = (self.enemy_alert_cooldown - delta_time).max(0.0);
        }
    }

    fn current_music(&self) -> Option<&Music<'aud>> {
        self.current_track.and_then(|track| self.music.get(&track))
    }

    /// Selecciona `track` como la pista activa (Tarea 46.5, sección
    /// 13): única forma de cambiar QUÉ suena. No-op si `track` ya es
    /// la pista activa — evita reiniciar el stream en llamadas
    /// redundantes (p. ej. `Welcome` <-> `Level Select`, que ambas
    /// piden `MusicTrack::Menu` sin que la transición deba reiniciar
    /// nada). En caso contrario detiene por completo la pista
    /// anterior (si había una) e inicia `track` desde el principio,
    /// de modo que nunca puedan sonar dos pistas a la vez.
    pub(crate) fn set_music(&mut self, track: MusicTrack) {
        if self.current_track == Some(track) {
            return;
        }

        if let Some(previous) = self.current_track {
            if let Some(music) = self.music.get(&previous) {
                music.stop_stream();
            }
        }

        if let Some(music) = self.music.get(&track) {
            music.play_stream();
        }

        self.current_track = Some(track);
    }

    /// Reanuda la pista actualmente activa desde donde quedó
    /// pausada (nunca recarga desde disco ni reinicia la posición).
    /// No-op seguro si no hay pista activa.
    pub(crate) fn play_music(&self) {
        if let Some(music) = self.current_music() {
            music.resume_stream();
        }
    }

    /// Pausa la pista actualmente activa preservando su posición de
    /// reproducción (nunca la detiene ni la reinicia). No-op seguro
    /// si no hay pista activa.
    pub(crate) fn pause_music(&self) {
        if let Some(music) = self.current_music() {
            music.pause_stream();
        }
    }

    /// Detiene por completo la pista actualmente activa y limpia la
    /// selección (Tarea 46.5, secciones 5/8: estados terminales de
    /// `Playing` quedan sin música). Como `current_track` queda en
    /// `None`, la siguiente llamada a `set_music` — incluso con la
    /// MISMA pista que sonaba antes de este `stop_music` (Retry) —
    /// siempre la arranca desde el principio en vez de no-operar.
    pub(crate) fn stop_music(&mut self) {
        if let Some(music) = self.current_music() {
            music.stop_stream();
        }

        self.current_track = None;
    }

    /// Solicita la reproducción de un efecto discreto
    /// (`Shoot`/`WallHit`/`EnemyHit`/`EnemyDeath`/`MenuMove`/
    /// `MenuSelect`/`Victory`/`Footstep`/`Reload`/`PlayerHit`). No-op seguro si
    /// ese efecto no cargó. `EnemyIdle`/`EnemyAlert` aplican además
    /// su propio cooldown anti-spam antes de sonar.
    pub(crate) fn play_sound(&mut self, effect: SoundEffect) {
        match effect {
            SoundEffect::EnemyIdle => {
                if self.enemy_idle_cooldown > 0.0 {
                    return;
                }

                self.enemy_idle_cooldown = ENEMY_IDLE_COOLDOWN_SECONDS;
            }

            SoundEffect::EnemyAlert => {
                if self.enemy_alert_cooldown > 0.0 {
                    return;
                }

                self.enemy_alert_cooldown = ENEMY_ALERT_COOLDOWN_SECONDS;
            }

            _ => {}
        }

        if let Some(sound) = self.sounds.get(&effect) {
            sound.play();
        }
    }

    /// Debe llamarse una vez por cuadro de `Playing` con si el
    /// jugador se desplazó realmente este cuadro (`is_moving`,
    /// decidido por `App` comparando posición antes/después de
    /// `process_events`). Reproduce `SoundEffect::Footstep` según la
    /// cadencia interna; no conoce `Player` ni ningún otro tipo de
    /// gameplay.
    pub(crate) fn update_footsteps(&mut self, is_moving: bool, delta_time: f32) {
        if self.footstep_cadence.update(is_moving, delta_time) {
            self.play_sound(SoundEffect::Footstep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // --- Catálogo de música: pruebas puras, sin `RaylibAudio`. ---

    #[test]
    fn catalog_contains_exactly_five_music_tracks() {
        assert_eq!(ALL_MUSIC_TRACKS.len(), 5);
    }

    #[test]
    fn every_music_track_maps_to_a_unique_expected_path() {
        assert_eq!(music_path(MusicTrack::Menu), "assets/audio/music/menu.mp3");
        assert_eq!(
            music_path(MusicTrack::CrimsonEntrance),
            "assets/audio/music/crimson_entrance.mp3"
        );
        assert_eq!(
            music_path(MusicTrack::BlackClub),
            "assets/audio/music/black_club.mp3"
        );
        assert_eq!(
            music_path(MusicTrack::HouseOfCards),
            "assets/audio/music/house_of_cards.mp3"
        );
        assert_eq!(
            music_path(MusicTrack::TheDealersTrueMaze),
            "assets/audio/music/the_dealers_true_maze.mp3"
        );
    }

    #[test]
    fn the_dealers_true_maze_track_is_never_reachable_through_a_level_theme() {
        // Sección 9/18: la música del nivel procedural nunca se
        // deriva del tema visual (aleatorio); solo `App` la
        // selecciona explícitamente para ese nivel.
        for theme in [
            LevelTheme::CrimsonEntrance,
            LevelTheme::BlackClub,
            LevelTheme::HouseOfCards,
        ] {
            assert_ne!(music_track_for_theme(theme), MusicTrack::TheDealersTrueMaze);
        }
    }

    #[test]
    fn music_catalog_contains_no_duplicate_path() {
        let paths: HashSet<&str> = ALL_MUSIC_TRACKS.iter().copied().map(music_path).collect();

        assert_eq!(paths.len(), ALL_MUSIC_TRACKS.len());
    }

    #[test]
    fn all_music_paths_are_under_the_music_directory() {
        for track in ALL_MUSIC_TRACKS {
            assert!(music_path(track).starts_with("assets/audio/music/"));
        }
    }

    #[test]
    fn no_music_path_equals_any_sfx_path() {
        for track in ALL_MUSIC_TRACKS {
            for effect in ALL_SOUND_EFFECTS {
                assert_ne!(music_path(track), sfx_path(effect));
            }
        }
    }

    // --- LevelTheme -> MusicTrack: única asociación del proyecto. ---

    #[test]
    fn each_level_theme_maps_to_its_own_dedicated_music_track() {
        assert_eq!(
            music_track_for_theme(LevelTheme::CrimsonEntrance),
            MusicTrack::CrimsonEntrance
        );
        assert_eq!(
            music_track_for_theme(LevelTheme::BlackClub),
            MusicTrack::BlackClub
        );
        assert_eq!(
            music_track_for_theme(LevelTheme::HouseOfCards),
            MusicTrack::HouseOfCards
        );
    }

    #[test]
    fn no_level_theme_maps_to_the_menu_track() {
        for theme in [
            LevelTheme::CrimsonEntrance,
            LevelTheme::BlackClub,
            LevelTheme::HouseOfCards,
        ] {
            assert_ne!(music_track_for_theme(theme), MusicTrack::Menu);
        }
    }

    // --- Catálogo de SFX: pruebas puras, sin `RaylibAudio`. ---

    #[test]
    fn catalog_contains_exactly_twelve_sound_effects() {
        assert_eq!(ALL_SOUND_EFFECTS.len(), 12);
    }

    #[test]
    fn every_sound_effect_maps_to_a_unique_expected_wav_path() {
        assert_eq!(sfx_path(SoundEffect::Shoot), "assets/audio/sfx/shoot.wav");
        assert_eq!(
            sfx_path(SoundEffect::WallHit),
            "assets/audio/sfx/wall_hit.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::EnemyIdle),
            "assets/audio/sfx/enemy_idle.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::EnemyAlert),
            "assets/audio/sfx/enemy_alert.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::EnemyHit),
            "assets/audio/sfx/enemy_hit.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::EnemyDeath),
            "assets/audio/sfx/enemy_death.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::Footstep),
            "assets/audio/sfx/footstep.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::MenuMove),
            "assets/audio/sfx/menu_move.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::MenuSelect),
            "assets/audio/sfx/menu_select.wav"
        );
        assert_eq!(
            sfx_path(SoundEffect::Victory),
            "assets/audio/sfx/victory.wav"
        );
        assert_eq!(sfx_path(SoundEffect::Reload), "assets/audio/sfx/reload.wav");
        assert_eq!(
            sfx_path(SoundEffect::PlayerHit),
            "assets/audio/sfx/player_hit.wav"
        );
    }

    #[test]
    fn all_sfx_paths_are_under_the_sfx_directory() {
        for effect in ALL_SOUND_EFFECTS {
            assert!(sfx_path(effect).starts_with("assets/audio/sfx/"));
        }
    }

    #[test]
    fn catalog_contains_no_duplicate_path() {
        let paths: HashSet<&str> = ALL_SOUND_EFFECTS.iter().copied().map(sfx_path).collect();

        assert_eq!(paths.len(), ALL_SOUND_EFFECTS.len());
    }

    // --- Cadencia de pasos: temporizador puro, sin `RaylibAudio`. ---

    #[test]
    fn movement_start_allows_a_prompt_first_step() {
        let mut cadence = FootstepCadence::new();

        assert!(cadence.update(true, 0.016));
    }

    #[test]
    fn continuous_movement_before_interval_does_not_retrigger() {
        let mut cadence = FootstepCadence::new();

        assert!(cadence.update(true, 0.016));
        assert!(!cadence.update(true, 0.05));
    }

    #[test]
    fn interval_threshold_triggers_next_step() {
        let mut cadence = FootstepCadence::new();

        assert!(cadence.update(true, 0.016));

        let mut triggered = false;

        let mut elapsed = 0.0;

        while elapsed < FOOTSTEP_INTERVAL_SECONDS + 0.02 {
            if cadence.update(true, 0.02) {
                triggered = true;

                break;
            }

            elapsed += 0.02;
        }

        assert!(triggered);
    }

    #[test]
    fn no_movement_resets_cadence() {
        let mut cadence = FootstepCadence::new();

        assert!(cadence.update(true, 0.016));

        assert!(!cadence.update(false, 0.016));
    }

    #[test]
    fn movement_after_reset_permits_a_prompt_step() {
        let mut cadence = FootstepCadence::new();

        assert!(cadence.update(true, 0.016));

        cadence.update(false, 0.016);

        assert!(cadence.update(true, 0.016));
    }

    #[test]
    fn invalid_delta_time_is_ignored_safely() {
        let mut cadence = FootstepCadence::new();

        assert!(!cadence.update(true, 0.0));
        assert!(!cadence.update(true, -1.0));
        assert!(!cadence.update(true, f32::NAN));
        assert!(!cadence.update(true, f32::INFINITY));

        // El temporizador sigue en su estado inicial: un cuadro con
        // dt válido todavía produce un paso pronto.
        assert!(cadence.update(true, 0.016));
    }
}
