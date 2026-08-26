use std::fmt;

use super::level::{Level, LevelError};
use super::level_generator::{self, GeneratedLevel};

/// Identidad semántica de la ambientación visual final de un nivel.
///
/// Metadatos de DOMINIO puro: no contiene ningún `raylib::Color` ni
/// ningún otro tipo de la capa de rendering. La capa de rendering
/// (`rendering::background`) es quien traduce este valor a colores
/// concretos; `World`/`LevelManager` solo identifican QUÉ tema
/// corresponde a cada nivel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LevelTheme {
    CrimsonEntrance,
    BlackClub,
    HouseOfCards,
}

/// Metadatos de un nivel del catálogo.
pub(crate) struct LevelInfo {
    pub(crate) name: &'static str,
    pub(crate) path: &'static str,
    pub(crate) theme: LevelTheme,
}

/// Catálogo explícito de los niveles disponibles.
const LEVELS: [LevelInfo; 3] = [
    LevelInfo {
        name: "Crimson Entrance",
        path: "./levels/level_01.txt",
        theme: LevelTheme::CrimsonEntrance,
    },
    LevelInfo {
        name: "Black Club",
        path: "./levels/level_02.txt",
        theme: LevelTheme::BlackClub,
    },
    LevelInfo {
        name: "House of Cards",
        path: "./levels/level_03.txt",
        theme: LevelTheme::HouseOfCards,
    },
];

/// Índice del cuarto nivel, "The Dealer's True Maze" (Tarea 48): el
/// único ÚNICO del catálogo sin `LevelInfo` estático — no tiene
/// ruta de archivo, y su `LevelTheme` se decide al azar en cada
/// generación en vez de ser un literal fijo. Vive un paso más allá
/// de `LEVELS` (`LEVELS.len() == PROCEDURAL_INDEX`), nunca dentro del
/// arreglo.
const PROCEDURAL_INDEX: usize = LEVELS.len();

/// Nombre canónico exacto pedido para el cuarto nivel; ningún otro
/// módulo posee una segunda copia de este literal.
const PROCEDURAL_LEVEL_NAME: &str = "The Dealer's True Maze";

/// Error al administrar el catálogo de niveles.
#[derive(Debug, Clone)]
pub enum LevelManagerError {
    InvalidIndex(usize),
    Load(LevelError),
}

impl fmt::Display for LevelManagerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LevelManagerError::InvalidIndex(index) => {
                write!(formatter, "Índice de nivel inválido: {index}")
            }

            LevelManagerError::Load(error) => write!(formatter, "{error}"),
        }
    }
}

/// Administra el catálogo de niveles y cuál está activo.
///
/// Tarea 48: el cuarto nivel ("The Dealer's True Maze") NO vive en
/// `levels` — no tiene `LevelInfo` estático porque no tiene ruta de
/// archivo ni tema fijo. `procedural` cachea su ÚNICA generación
/// vigente (semilla, cuadrícula, tema, conteos de entidades): existe
/// precisamente para que `restart()` (Retry) pueda reconstruir
/// EXACTAMENTE el mismo nivel sin generar de nuevo, mientras que
/// `load(PROCEDURAL_INDEX)`/`next()` SIEMPRE generan una partida
/// fresca y sobrescriben este caché — la regla "Retry = same bet,
/// New Game = new bet" de la sección 7 vive exactamente en esta
/// distinción entre los dos métodos.
pub struct LevelManager {
    levels: &'static [LevelInfo],
    current: usize,
    procedural: Option<GeneratedLevel>,
}

impl LevelManager {
    /// Crea el administrador con el catálogo explícito de niveles.
    pub fn new() -> Self {
        Self {
            levels: &LEVELS,
            current: 0,
            procedural: None,
        }
    }

    /// Cantidad de niveles del catálogo, incluyendo el cuarto nivel
    /// procedural.
    ///
    /// Lectura pura; no expone ruta de archivo alguna. Pensado para
    /// que la UI (Selección de Nivel) sepa cuántas opciones mostrar
    /// sin conocer el catálogo interno.
    pub(crate) fn level_count(&self) -> usize {
        self.levels.len() + 1
    }

    /// Nombre canónico del nivel en `index`, o `None` si el índice
    /// está fuera de rango.
    ///
    /// Retorna únicamente el nombre; nunca la ruta de archivo. No
    /// entra en pánico ante un índice inválido.
    pub(crate) fn level_name(&self, index: usize) -> Option<&'static str> {
        if index == PROCEDURAL_INDEX {
            return Some(PROCEDURAL_LEVEL_NAME);
        }

