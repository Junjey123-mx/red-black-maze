use raylib::prelude::Vector2;

/// Resultado del cálculo de un único rayo.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RayHit {
    /// Distancia desde el jugador hasta el punto exacto de impacto.
    pub(crate) distance: f32,

    /// Carácter de la pared golpeada.
    pub(crate) tile: char,

    /// Posición exacta del impacto en el borde de la celda.
    pub(crate) position: Vector2,

    /// Coordenada horizontal normalizada a lo largo de la cara
    /// golpeada, en el rango `[0.0, 1.0)`.
    pub(crate) texture_offset: f32,
}
