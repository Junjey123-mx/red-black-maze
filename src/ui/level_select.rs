use raylib::prelude::Color;

use crate::rendering::framebuffer::Framebuffer;
use crate::ui::{GameOfLife, GameOfLifeRenderConfig};
use crate::world::LevelManager;

/// Tamaño, en píxeles de framebuffer, de cada celda del Juego de la
/// Vida de fondo. Deliberadamente ajeno a `BLOCK_SIZE`/`Level`: es
/// una unidad puramente de presentación de UI, propia de esta
/// pantalla (independiente de la de Bienvenida).
const CELL_SIZE: i32 = 8;

/// Separación, en píxeles, entre celdas contiguas del fondo.
const CELL_GAP: i32 = 1;

/// Intervalo de paso del Juego de la Vida de Selección de Nivel.
const STEP_INTERVAL: f32 = 0.12;

const BACKGROUND_COLOR: Color = Color::new(5, 5, 8, 255);

/// Rojo notablemente más apagado que el de Bienvenida
/// (`Color::new(195, 28, 38, 255)`), para que los nombres de nivel y
/// la selección dominen visualmente.
const GAME_OF_LIFE_COLOR: Color = Color::new(100, 14, 20, 255);

const TITLE_COLOR: Color = Color::new(224, 218, 205, 255);

const SELECTED_PANEL_COLOR: Color = Color::new(40, 10, 14, 255);
const SELECTED_BORDER_COLOR: Color = Color::new(210, 40, 50, 255);
const SELECTED_TEXT_COLOR: Color = Color::new(235, 230, 215, 255);

const UNSELECTED_PANEL_COLOR: Color = Color::new(14, 12, 14, 255);
const UNSELECTED_TEXT_COLOR: Color = Color::new(150, 144, 132, 255);

const TITLE_TEXT: &str = "SELECT LEVEL";
const TITLE_SCALE: i32 = 4;
const TITLE_TOP_MARGIN: i32 = 32;
const TITLE_TO_ROWS_GAP: i32 = 28;

/// Etiquetas romanas de presentación. Pertenecen a esta pantalla,
/// NO a `LevelManager`: son puramente visuales.
const ROMAN_NUMERALS: [&str; 3] = ["I", "II", "III"];

const ROW_SCALE: i32 = 3;
const ROW_PADDING: i32 = 10;
const ROW_BORDER_THICKNESS: i32 = 2;
const ROW_SPACING: i32 = 12;
const ROMAN_COLUMN_GAP: i32 = 8;

/// Ancho fijo de cada fila, suficientemente amplio para el nombre
/// más largo del catálogo actual (`HOUSE OF CARDS`); verificado por
/// `longest_known_row_text_fits_within_row_width`.
const ROW_WIDTH: i32 = 340;

/// Ancho/alto, en píxeles lógicos (sin escalar), de un glifo.
const GLYPH_WIDTH: i32 = 5;
const GLYPH_HEIGHT: i32 = 7;

/// Separación horizontal, en píxeles lógicos, entre glifos.
const GLYPH_GAP: i32 = 1;

/// Fuente bitmap 5x7 mínima, privada de esta pantalla: solo los
/// glifos que `SELECT LEVEL` y los tres nombres de nivel en
/// mayúsculas requieren realmente. Deliberadamente NO comparte
/// implementación con la fuente de `welcome.rs` ni con la de
/// `rendering/hud.rs`.
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
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
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
        'N' => [
            0b10001, 0b11001, 0b10101, 0b10101, 0b10011, 0b10001, 0b10001,
        ],
        'O' => [
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ],
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'V' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
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

/// Disposición geométrica ya resuelta de la pantalla de Selección
/// de Nivel para un framebuffer concreto.
///
/// Cálculo puro, sin dependencia de `Framebuffer`/Raylib ni de
/// `LevelManager`: las filas tienen tamaño fijo, independiente del
/// número real de niveles, para poder probarse sin abrir una
/// ventana.
struct LevelSelectLayout {
    title_x: i32,
    title_y: i32,
    row_x: i32,
    row_width: i32,
    row_height: i32,
    rows_top: i32,
    row_spacing: i32,
    roman_column_width: i32,
}

impl LevelSelectLayout {
    /// Coordenada Y superior de la fila `row_index` (0-based).
    fn row_y(&self, row_index: usize) -> i32 {
        self.rows_top + row_index as i32 * (self.row_height + self.row_spacing)
    }
}

