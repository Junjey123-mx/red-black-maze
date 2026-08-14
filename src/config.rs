pub const BLOCK_SIZE: usize = 48;

/// Resolución lógica FIJA del framebuffer, independiente de las
/// dimensiones de cualquier nivel.
///
/// Antes de Tarea 35 estos valores se derivaban implícitamente de
/// `level.width()/height() * BLOCK_SIZE` del nivel inicial (Crimson
/// Entrance, 13×9), lo que acoplaba accidentalmente el tamaño de
/// ventana al tamaño de UN nivel concreto. Con House of Cards ahora
/// en 17×13, ese acoplamiento dejó de ser válido en general, así que
/// la resolución lógica se fija aquí de forma explícita, preservando
/// EXACTAMENTE el valor histórico (13×9×48 = 624×432) sin adoptar la
/// resolución mayor de ejemplo del Plan Maestro.
pub const FRAMEBUFFER_WIDTH: i32 = 624;
pub const FRAMEBUFFER_HEIGHT: i32 = 432;

/// Cantidad de rayos utilizados únicamente para
/// mostrar el abanico en el mapa 2D.
pub const MAP_RAYS: usize = 180;

pub const TARGET_FPS: u32 = 60;

/// Radianes de giro de cámara por unidad de desplazamiento
/// horizontal del mouse.
pub const MOUSE_SENSITIVITY: f32 = 0.0025;
