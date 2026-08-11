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
