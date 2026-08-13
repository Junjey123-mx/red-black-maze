use raylib::prelude::Color;

use crate::rendering::framebuffer::Framebuffer;
use crate::ui::{GameOfLife, GameOfLifeRenderConfig};

/// Tamaño, en píxeles de framebuffer, de cada celda del Juego de la
/// Vida de fondo. Deliberadamente ajeno a `BLOCK_SIZE`: es una
/// unidad puramente de presentación de UI.
const CELL_SIZE: i32 = 8;

/// Separación, en píxeles, entre celdas contiguas del fondo.
const CELL_GAP: i32 = 1;

/// Intervalo de paso del Juego de la Vida de Bienvenida. Este valor
/// es propio de esta pantalla; el motor de Tarea 27 no impone
/// ninguna velocidad de pantalla específica.
const STEP_INTERVAL: f32 = 0.12;

const BACKGROUND_COLOR: Color = Color::new(5, 5, 8, 255);
const GAME_OF_LIFE_COLOR: Color = Color::new(195, 28, 38, 255);
const TITLE_COLOR: Color = Color::new(224, 218, 205, 255);
const TITLE_SHADOW_COLOR: Color = Color::new(120, 18, 24, 255);
const BUTTON_PANEL_COLOR: Color = Color::new(14, 12, 14, 255);
const BUTTON_BORDER_COLOR: Color = Color::new(150, 24, 32, 255);
const BUTTON_TEXT_COLOR: Color = Color::new(224, 218, 205, 255);

const TITLE_TEXT: &str = "RED-BLACK MAZE";
const TITLE_SCALE: i32 = 4;
const TITLE_TOP_MARGIN: i32 = 40;

const PLAY_TEXT: &str = "PLAY";
const PLAY_SCALE: i32 = 3;
const PLAY_BOTTOM_MARGIN: i32 = 48;

const BUTTON_PADDING_X: i32 = 16;
const BUTTON_PADDING_Y: i32 = 10;
const BUTTON_BORDER_THICKNESS: i32 = 2;

/// Ancho/alto, en píxeles lógicos (sin escalar), de un glifo.
const GLYPH_WIDTH: i32 = 5;
const GLYPH_HEIGHT: i32 = 7;

/// Separación horizontal, en píxeles lógicos, entre glifos.
const GLYPH_GAP: i32 = 1;

/// Fuente bitmap 5x7 mínima: solo los glifos que
/// `RED-BLACK MAZE`/`PLAY` requieren realmente. No es un motor de
/// texto genérico ni implementa el alfabeto completo.
fn glyph_rows(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'B' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'Z' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ],
        '-' => [
            0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000,
        ],

        // El espacio, y cualquier carácter no soportado por esta
        // fuente mínima, se dibujan en blanco en vez de entrar en
        // pánico o intentar una fuente completa.
        _ => [0; 7],
    }
}

/// Ancho total, en píxeles ya escalados, que ocupará `text` al
/// dibujarse con `draw_text` en la escala indicada.
///
/// Función pura de geometría: no depende de `Framebuffer`.
fn text_width(text: &str, scale: i32) -> i32 {
    let glyph_count = text.chars().count() as i32;

    if glyph_count == 0 {
        return 0;
    }

    glyph_count * (GLYPH_WIDTH + GLYPH_GAP) * scale - GLYPH_GAP * scale
}

/// Coordenada X que centra horizontalmente un contenido de
/// `content_width` píxeles dentro de un framebuffer de
/// `framebuffer_width` píxeles.
fn centered_x(framebuffer_width: i32, content_width: i32) -> i32 {
    (framebuffer_width - content_width) / 2
}