        self.levels.get(index).map(|info| info.name)
    }

    /// Identidad temática (`LevelTheme`) del nivel en `index`, o
    /// `None` si el índice está fuera de rango.
    ///
    /// Tarea 41: acceso mínimo de solo lectura para que la UI
    /// (Selección de Nivel) pueda resolver el acento cromático de
    /// CADA fila del catálogo — no solo del nivel actualmente
    /// cargado (`current`) — sin exponer `LevelInfo`/rutas de
    /// archivo. Sigue el mismo patrón que `level_name`: no entra en
    /// pánico ante un índice inválido.
    ///
    /// Tarea 48: para `PROCEDURAL_INDEX` el tema no es un literal
    /// fijo — se decide al azar en cada generación — así que esta
    /// función retorna `None` hasta que el nivel se haya generado al
    /// menos una vez en esta sesión (`self.procedural` sigue vacío).
    /// La UI (`level_select`) ya degrada este `None` de forma segura
    /// a un acento neutro, el mismo camino que ya usaba para
    /// cualquier índice sin tema conocido.
    pub(crate) fn level_theme(&self, index: usize) -> Option<LevelTheme> {
        if index == PROCEDURAL_INDEX {
            return self.procedural.as_ref().map(|generated| generated.theme);
        }

        self.levels.get(index).map(|info| info.theme)
    }

    /// `true` si el nivel actualmente activo es el cuarto nivel
    /// procedural. `App` la usa para decidir la música (siempre
    /// exclusiva, nunca derivada del tema) sin que `world` necesite
    /// conocer nada de `audio::MusicTrack`.
    pub(crate) fn current_is_procedural(&self) -> bool {
        self.current == PROCEDURAL_INDEX
    }

    /// Tema resuelto del nivel procedural EN LA GENERACIÓN VIGENTE,
    /// o `None` si aún no se ha generado ninguna en esta sesión.
    /// Útil para que `App` seleccione paleta/texturas sin tener que
    /// volver a consultar `level_theme(PROCEDURAL_INDEX)` con su
    /// semántica de índice genérica.
    pub(crate) fn current_theme(&self) -> Option<LevelTheme> {
        if self.current_is_procedural() {
            self.procedural.as_ref().map(|generated| generated.theme)
        } else {
            self.levels.get(self.current).map(|info| info.theme)
        }
    }

    /// Semilla de la generación procedural vigente, o `None` si el
    /// nivel activo no es el procedural o aún no se generó. Expuesta
    /// para debugging/reproducibilidad (sección 6) y para las
    /// pruebas de Retry/New Game.
    pub(crate) fn current_procedural_seed(&self) -> Option<u64> {
        self.procedural.as_ref().map(|generated| generated.seed)
    }

    /// Semilla determinista para el sistema de Hands (Dealer Hands,
    /// sección 17) del nivel ACTUALMENTE activo.
    ///
    /// Para "The Dealer's True Maze" es la semilla real de la
    /// generación vigente (`0` como último recurso defensivo si por
    /// algún motivo aún no se generó nada — nunca debería ocurrir en
    /// la práctica, `App` siempre genera antes de construir la
    /// sesión). Para los tres niveles estáticos no existe una "semilla
    /// de nivel" real — se deriva un valor FIJO y determinista por
    /// índice de catálogo (nunca aleatorio), suficiente para que las
    /// Hands adicionales de esos niveles también sean reproducibles
    /// en pruebas sin necesitar el concepto de semilla de generación
    /// que solo tiene sentido para el nivel procedural.
    pub(crate) fn current_hand_seed(&self) -> u64 {
        if self.current_is_procedural() {
            self.current_procedural_seed().unwrap_or(0)
        } else {
            // Constante arbitraria pero FIJA por índice: determinista,
            // nunca dependiente del reloj ni de ninguna otra fuente
            // no reproducible.
            0xD00D_0000_0000_0000_u64 ^ (self.current as u64)
        }
    }

    /// Tope de Dealers vivos simultáneos del nivel ACTUALMENTE activo
    /// (sección 11 de "Dealer Hands"): decidido según tamaño real del
    /// mapa y cantidad inicial de cada uno de los cuatro niveles,
    /// nunca superior a `hand::GLOBAL_HARD_DEALER_CAP` (52).
    ///
    /// Valores elegidos (documentados también en la auditoría de la
    /// tarea):
    /// - Crimson Entrance (53 celdas transitables, 2 Dealers
    ///   iniciales): cap 16 -> progresión 2, 4, 8, 16(cap).
    /// - Black Club (47 celdas transitables, 3 iniciales): cap 12 ->
    ///   progresión 3, 6, 12(cap).
    /// - House of Cards (111 celdas transitables, 4 iniciales): cap
    ///   32 -> progresión 4, 8, 16, 32(cap).
    /// - The Dealer's True Maze (~222 celdas transitables, ~23
    ///   iniciales, varía por seed): cap 50 -> progresión ~23, ~46,
    ///   50(cap) — el cap más alto del catálogo, pedido
    ///   explícitamente, y seguro bajo el límite global de 52.
    pub(crate) fn current_dealer_cap(&self) -> usize {
        const CRIMSON_ENTRANCE_DEALER_CAP: usize = 16;
        const BLACK_CLUB_DEALER_CAP: usize = 12;
        const HOUSE_OF_CARDS_DEALER_CAP: usize = 32;
        const PROCEDURAL_DEALER_CAP: usize = 50;

        if self.current_is_procedural() {
            return PROCEDURAL_DEALER_CAP;
        }

        match self.current {
            0 => CRIMSON_ENTRANCE_DEALER_CAP,
            1 => BLACK_CLUB_DEALER_CAP,
            2 => HOUSE_OF_CARDS_DEALER_CAP,
            _ => PROCEDURAL_DEALER_CAP,
        }
    }

    /// Carga explícitamente el nivel indicado por índice.
    ///
    /// `current` solo se actualiza después de una carga exitosa.
    ///
    /// Tarea 48: `index == PROCEDURAL_INDEX` SIEMPRE genera una
    /// partida NUEVA (`level_generator::fresh_seed`) y sobrescribe
    /// `self.procedural` — este es el camino de "New Game" para el
    /// cuarto nivel, alcanzado tanto por selección directa en Level
    /// Select como por `next()` al completar House of Cards. Nunca
    /// escribe ni lee un `.txt`: la generación ocurre íntegramente
    /// en memoria.
    pub fn load(&mut self, index: usize) -> Result<Level, LevelManagerError> {
        if index == PROCEDURAL_INDEX {
            let generated = level_generator::generate(level_generator::fresh_seed());

            let level =
                Level::from_cells(generated.cells.clone()).map_err(LevelManagerError::Load)?;

            self.procedural = Some(generated);
            self.current = index;

            return Ok(level);
        }

        let info = self
            .levels
            .get(index)
            .ok_or(LevelManagerError::InvalidIndex(index))?;

        let level = Level::load(info.path).map_err(LevelManagerError::Load)?;

        self.current = index;

        Ok(level)
    }

    /// Vuelve a cargar el nivel actual.
    ///
    /// Tarea 48, sección 7 (regla crítica): si el nivel actual es el
    /// procedural, esto NUNCA regenera — reconstruye un `Level`
    /// fresco a partir de la MISMA cuadrícula ya cacheada en
    /// `self.procedural` (mismo seed, mismo layout, mismos Dealers,
    /// mismos pickups, mismo tema). Para los tres niveles estáticos
    /// el comportamiento es exactamente el de antes: recargar el
    /// mismo `.txt` desde disco.
    pub fn restart(&mut self) -> Result<Level, LevelManagerError> {
        if self.current == PROCEDURAL_INDEX {
            let generated = self
                .procedural
                .as_ref()
                .expect("Retry en el nivel procedural requiere una generación previa");

            eprintln!(
                "The Dealer's True Maze — Retry: reutilizando la semilla {} (mismo laberinto, sin regenerar).",
                generated.seed
            );

            return Level::from_cells(generated.cells.clone()).map_err(LevelManagerError::Load);
        }

        self.load(self.current)
    }

    /// Indica si existe un nivel siguiente en el catálogo después
    /// del actual.
    ///
    /// Consulta pura de solo lectura: no muta `current`, no carga
    /// ningún archivo y no expone ninguna ruta. Pensada para que la
    /// UI (Victoria) sepa, ANTES de activar `NEXT LEVEL`, si esa
    /// acción está disponible.
    pub fn has_next(&self) -> bool {
        self.current + 1 < self.level_count()
    }

    /// Carga el siguiente nivel del catálogo, si existe.
    ///
    /// Retorna `Ok(None)` sin cambiar `current` cuando el nivel
    /// actual ya es el último del catálogo. Cuando el siguiente es
    /// el procedural (al completar House of Cards), delega en
    /// `load`, que ya genera una partida nueva — la victoria de
    /// House of Cards deja de ser el final del juego y pasa a
    /// conducir a "The Dealer's True Maze" (sección 2).
    pub fn next(&mut self) -> Result<Option<Level>, LevelManagerError> {
        let next_index = self.current + 1;

        if next_index >= self.level_count() {
            return Ok(None);
        }

        let level = self.load(next_index)?;

        Ok(Some(level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_count_includes_the_procedural_fourth_level() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_count(), 4);
    }

    #[test]
    fn level_name_returns_the_canonical_catalog_names_in_order() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_name(0), Some("Crimson Entrance"));
        assert_eq!(manager.level_name(1), Some("Black Club"));
        assert_eq!(manager.level_name(2), Some("House of Cards"));
        assert_eq!(manager.level_name(3), Some("The Dealer's True Maze"));
    }

    #[test]
    fn level_name_out_of_range_is_none() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_name(4), None);
    }

    #[test]
    fn has_next_is_true_on_the_fresh_first_level() {
        let manager = LevelManager::new();

        assert!(manager.has_next());
    }

    #[test]
    fn has_next_is_true_after_loading_the_second_level() {
        let mut manager = LevelManager::new();

        manager.load(1).expect("el índice 1 debe cargar");

        assert!(manager.has_next());
    }

    #[test]
    fn has_next_is_true_after_house_of_cards_because_the_fourth_level_follows() {
        let mut manager = LevelManager::new();

        manager
            .load(2)
            .expect("el índice 2 (House of Cards) debe cargar");

        assert!(manager.has_next());
    }

    #[test]
    fn has_next_is_false_on_the_procedural_fourth_level() {
        let mut manager = LevelManager::new();

        manager
            .load(3)
            .expect("el índice 3 (procedural) debe generar y cargar");

        assert!(!manager.has_next());
    }

    #[test]
    fn has_next_does_not_mutate_current() {
        let mut manager = LevelManager::new();

        manager.load(1).expect("el índice 1 debe cargar");

        let _ = manager.has_next();
        let _ = manager.has_next();

        assert_eq!(manager.level_name(1), Some("Black Club"));
    }

    #[test]
    fn catalog_maps_each_static_index_to_its_expected_theme() {
        let manager = LevelManager::new();

        assert_eq!(manager.levels[0].theme, LevelTheme::CrimsonEntrance);
        assert_eq!(manager.levels[1].theme, LevelTheme::BlackClub);
        assert_eq!(manager.levels[2].theme, LevelTheme::HouseOfCards);
    }

    #[test]
    fn level_theme_returns_the_expected_theme_for_each_static_catalog_index() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_theme(0), Some(LevelTheme::CrimsonEntrance));
        assert_eq!(manager.level_theme(1), Some(LevelTheme::BlackClub));
        assert_eq!(manager.level_theme(2), Some(LevelTheme::HouseOfCards));
    }

    #[test]
    fn level_theme_out_of_range_is_none() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_theme(4), None);
        assert_eq!(manager.level_theme(usize::MAX), None);
    }

    #[test]
    fn procedural_theme_is_unknown_until_generated_then_resolves_to_one_of_the_three() {
        let mut manager = LevelManager::new();

        assert_eq!(manager.level_theme(3), None);

        manager.load(3).expect("el nivel procedural debe generar");

        let theme = manager
            .level_theme(3)
            .expect("ya generado, debe tener tema");

        assert!(matches!(
            theme,
            LevelTheme::CrimsonEntrance | LevelTheme::BlackClub | LevelTheme::HouseOfCards
        ));
    }

    #[test]
    fn every_catalog_index_loads_a_valid_level() {
        let mut manager = LevelManager::new();

        for index in 0..manager.level_count() {
            let level = manager
                .load(index)
                .expect("cada índice del catálogo debe cargar");

            assert!(level.width() > 0);
            assert!(level.height() > 0);
        }
    }

    // --- Tarea 48: nivel procedural ("The Dealer's True Maze") ---

    #[test]
    fn current_is_procedural_only_on_the_fourth_level() {
        let mut manager = LevelManager::new();

        assert!(!manager.current_is_procedural());

        manager.load(0).expect("índice 0 debe cargar");
        assert!(!manager.current_is_procedural());

        manager.load(3).expect("índice 3 debe generar");
        assert!(manager.current_is_procedural());
    }

    #[test]
    fn retry_on_the_procedural_level_keeps_the_same_seed_and_layout() {
        let mut manager = LevelManager::new();

        let first = manager.load(3).expect("primera generación");

        let seed_after_first_load = manager
            .current_procedural_seed()
            .expect("debe haber una semilla tras generar");

        let retried = manager.restart().expect("retry no debe fallar");

        let seed_after_retry = manager
            .current_procedural_seed()
            .expect("la semilla debe seguir presente tras retry");

        assert_eq!(seed_after_first_load, seed_after_retry);
        assert_eq!(first.player_spawn(), retried.player_spawn());
        assert_eq!(first.goal(), retried.goal());
        assert_eq!(first.enemy_spawns(), retried.enemy_spawns());
        assert_eq!(first.ammo_spawns(), retried.ammo_spawns());
        assert_eq!(manager.level_theme(3), manager.level_theme(3));
    }

    #[test]
    fn selecting_the_procedural_level_again_generates_a_new_seed() {
        let mut manager = LevelManager::new();

        manager.load(3).expect("primera generación");

        let first_seed = manager
            .current_procedural_seed()
            .expect("debe haber semilla");

        manager
            .load(3)
            .expect("segunda selección directa (New Game)");

        let second_seed = manager
            .current_procedural_seed()
            .expect("debe haber semilla nueva");

        // `fresh_seed` está basada en reloj+PID: en la práctica
        // virtualmente nunca coincide entre dos llamadas reales.
        assert_ne!(first_seed, second_seed);
    }

    #[test]
    fn reaching_the_procedural_level_through_next_also_generates_a_fresh_seed() {
        let mut manager = LevelManager::new();

        manager.load(2).expect("House of Cards debe cargar");

        let level = manager
            .next()
            .expect("next no debe fallar")
            .expect("debe existir un nivel siguiente: el procedural");

        assert!(manager.current_is_procedural());
        assert!(manager.current_procedural_seed().is_some());
        assert!(level.width() > 0);
        assert!(level.height() > 0);

        // Ya no hay nivel siguiente: el procedural es el último.
        assert!(!manager.has_next());
    }

    // --- Dealer Hands: caps por nivel y semilla determinista ---

    /// Mismo valor que `game::hand::GLOBAL_HARD_DEALER_CAP` — no se
    /// importa desde aquí para no introducir una dependencia
    /// `world -> game` (dirección incorrecta); en su lugar,
    /// `game::hand` documenta el mismo número y esta prueba lo repite
    /// como literal.
    const GLOBAL_HARD_DEALER_CAP: usize = 52;

    #[test]
    fn no_level_dealer_cap_exceeds_the_global_hard_cap() {
        let mut manager = LevelManager::new();

        for index in 0..manager.level_count() {
            manager.load(index).expect("cada índice debe cargar");

            assert!(manager.current_dealer_cap() <= GLOBAL_HARD_DEALER_CAP);
        }
    }

    #[test]
    fn each_static_level_has_its_documented_dealer_cap() {
        let mut manager = LevelManager::new();

        manager.load(0).expect("Crimson Entrance debe cargar");
        assert_eq!(manager.current_dealer_cap(), 16);

        manager.load(1).expect("Black Club debe cargar");
        assert_eq!(manager.current_dealer_cap(), 12);

        manager.load(2).expect("House of Cards debe cargar");
        assert_eq!(manager.current_dealer_cap(), 32);
    }

    #[test]
    fn the_procedural_level_has_the_highest_cap_of_the_catalog() {
        let mut manager = LevelManager::new();

        manager.load(3).expect("el nivel procedural debe generar");

        assert_eq!(manager.current_dealer_cap(), 50);
    }

    #[test]
    fn hand_seed_is_deterministic_and_static_levels_get_distinct_fixed_seeds() {
        let mut manager = LevelManager::new();

        manager.load(0).expect("índice 0 debe cargar");
        let seed_a_first = manager.current_hand_seed();

        manager.load(1).expect("índice 1 debe cargar");
        let seed_b = manager.current_hand_seed();

        manager.load(0).expect("índice 0 debe cargar de nuevo");
        let seed_a_second = manager.current_hand_seed();

        assert_eq!(seed_a_first, seed_a_second);
        assert_ne!(seed_a_first, seed_b);
    }

    #[test]
    fn procedural_hand_seed_matches_the_generated_level_seed() {
        let mut manager = LevelManager::new();

        manager.load(3).expect("el nivel procedural debe generar");

        assert_eq!(
            manager.current_hand_seed(),
            manager.current_procedural_seed().unwrap()
        );
    }
}
