/// Resultado del cálculo de un único rayo.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RayHit {
    /// Distancia desde el jugador hasta la pared.
    pub(crate) distance: f32,

    /// Carácter de la pared golpeada.
    pub(crate) tile: char,
}