/// Rellena `[x0, x1) x [y0, y1)` dentro del framebuffer.
///
/// Se apoya enteramente en `Framebuffer::point`, que ya recorta
/// coordenadas fuera de rango, por lo que ninguna coordenada,
/// incluso negativa, puede producir una escritura fuera de límites.
fn fill_rect(framebuffer: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
    framebuffer.set_current_color(color);

    for y in y0..y1 {
        for x in x0..x1 {
            framebuffer.point(x, y);
        }
    }
}

/// Dibuja un único glifo escalado, anclado en `(origin_x,
/// origin_y)`.
fn draw_glyph(
    framebuffer: &mut Framebuffer,
    rows: &[u8; 7],
    origin_x: i32,
    origin_y: i32,
    scale: i32,
    color: Color,
) {
    framebuffer.set_current_color(color);

    for (row_index, row_bits) in rows.iter().enumerate() {
        for column in 0..GLYPH_WIDTH {
            let bit_position = GLYPH_WIDTH - 1 - column;

            if (row_bits >> bit_position) & 1 == 0 {
                continue;
            }

            let pixel_x = origin_x + column * scale;

            let pixel_y = origin_y + row_index as i32 * scale;

            for offset_y in 0..scale {
                for offset_x in 0..scale {
                    framebuffer.point(pixel_x + offset_x, pixel_y + offset_y);
                }
            }
        }
    }
}

/// Dibuja `text` de izquierda a derecha comenzando en `(origin_x,
/// origin_y)`, con la fuente bitmap mínima local.
fn draw_text(
    framebuffer: &mut Framebuffer,
    text: &str,
    origin_x: i32,
    origin_y: i32,
    scale: i32,
    color: Color,
) {
    let mut cursor_x = origin_x;

    for character in text.chars() {
        let rows = glyph_rows(character);

        draw_glyph(framebuffer, &rows, cursor_x, origin_y, scale, color);

        cursor_x += (GLYPH_WIDTH + GLYPH_GAP) * scale;
    }
}

/// Disposición geométrica ya resuelta de la pantalla de Bienvenida
/// para un framebuffer concreto.
///
/// Cálculo puro, sin dependencia de `Framebuffer`/Raylib, para
/// poder probarse sin abrir una ventana.
struct WelcomeLayout {
    title_x: i32,
    title_y: i32,
    button_x: i32,
    button_y: i32,
    button_width: i32,
    button_height: i32,
}

fn compute_layout(framebuffer_width: i32, framebuffer_height: i32) -> WelcomeLayout {
    let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

    let title_x = centered_x(framebuffer_width, title_width);

    let title_y = TITLE_TOP_MARGIN;

    let play_text_width = text_width(PLAY_TEXT, PLAY_SCALE);

    let play_text_height = GLYPH_HEIGHT * PLAY_SCALE;

    let button_width = play_text_width + 2 * BUTTON_PADDING_X;

    let button_height = play_text_height + 2 * BUTTON_PADDING_Y;

    let button_x = centered_x(framebuffer_width, button_width);

    let button_y = framebuffer_height - PLAY_BOTTOM_MARGIN - button_height;

    WelcomeLayout {
        title_x,
        title_y,
        button_x,
        button_y,
        button_width,
        button_height,
    }
}

/// Convierte el tamaño del framebuffer, en un eje, a número de
/// celdas del Juego de la Vida usando `CELL_SIZE`, redondeando hacia
/// arriba para cubrir toda la pantalla.
///
/// Dimensiones no positivas producen `0` celdas (simulación vacía),
/// sin entrar en pánico.
fn grid_dimension(framebuffer_size: i32, cell_size: i32) -> usize {
    if framebuffer_size <= 0 || cell_size <= 0 {
        return 0;
    }

    let framebuffer_size = framebuffer_size as u32;

    let cell_size = cell_size as u32;

    ((framebuffer_size + cell_size - 1) / cell_size) as usize
}

