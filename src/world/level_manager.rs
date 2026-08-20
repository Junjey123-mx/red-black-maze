use std::fmt;

use super::level::{Level, LevelError};

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
pub struct LevelManager {
    levels: &'static [LevelInfo],
    current: usize,
}

impl LevelManager {
    /// Crea el administrador con el catálogo explícito de niveles.
    pub fn new() -> Self {
        Self {
            levels: &LEVELS,
            current: 0,
        }
    }

    /// Metadatos del nivel actualmente seleccionado.
    pub(crate) fn current(&self) -> &LevelInfo {
        &self.levels[self.current]
    }

    /// Cantidad de niveles del catálogo.
    ///
    /// Lectura pura de `self.levels`; no expone ruta de archivo
    /// alguna. Pensado para que la UI (Selección de Nivel) sepa
    /// cuántas opciones mostrar sin conocer el catálogo interno.
    pub(crate) fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Nombre canónico del nivel en `index`, o `None` si el índice
    /// está fuera de rango.
    ///
    /// Retorna únicamente el nombre; nunca la ruta de archivo. No
    /// entra en pánico ante un índice inválido.
    pub(crate) fn level_name(&self, index: usize) -> Option<&'static str> {
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
    pub(crate) fn level_theme(&self, index: usize) -> Option<LevelTheme> {
        self.levels.get(index).map(|info| info.theme)
    }

    /// Carga explícitamente el nivel indicado por índice.
    ///
    /// `current` solo se actualiza después de una carga exitosa.
    pub fn load(&mut self, index: usize) -> Result<Level, LevelManagerError> {
        let info = self
            .levels
            .get(index)
            .ok_or(LevelManagerError::InvalidIndex(index))?;

        let level = Level::load(info.path).map_err(LevelManagerError::Load)?;

        self.current = index;

        Ok(level)
    }

    /// Vuelve a cargar el nivel actual desde disco.
    pub fn restart(&mut self) -> Result<Level, LevelManagerError> {
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
        self.current + 1 < self.levels.len()
    }

    /// Carga el siguiente nivel del catálogo, si existe.
    ///
    /// Retorna `Ok(None)` sin cambiar `current` cuando el nivel
    /// actual ya es el último del catálogo.
    pub fn next(&mut self) -> Result<Option<Level>, LevelManagerError> {
        let next_index = self.current + 1;

        if next_index >= self.levels.len() {
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
    fn level_count_matches_the_catalog() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_count(), 3);
    }

    #[test]
    fn level_name_returns_the_canonical_catalog_names_in_order() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_name(0), Some("Crimson Entrance"));
        assert_eq!(manager.level_name(1), Some("Black Club"));
        assert_eq!(manager.level_name(2), Some("House of Cards"));
    }

    #[test]
    fn level_name_out_of_range_is_none() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_name(3), None);
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
    fn has_next_is_false_on_the_last_level() {
        let mut manager = LevelManager::new();

        manager.load(2).expect("el índice 2 debe cargar");

        assert!(!manager.has_next());
    }

    #[test]
    fn has_next_does_not_mutate_current() {
        let mut manager = LevelManager::new();

        manager.load(1).expect("el índice 1 debe cargar");

        let _ = manager.has_next();
        let _ = manager.has_next();

        assert_eq!(manager.current().name, "Black Club");
    }

    #[test]
    fn catalog_maps_each_index_to_its_expected_theme() {
        let manager = LevelManager::new();

        assert_eq!(manager.levels[0].theme, LevelTheme::CrimsonEntrance);
        assert_eq!(manager.levels[1].theme, LevelTheme::BlackClub);
        assert_eq!(manager.levels[2].theme, LevelTheme::HouseOfCards);
    }

    #[test]
    fn level_theme_returns_the_expected_theme_for_each_catalog_index() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_theme(0), Some(LevelTheme::CrimsonEntrance));
        assert_eq!(manager.level_theme(1), Some(LevelTheme::BlackClub));
        assert_eq!(manager.level_theme(2), Some(LevelTheme::HouseOfCards));
    }

    #[test]
    fn level_theme_out_of_range_is_none() {
        let manager = LevelManager::new();

        assert_eq!(manager.level_theme(3), None);
        assert_eq!(manager.level_theme(usize::MAX), None);
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
}
