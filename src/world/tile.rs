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
            _ => None,
        }
    }

    /// Indica si esta clasificación semántica puede ser
    /// atravesada por el jugador y por los rayos.
    pub(crate) fn is_walkable(self) -> bool {
        matches!(self, Tile::Empty | Tile::PlayerSpawn | Tile::Goal)
    }
}