/// Hash entero determinista de `(row, column)`, sin dependencia de
/// ninguna fuente de aleatoriedad externa (no crate `rand`, sin
/// semilla basada en tiempo). El mismo par produce siempre el mismo
/// resultado, en cualquier ejecución.
fn deterministic_hash(row: usize, column: usize) -> u64 {
    let mut hash =
        (row as u64).wrapping_mul(374_761_393) ^ (column as u64).wrapping_mul(668_265_263);

    hash ^= hash >> 13;
    hash = hash.wrapping_mul(1_274_126_177);
    hash ^= hash >> 16;

    hash
}

/// Densidad aproximada (1 de cada `SEED_DENSITY` celdas) del patrón
/// inicial determinista.
const SEED_DENSITY: u64 = 5;

/// Calcula, de forma puramente determinista, qué celdas de una
/// cuadrícula `grid_width x grid_height` comienzan vivas.
///
/// Misma entrada -> exactamente el mismo resultado siempre; no
/// existe estado oculto ni dependencia de tiempo/plataforma.
fn deterministic_seed_cells(grid_width: usize, grid_height: usize) -> Vec<(usize, usize)> {
    let mut cells = Vec::new();

    for row in 0..grid_height {
        for column in 0..grid_width {
            if deterministic_hash(row, column) % SEED_DENSITY == 0 {
                cells.push((row, column));
            }
        }
    }

    cells
}

/// Pantalla de Bienvenida: dueña de su propia simulación del Juego
/// de la Vida (fondo animado), del título `RED-BLACK MAZE` y del
/// control `PLAY`.
///
/// No depende de `Level`, `Player`, `GameSession` ni
/// `TextureManager`: es independiente del gameplay. `App` sigue
/// siendo el único responsable de decidir la transición de estado
/// (`Welcome -> LevelSelect`); esta pantalla solo actualiza/dibuja
/// su propia presentación.
pub(crate) struct WelcomeScreen {
    game_of_life: GameOfLife,
}

impl WelcomeScreen {
    /// Construye la pantalla de Bienvenida para un framebuffer de
    /// `framebuffer_width x framebuffer_height`, sembrando su Juego
    /// de la Vida de fondo de forma determinista.
    ///
    /// La cuadrícula se deriva ÚNICAMENTE del tamaño del
    /// framebuffer/UI (vía `CELL_SIZE`), nunca de `Level` ni de
    /// `BLOCK_SIZE`.
    pub(crate) fn new(framebuffer_width: i32, framebuffer_height: i32) -> Self {
        let grid_width = grid_dimension(framebuffer_width, CELL_SIZE);

        let grid_height = grid_dimension(framebuffer_height, CELL_SIZE);

        let mut game_of_life = GameOfLife::new(grid_width, grid_height, STEP_INTERVAL);

        game_of_life.seed(&deterministic_seed_cells(grid_width, grid_height));

        Self { game_of_life }
    }

    /// Avanza únicamente la simulación de fondo según el tiempo
    /// transcurrido. Delega enteramente en `GameOfLife::update`: no
    /// reimplementa ninguna regla de Conway.
    pub(crate) fn update(&mut self, delta_time: f32) {
        self.game_of_life.update(delta_time);
    }

