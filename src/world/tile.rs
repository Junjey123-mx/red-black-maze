/// Clasificación semántica de una celda del mundo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tile {
    Empty,
    PlayerSpawn,
    Goal,
    HeartWall,
    DiamondWall,
    ClubWall,
    SpadeWall,
    EnemySpawn,
    Torch,
    AmmoSpawn,
}

impl Tile {
    /// Clasifica el carácter crudo del mapa como un `Tile` conocido.
    ///
    /// Retorna `None` para cualquier carácter no reconocido.
    pub(crate) fn from_char(cell: char) -> Option<Self> {
        match cell {
            ' ' => Some(Tile::Empty),
            'p' => Some(Tile::PlayerSpawn),
            'g' => Some(Tile::Goal),
            '+' => Some(Tile::HeartWall),
            '-' => Some(Tile::DiamondWall),
            '|' => Some(Tile::ClubWall),
            '#' => Some(Tile::SpadeWall),
            'e' => Some(Tile::EnemySpawn),
            't' => Some(Tile::Torch),
            'a' => Some(Tile::AmmoSpawn),
            _ => None,
        }
    }

    /// Indica si esta clasificación semántica puede ser
    /// atravesada por el jugador y por los rayos.
    ///
    /// `Torch` es transitable: es únicamente un marcador visual de
    /// aparición de sprite, no una pared. `EnemySpawn` también es
    /// transitable desde Tarea 23: es solo un marcador de aparición
    /// de un `Entity` real, no una pared. Si permaneciera bloqueado,
    /// un rayo terminaría exactamente sobre la posición del Dealer
    /// y su propio billboard quedaría auto-ocluido detrás de esa
    /// "pared". `AmmoSpawn` (Tarea 44) sigue exactamente el mismo
    /// principio: es solo un marcador de aparición de un
    /// `AmmoPickup` runtime, nunca una pared.
    pub(crate) fn is_walkable(self) -> bool {
        matches!(
            self,
            Tile::Empty
                | Tile::PlayerSpawn
                | Tile::Goal
                | Tile::Torch
                | Tile::EnemySpawn
                | Tile::AmmoSpawn
        )
    }
}
