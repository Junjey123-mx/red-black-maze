use std::fmt;

use super::level::{Level, LevelError};

/// Metadatos de un nivel del catálogo.
pub(crate) struct LevelInfo {
    pub(crate) name: &'static str,
    pub(crate) path: &'static str,
}

/// Catálogo explícito de los niveles disponibles.
const LEVELS: [LevelInfo; 3] = [
    LevelInfo {
        name: "Crimson Entrance",
        path: "./levels/level_01.txt",
    },
    LevelInfo {
        name: "Black Club",
        path: "./levels/level_02.txt",
    },
    LevelInfo {
        name: "House of Cards",
        path: "./levels/level_03.txt",
    },
];

/// Error al administrar el catálogo de niveles.
#[derive(Debug, Clone)]
pub(crate) enum LevelManagerError {
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
pub(crate) struct LevelManager {
    levels: &'static [LevelInfo],
    current: usize,
}

impl LevelManager {
    /// Crea el administrador con el catálogo explícito de niveles.
    pub(crate) fn new() -> Self {
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

    /// Carga explícitamente el nivel indicado por índice.
    ///
    /// `current` solo se actualiza después de una carga exitosa.
    pub(crate) fn load(&mut self, index: usize) -> Result<Level, LevelManagerError> {
        let info = self
            .levels
            .get(index)
            .ok_or(LevelManagerError::InvalidIndex(index))?;

        let level = Level::load(info.path).map_err(LevelManagerError::Load)?;

        self.current = index;

        Ok(level)
    }

    /// Vuelve a cargar el nivel actual desde disco.
    pub(crate) fn restart(&mut self) -> Result<Level, LevelManagerError> {
        self.load(self.current)
    }

    /// Carga el siguiente nivel del catálogo, si existe.
    ///
    /// Retorna `Ok(None)` sin cambiar `current` cuando el nivel
    /// actual ya es el último del catálogo.
    pub(crate) fn next(&mut self) -> Result<Option<Level>, LevelManagerError> {
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
