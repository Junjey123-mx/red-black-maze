use crate::player::Player;
use crate::world::Level;

/// Modos de visualización disponibles.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ViewMode {
    Map2D,
    World3D,
}

/// Estado en tiempo de ejecución de la partida activa.
pub(crate) struct GameSession {
    pub(crate) level: Level,
    pub(crate) player: Player,
    pub(crate) view_mode: ViewMode,
}

impl GameSession {
    /// Crea una sesión a partir de un nivel y un jugador
    /// ya construidos.
    ///
    /// Inicia mostrando el mapa 2D.
    pub(crate) fn new(level: Level, player: Player) -> Self {
        Self {
            level,
            player,
            view_mode: ViewMode::Map2D,
        }
    }
}