    /// Dibuja la pantalla completa: fondo oscuro, Juego de la Vida
    /// en rojo, título y control `PLAY`, en ese orden.
    pub(crate) fn render(&self, framebuffer: &mut Framebuffer) {
        let framebuffer_width = framebuffer.width();

        let framebuffer_height = framebuffer.height();

        fill_rect(
            framebuffer,
            0,
            0,
            framebuffer_width,
            framebuffer_height,
            BACKGROUND_COLOR,
        );

        let game_of_life_config = GameOfLifeRenderConfig {
            origin_x: 0,
            origin_y: 0,
            cell_size: CELL_SIZE,
            cell_gap: CELL_GAP,
            alive_color: GAME_OF_LIFE_COLOR,
        };

        self.game_of_life.render(framebuffer, &game_of_life_config);

        let layout = compute_layout(framebuffer_width, framebuffer_height);

        // Título, con una pequeña sombra carmesí detrás para
        // resaltar sobre el fondo animado.
        draw_text(
            framebuffer,
            TITLE_TEXT,
            layout.title_x + 2,
            layout.title_y + 2,
            TITLE_SCALE,
            TITLE_SHADOW_COLOR,
        );

        draw_text(
            framebuffer,
            TITLE_TEXT,
            layout.title_x,
            layout.title_y,
            TITLE_SCALE,
            TITLE_COLOR,
        );

        // Control PLAY: panel oscuro con borde carmesí y texto
        // marfil centrado.
        fill_rect(
            framebuffer,
            layout.button_x,
            layout.button_y,
            layout.button_x + layout.button_width,
            layout.button_y + layout.button_height,
            BUTTON_BORDER_COLOR,
        );

        fill_rect(
            framebuffer,
            layout.button_x + BUTTON_BORDER_THICKNESS,
            layout.button_y + BUTTON_BORDER_THICKNESS,
            layout.button_x + layout.button_width - BUTTON_BORDER_THICKNESS,
            layout.button_y + layout.button_height - BUTTON_BORDER_THICKNESS,
            BUTTON_PANEL_COLOR,
        );

        draw_text(
            framebuffer,
            PLAY_TEXT,
            layout.button_x + BUTTON_PADDING_X,
            layout.button_y + BUTTON_PADDING_Y,
            PLAY_SCALE,
            BUTTON_TEXT_COLOR,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE_WIDTH: i32 = 624;
    const REFERENCE_HEIGHT: i32 = 432;

    #[test]
    fn title_text_width_is_positive_and_fits_reference_framebuffer() {
        let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

        assert!(title_width > 0);
        assert!(title_width <= REFERENCE_WIDTH);
    }

    #[test]
    fn play_button_is_horizontally_centered_for_reference_framebuffer() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let expected_x = (REFERENCE_WIDTH - layout.button_width) / 2;

        assert_eq!(layout.button_x, expected_x);
        assert!(layout.button_x >= 0);
    }

    #[test]
    fn layout_remains_within_reference_framebuffer_bounds() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

        assert!(layout.title_x >= 0);
        assert!(layout.title_x + title_width <= REFERENCE_WIDTH);

        assert!(layout.button_x >= 0);
        assert!(layout.button_x + layout.button_width <= REFERENCE_WIDTH);

        assert!(layout.button_y >= 0);
        assert!(layout.button_y + layout.button_height <= REFERENCE_HEIGHT);
    }

    #[test]
    fn deterministic_seed_creates_at_least_one_alive_cell() {
        let grid_width = grid_dimension(REFERENCE_WIDTH, CELL_SIZE);

        let grid_height = grid_dimension(REFERENCE_HEIGHT, CELL_SIZE);

        let cells = deterministic_seed_cells(grid_width, grid_height);

        assert!(!cells.is_empty());
    }

    #[test]
    fn deterministic_construction_produces_the_same_seed_twice() {
        let grid_width = grid_dimension(REFERENCE_WIDTH, CELL_SIZE);

        let grid_height = grid_dimension(REFERENCE_HEIGHT, CELL_SIZE);

        let first = deterministic_seed_cells(grid_width, grid_height);

        let second = deterministic_seed_cells(grid_width, grid_height);

        assert_eq!(first, second);
    }

    #[test]
    fn tiny_framebuffer_layout_does_not_panic() {
        let layout = compute_layout(1, 1);

        // No se exige que quepa: solo que el cálculo sea seguro.
        let _ = layout.title_x;
        let _ = layout.button_x;

        assert_eq!(grid_dimension(0, CELL_SIZE), 0);
        assert_eq!(grid_dimension(1, 0), 0);

        let _screen = WelcomeScreen::new(1, 1);
        let _screen_zero = WelcomeScreen::new(0, 0);
    }
}