fn compute_layout(framebuffer_width: i32, framebuffer_height: i32) -> LevelSelectLayout {
    let _ = framebuffer_height;

    let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

    let title_x = centered_x(framebuffer_width, title_width);

    let title_y = TITLE_TOP_MARGIN;

    let roman_column_width = text_width("III", ROW_SCALE) + ROMAN_COLUMN_GAP;

    let row_width = ROW_WIDTH;

    let row_height = GLYPH_HEIGHT * ROW_SCALE + 2 * ROW_PADDING;

    let row_x = centered_x(framebuffer_width, row_width);

    let rows_top = title_y + GLYPH_HEIGHT * TITLE_SCALE + TITLE_TO_ROWS_GAP;

    LevelSelectLayout {
        title_x,
        title_y,
        row_x,
        row_width,
        row_height,
        rows_top,
        row_spacing: ROW_SPACING,
        roman_column_width,
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
/// semilla basada en tiempo). Usa constantes distintas a las de
/// `welcome.rs` para producir un patrón visualmente diferente,
/// aunque igualmente determinista.
fn deterministic_hash(row: usize, column: usize) -> u64 {
    let mut hash = (row as u64).wrapping_mul(2_654_435_761) ^ (column as u64).wrapping_mul(40_503);

    hash ^= hash >> 15;
    hash = hash.wrapping_mul(2_246_822_519);
    hash ^= hash >> 13;

    hash
}

/// Densidad aproximada (1 de cada `SEED_DENSITY` celdas) del patrón
/// inicial determinista.
const SEED_DENSITY: u64 = 7;

/// Calcula, de forma puramente determinista, qué celdas de una
/// cuadrícula `grid_width x grid_height` comienzan vivas.
///
/// Misma entrada -> exactamente el mismo resultado siempre.
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

/// Pantalla de Selección de Nivel: dueña de su propia simulación
/// del Juego de la Vida (fondo animado, independiente de la de
/// Bienvenida) y del índice de selección actual.
///
/// No posee `Level`, `GameSession`, `Player`, `TextureManager` ni
/// una `LevelManager` propia: `render` solo LEE el catálogo a
/// través de una referencia prestada, y nunca conoce ninguna ruta
/// de archivo. `App` sigue siendo el único responsable de cargar el
/// nivel elegido y de decidir la transición de estado.
pub(crate) struct LevelSelectScreen {
    selected_index: usize,
    game_of_life: GameOfLife,
}

impl LevelSelectScreen {
    /// Construye la pantalla para un framebuffer de
    /// `framebuffer_width x framebuffer_height`, sembrando su Juego
    /// de la Vida de fondo de forma determinista y comenzando la
    /// selección en el índice `0`.
    pub(crate) fn new(framebuffer_width: i32, framebuffer_height: i32) -> Self {
        let grid_width = grid_dimension(framebuffer_width, CELL_SIZE);

        let grid_height = grid_dimension(framebuffer_height, CELL_SIZE);

        let mut game_of_life = GameOfLife::new(grid_width, grid_height, STEP_INTERVAL);

        game_of_life.seed(&deterministic_seed_cells(grid_width, grid_height));

        Self {
            selected_index: 0,
            game_of_life,
        }
    }

    /// Índice actualmente seleccionado.
    pub(crate) fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Mueve la selección al elemento anterior, con envoltura
    /// (`0 -> level_count - 1`).
    ///
    /// `level_count == 0` se ignora de forma segura (sin módulo por
    /// cero ni pánico).
    pub(crate) fn select_previous(&mut self, level_count: usize) {
        if level_count == 0 {
            return;
        }

        self.selected_index = if self.selected_index == 0 {
            level_count - 1
        } else {
            self.selected_index - 1
        };
    }

    /// Mueve la selección al siguiente elemento, con envoltura
    /// (`level_count - 1 -> 0`).
    ///
    /// `level_count == 0` se ignora de forma segura.
    pub(crate) fn select_next(&mut self, level_count: usize) {
        if level_count == 0 {
            return;
        }

        self.selected_index = (self.selected_index + 1) % level_count;
    }

    /// Avanza únicamente la simulación de fondo según el tiempo
    /// transcurrido. Delega enteramente en `GameOfLife::update`.
    pub(crate) fn update(&mut self, delta_time: f32) {
        self.game_of_life.update(delta_time);
    }

    /// Dibuja la pantalla completa: fondo oscuro, Juego de la Vida
    /// apagado, título `SELECT LEVEL` y las filas de nivel
    /// provistas por `level_manager`, con la fila seleccionada
    /// resaltada.
    ///
    /// Puramente de lectura sobre `level_manager`: nunca llama
    /// `load`/`restart`/`next`.
    pub(crate) fn render(&self, framebuffer: &mut Framebuffer, level_manager: &LevelManager) {
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

        draw_text(
            framebuffer,
            TITLE_TEXT,
            layout.title_x,
            layout.title_y,
            TITLE_SCALE,
            TITLE_COLOR,
        );

        for index in 0..level_manager.level_count() {
            let Some(name) = level_manager.level_name(index) else {
                continue;
            };

            let roman = ROMAN_NUMERALS.get(index).copied().unwrap_or("");

            let row_y = layout.row_y(index);

            let selected = index == self.selected_index;

            let (panel_color, border_color, text_color) = if selected {
                (
                    SELECTED_PANEL_COLOR,
                    SELECTED_BORDER_COLOR,
                    SELECTED_TEXT_COLOR,
                )
            } else {
                (
                    UNSELECTED_PANEL_COLOR,
                    UNSELECTED_PANEL_COLOR,
                    UNSELECTED_TEXT_COLOR,
                )
            };

            fill_rect(
                framebuffer,
                layout.row_x,
                row_y,
                layout.row_x + layout.row_width,
                row_y + layout.row_height,
                border_color,
            );

            fill_rect(
                framebuffer,
                layout.row_x + ROW_BORDER_THICKNESS,
                row_y + ROW_BORDER_THICKNESS,
                layout.row_x + layout.row_width - ROW_BORDER_THICKNESS,
                row_y + layout.row_height - ROW_BORDER_THICKNESS,
                panel_color,
            );

            let text_y = row_y + ROW_PADDING;

            draw_text(
                framebuffer,
                roman,
                layout.row_x + ROW_PADDING,
                text_y,
                ROW_SCALE,
                text_color,
            );

            let name_uppercase = name.to_ascii_uppercase();

            draw_text(
                framebuffer,
                &name_uppercase,
                layout.row_x + ROW_PADDING + layout.roman_column_width,
                text_y,
                ROW_SCALE,
                text_color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REFERENCE_WIDTH: i32 = 624;
    const REFERENCE_HEIGHT: i32 = 432;

    #[test]
    fn initial_selection_is_index_zero() {
        let screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        assert_eq!(screen.selected_index(), 0);
    }

    #[test]
    fn select_next_advances_by_one() {
        let mut screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.select_next(3);

        assert_eq!(screen.selected_index(), 1);
    }

    #[test]
    fn select_next_wraps_from_last_to_first() {
        let mut screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.select_next(3);
        screen.select_next(3);
        screen.select_next(3);

        assert_eq!(screen.selected_index(), 0);
    }

    #[test]
    fn select_previous_moves_back_by_one() {
        let mut screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.select_next(3);
        screen.select_next(3);

        screen.select_previous(3);

        assert_eq!(screen.selected_index(), 1);
    }

    #[test]
    fn select_previous_wraps_from_first_to_last() {
        let mut screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.select_previous(3);

        assert_eq!(screen.selected_index(), 2);
    }

    #[test]
    fn zero_level_count_does_not_panic_and_does_not_change_selection() {
        let mut screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.select_next(0);
        screen.select_previous(0);

        assert_eq!(screen.selected_index(), 0);
    }

    #[test]
    fn selection_always_remains_within_bounds() {
        let mut screen = LevelSelectScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        for _ in 0..10 {
            screen.select_next(3);
            assert!(screen.selected_index() < 3);
        }

        for _ in 0..10 {
            screen.select_previous(3);
            assert!(screen.selected_index() < 3);
        }
    }

    #[test]
    fn rows_fit_within_reference_framebuffer_bounds() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        assert!(layout.row_x >= 0);
        assert!(layout.row_x + layout.row_width <= REFERENCE_WIDTH);

        for index in 0..3usize {
            let row_top = layout.row_y(index);

            assert!(row_top >= 0);
            assert!(row_top + layout.row_height <= REFERENCE_HEIGHT);
        }
    }

    #[test]
    fn three_rows_have_non_overlapping_vertical_positions() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let row0_bottom = layout.row_y(0) + layout.row_height;

        let row1_top = layout.row_y(1);

        let row1_bottom = layout.row_y(1) + layout.row_height;

        let row2_top = layout.row_y(2);

        assert!(row0_bottom <= row1_top);
        assert!(row1_bottom <= row2_top);
    }

    #[test]
    fn longest_known_row_text_fits_within_row_width() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let longest_name_width = text_width("HOUSE OF CARDS", ROW_SCALE);

        let content_width = layout.roman_column_width + longest_name_width + 2 * ROW_PADDING;

        assert!(content_width <= layout.row_width);
    }

    #[test]
    fn tiny_framebuffer_layout_does_not_panic() {
        let layout = compute_layout(1, 1);

        let _ = layout.title_x;
        let _ = layout.row_x;

        assert_eq!(grid_dimension(0, CELL_SIZE), 0);
        assert_eq!(grid_dimension(1, 0), 0);

        let _screen = LevelSelectScreen::new(1, 1);
        let _screen_zero = LevelSelectScreen::new(0, 0);
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
}
