use std::collections::HashSet;

use raylib::prelude::Color;

use crate::rendering::framebuffer::Framebuffer;
use crate::ui::{GameOfLife, GameOfLifeRenderConfig};

/// Tamaño, en píxeles de framebuffer, de cada celda del Juego de la
/// Vida de fondo. Deliberadamente ajeno a `BLOCK_SIZE`/`Level`: es
/// una unidad puramente de presentación de UI, propia de esta
/// pantalla (independiente de las de Bienvenida y Selección de
/// Nivel).
const CELL_SIZE: i32 = 8;

/// Separación, en píxeles, entre celdas contiguas del fondo.
const CELL_GAP: i32 = 1;

/// Intervalo de paso del Juego de la Vida de Victoria.
const STEP_INTERVAL: f32 = 0.12;

const BACKGROUND_COLOR: Color = Color::new(5, 5, 8, 255);

/// Rojo/carmesí, dentro del rango sugerido para una pantalla
/// celebratoria, sin dejar de ser legible junto al texto.
const GAME_OF_LIFE_COLOR: Color = Color::new(165, 22, 34, 255);

const TITLE_COLOR: Color = Color::new(224, 218, 205, 255);

const SELECTED_PANEL_COLOR: Color = Color::new(40, 10, 14, 255);
const SELECTED_BORDER_COLOR: Color = Color::new(210, 40, 50, 255);
const SELECTED_TEXT_COLOR: Color = Color::new(235, 230, 215, 255);

const UNSELECTED_PANEL_COLOR: Color = Color::new(14, 12, 14, 255);
const UNSELECTED_TEXT_COLOR: Color = Color::new(150, 144, 132, 255);

/// Estilo de la fila `NEXT LEVEL` cuando no existe nivel siguiente:
/// notablemente más apagado que una fila simplemente "no
/// seleccionada", para que quede claro que la opción no está
/// disponible en absoluto.
const DISABLED_PANEL_COLOR: Color = Color::new(9, 8, 10, 255);
const DISABLED_TEXT_COLOR: Color = Color::new(70, 66, 62, 255);

const TITLE_TEXT: &str = "LEVEL COMPLETE";
const TITLE_SCALE: i32 = 4;
const TITLE_TOP_MARGIN: i32 = 32;
const TITLE_TO_ROWS_GAP: i32 = 28;

/// Etiquetas de acción, en el orden visual exacto requerido:
/// `0 -> NEXT LEVEL`, `1 -> RETRY`, `2 -> MAIN MENU`.
const ACTION_LABELS: [&str; 3] = ["NEXT LEVEL", "RETRY", "MAIN MENU"];

const ROW_SCALE: i32 = 3;
const ROW_PADDING: i32 = 10;
const ROW_BORDER_THICKNESS: i32 = 2;
const ROW_SPACING: i32 = 12;

/// Ancho fijo de cada fila, suficientemente amplio para la etiqueta
/// más larga (`NEXT LEVEL`); verificado por
/// `longest_known_row_text_fits_within_row_width`.
const ROW_WIDTH: i32 = 260;

/// Ancho/alto, en píxeles lógicos (sin escalar), de un glifo.
const GLYPH_WIDTH: i32 = 5;
const GLYPH_HEIGHT: i32 = 7;

/// Separación horizontal, en píxeles lógicos, entre glifos.
const GLYPH_GAP: i32 = 1;

/// Índice de fila que representa `NEXT LEVEL`. Es la única fila que
/// puede quedar deshabilitada.
const NEXT_LEVEL_ROW: usize = 0;
const RETRY_ROW: usize = 1;
const MAIN_MENU_ROW: usize = 2;

/// Acción de dominio de UI seleccionable en la pantalla de
/// Victoria.
///
/// `VictoryScreen` solo produce este valor; `App` es quien realiza
/// la operación real de `LevelManager`/`GameSession` asociada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VictoryAction {
    NextLevel,
    Retry,
    MainMenu,
}

/// Fuente bitmap 5x7 mínima, privada de esta pantalla: solo los
/// glifos que `LEVEL COMPLETE`/`NEXT LEVEL`/`RETRY`/`MAIN MENU`
/// requieren realmente. Deliberadamente NO comparte implementación
/// con las fuentes de `welcome.rs`, `level_select.rs` ni
/// `rendering/hud.rs`.
fn glyph_rows(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
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
        'P' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'R' => [
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
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
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
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

/// Disposición geométrica ya resuelta de la pantalla de Victoria
/// para un framebuffer concreto.
///
/// Cálculo puro, sin dependencia de `Framebuffer`/Raylib: las filas
/// tienen tamaño fijo, para poder probarse sin abrir una ventana.
struct VictoryLayout {
    title_x: i32,
    title_y: i32,
    row_x: i32,
    row_width: i32,
    row_height: i32,
    rows_top: i32,
    row_spacing: i32,
}

impl VictoryLayout {
    /// Coordenada Y superior de la fila `row_index` (0-based).
    fn row_y(&self, row_index: usize) -> i32 {
        self.rows_top + row_index as i32 * (self.row_height + self.row_spacing)
    }
}

fn compute_layout(framebuffer_width: i32, framebuffer_height: i32) -> VictoryLayout {
    let _ = framebuffer_height;

    let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

    let title_x = centered_x(framebuffer_width, title_width);

    let title_y = TITLE_TOP_MARGIN;

    let row_width = ROW_WIDTH;

    let row_height = GLYPH_HEIGHT * ROW_SCALE + 2 * ROW_PADDING;

    let row_x = centered_x(framebuffer_width, row_width);

    let rows_top = title_y + GLYPH_HEIGHT * TITLE_SCALE + TITLE_TO_ROWS_GAP;

    VictoryLayout {
        title_x,
        title_y,
        row_x,
        row_width,
        row_height,
        rows_top,
        row_spacing: ROW_SPACING,
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

/// Patrones base deterministas (sin aleatoriedad), colocados
/// manualmente dentro del cuadrante superior-izquierdo de la
/// cuadrícula: un bloque 2x2, un blinker horizontal y otro bloque
/// 2x2 más alejado. Son coordenadas literales fijas, no generadas
/// por ninguna fuente de azar.
fn base_pattern_cells() -> [(usize, usize); 11] {
    [
        // Bloque 2x2.
        (4, 4),
        (4, 5),
        (5, 4),
        (5, 5),
        // Blinker horizontal.
        (10, 10),
        (10, 11),
        (10, 12),
        // Segundo bloque 2x2.
        (16, 6),
        (16, 7),
        (17, 6),
        (17, 7),
    ]
}

/// Calcula, de forma puramente determinista, qué celdas de una
/// cuadrícula `grid_width x grid_height` comienzan vivas.
///
/// Toma el patrón base fijo (§`base_pattern_cells`) y lo refleja
/// tanto verticalmente (a través del eje horizontal central) como
/// horizontalmente (a través del eje vertical central), produciendo
/// una semilla con simetría de cuatro pliegues: notablemente más
/// ordenada que las semillas tipo "ruido" de Bienvenida/Selección
/// de Nivel. No usa `rand` ni ninguna semilla basada en tiempo; la
/// misma entrada produce siempre exactamente el mismo resultado.
fn deterministic_seed_cells(grid_width: usize, grid_height: usize) -> Vec<(usize, usize)> {
    if grid_width == 0 || grid_height == 0 {
        return Vec::new();
    }

    let half_width = grid_width / 2;

    let half_height = grid_height / 2;

    let mut cells: HashSet<(usize, usize)> = HashSet::new();

    for &(row, column) in base_pattern_cells().iter() {
        // Un patrón que no cabe dentro del cuadrante para esta
        // cuadrícula (p. ej. framebuffer diminuto) simplemente se
        // omite, sin entrar en pánico.
        if row >= half_height || column >= half_width {
            continue;
        }

        let mirrored_row = grid_height - 1 - row;

        let mirrored_column = grid_width - 1 - column;

        cells.insert((row, column));
        cells.insert((row, mirrored_column));
        cells.insert((mirrored_row, column));
        cells.insert((mirrored_row, mirrored_column));
    }

    let mut result: Vec<(usize, usize)> = cells.into_iter().collect();

    result.sort_unstable();

    result
}

/// Pantalla de Victoria: dueña de su propia simulación del Juego de
/// la Vida (fondo animado simétrico, independiente de las de
/// Bienvenida y Selección de Nivel) y del índice de selección
/// actual entre `NEXT LEVEL`/`RETRY`/`MAIN MENU`.
///
/// No posee `Level`, `GameSession`, `Player`, `TextureManager` ni
/// una `LevelManager`: solo produce un `VictoryAction`; `App` es
/// quien decide y ejecuta la operación real de
/// `LevelManager`/`GameSession` asociada.
pub(crate) struct VictoryScreen {
    selected_index: usize,
    game_of_life: GameOfLife,
}

impl VictoryScreen {
    /// Construye la pantalla para un framebuffer de
    /// `framebuffer_width x framebuffer_height`, sembrando su Juego
    /// de la Vida de fondo de forma determinista y simétrica.
    pub(crate) fn new(framebuffer_width: i32, framebuffer_height: i32) -> Self {
        let grid_width = grid_dimension(framebuffer_width, CELL_SIZE);

        let grid_height = grid_dimension(framebuffer_height, CELL_SIZE);

        let mut game_of_life = GameOfLife::new(grid_width, grid_height, STEP_INTERVAL);

        game_of_life.seed(&deterministic_seed_cells(grid_width, grid_height));

        Self {
            selected_index: RETRY_ROW,
            game_of_life,
        }
    }

    /// Índice actualmente seleccionado.
    #[allow(dead_code)]
    pub(crate) fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Debe llamarse cada vez que `App` entra a esta pantalla
    /// (`Playing -> Victory`), para reiniciar la selección según si
    /// existe un nivel siguiente.
    ///
    /// NO construye el Juego de la Vida ni reconstruye la pantalla:
    /// esta sigue siendo propiedad persistente de `App`.
    pub(crate) fn on_enter(&mut self, has_next_level: bool) {
        self.selected_index = if has_next_level {
            NEXT_LEVEL_ROW
        } else {
            RETRY_ROW
        };
    }

    /// Mueve la selección al elemento anterior disponible.
    ///
    /// Si `has_next_level` es `false`, `NEXT LEVEL` está
    /// deshabilitada y la navegación cicla únicamente entre `RETRY`
    /// y `MAIN MENU`.
    pub(crate) fn select_previous(&mut self, has_next_level: bool) {
        self.selected_index = match (self.selected_index, has_next_level) {
            (NEXT_LEVEL_ROW, true) => MAIN_MENU_ROW,
            (RETRY_ROW, true) => NEXT_LEVEL_ROW,
            (MAIN_MENU_ROW, true) => RETRY_ROW,

            // `has_next_level == false`: NEXT_LEVEL nunca es un
            // destino válido, incluso si `selected_index` llegara a
            // valer `NEXT_LEVEL_ROW` por alguna vía inesperada.
            (RETRY_ROW, false) => MAIN_MENU_ROW,
            (MAIN_MENU_ROW, false) => RETRY_ROW,
            (_, false) => RETRY_ROW,

            _ => self.selected_index,
        };
    }

    /// Mueve la selección al siguiente elemento disponible.
    ///
    /// Misma exclusión de `NEXT LEVEL` que `select_previous` cuando
    /// `has_next_level` es `false`.
    pub(crate) fn select_next(&mut self, has_next_level: bool) {
        self.selected_index = match (self.selected_index, has_next_level) {
            (NEXT_LEVEL_ROW, true) => RETRY_ROW,
            (RETRY_ROW, true) => MAIN_MENU_ROW,
            (MAIN_MENU_ROW, true) => NEXT_LEVEL_ROW,

            (RETRY_ROW, false) => MAIN_MENU_ROW,
            (MAIN_MENU_ROW, false) => RETRY_ROW,
            (_, false) => RETRY_ROW,

            _ => self.selected_index,
        };
    }

    /// Acción actualmente seleccionada, o `None` si la fila
    /// seleccionada está deshabilitada (`NEXT LEVEL` sin nivel
    /// siguiente).
    ///
    /// Comprobación defensiva: incluso si `selected_index`
    /// terminara apuntando a `NEXT_LEVEL_ROW` mientras
    /// `has_next_level` es `false`, esta función retorna `None` en
    /// vez de `Some(VictoryAction::NextLevel)`, por lo que una
    /// acción deshabilitada nunca puede ejecutarse.
    pub(crate) fn selected_action(&self, has_next_level: bool) -> Option<VictoryAction> {
        match self.selected_index {
            NEXT_LEVEL_ROW if has_next_level => Some(VictoryAction::NextLevel),
            NEXT_LEVEL_ROW => None,
            RETRY_ROW => Some(VictoryAction::Retry),
            MAIN_MENU_ROW => Some(VictoryAction::MainMenu),
            _ => None,
        }
    }

    /// Avanza únicamente la simulación de fondo según el tiempo
    /// transcurrido. Delega enteramente en `GameOfLife::update`.
    pub(crate) fn update(&mut self, delta_time: f32) {
        self.game_of_life.update(delta_time);
    }

    /// Dibuja la pantalla completa: fondo oscuro, Juego de la Vida
    /// simétrico, título `LEVEL COMPLETE` y las tres filas de
    /// acción, con la fila `NEXT LEVEL` visiblemente deshabilitada
    /// cuando `has_next_level` es `false`.
    pub(crate) fn render(&self, framebuffer: &mut Framebuffer, has_next_level: bool) {
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

        for (index, label) in ACTION_LABELS.iter().enumerate() {
            let disabled = index == NEXT_LEVEL_ROW && !has_next_level;

            let selected = index == self.selected_index && !disabled;

            let (panel_color, border_color, text_color) = if disabled {
                (
                    DISABLED_PANEL_COLOR,
                    DISABLED_PANEL_COLOR,
                    DISABLED_TEXT_COLOR,
                )
            } else if selected {
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

            let row_y = layout.row_y(index);

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

            draw_text(
                framebuffer,
                label,
                layout.row_x + ROW_PADDING,
                row_y + ROW_PADDING,
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
    fn has_next_entry_selects_next_level() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(true);

        assert_eq!(screen.selected_index(), NEXT_LEVEL_ROW);
    }

    #[test]
    fn final_level_entry_selects_retry() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(false);

        assert_eq!(screen.selected_index(), RETRY_ROW);
    }

    #[test]
    fn normal_next_navigation_cycles_next_retry_main_menu() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(true);

        assert_eq!(screen.selected_index(), NEXT_LEVEL_ROW);

        screen.select_next(true);
        assert_eq!(screen.selected_index(), RETRY_ROW);

        screen.select_next(true);
        assert_eq!(screen.selected_index(), MAIN_MENU_ROW);

        screen.select_next(true);
        assert_eq!(screen.selected_index(), NEXT_LEVEL_ROW);
    }

    #[test]
    fn normal_previous_navigation_wraps() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(true);

        screen.select_previous(true);
        assert_eq!(screen.selected_index(), MAIN_MENU_ROW);
    }

    #[test]
    fn final_level_navigation_cycles_retry_and_main_menu_only() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(false);

        assert_eq!(screen.selected_index(), RETRY_ROW);

        screen.select_next(false);
        assert_eq!(screen.selected_index(), MAIN_MENU_ROW);

        screen.select_next(false);
        assert_eq!(screen.selected_index(), RETRY_ROW);

        screen.select_previous(false);
        assert_eq!(screen.selected_index(), MAIN_MENU_ROW);

        screen.select_previous(false);
        assert_eq!(screen.selected_index(), RETRY_ROW);
    }

    #[test]
    fn final_level_navigation_never_selects_disabled_next() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(false);

        for _ in 0..10 {
            screen.select_next(false);
            assert_ne!(screen.selected_index(), NEXT_LEVEL_ROW);
        }

        for _ in 0..10 {
            screen.select_previous(false);
            assert_ne!(screen.selected_index(), NEXT_LEVEL_ROW);
        }
    }

    #[test]
    fn disabled_next_returns_no_executable_action() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        // Estado defensivo: aunque `selected_index` apuntara a
        // NEXT_LEVEL_ROW mientras no hay nivel siguiente,
        // `selected_action` debe rechazarlo.
        screen.selected_index = NEXT_LEVEL_ROW;

        assert_eq!(screen.selected_action(false), None);
    }

    #[test]
    fn retry_maps_to_retry_action() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(false);

        assert_eq!(screen.selected_action(false), Some(VictoryAction::Retry));
        assert_eq!(screen.selected_action(true), Some(VictoryAction::Retry));
    }

    #[test]
    fn main_menu_maps_to_main_menu_action() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.selected_index = MAIN_MENU_ROW;

        assert_eq!(screen.selected_action(true), Some(VictoryAction::MainMenu));
    }

    #[test]
    fn next_level_selected_with_next_available_maps_to_next_level_action() {
        let mut screen = VictoryScreen::new(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        screen.on_enter(true);

        assert_eq!(screen.selected_action(true), Some(VictoryAction::NextLevel));
    }

    #[test]
    fn title_fits_reference_framebuffer() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

        assert!(title_width > 0);
        assert!(layout.title_x >= 0);
        assert!(layout.title_x + title_width <= REFERENCE_WIDTH);
    }

    #[test]
    fn three_rows_fit_and_do_not_overlap() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        assert!(layout.row_x >= 0);
        assert!(layout.row_x + layout.row_width <= REFERENCE_WIDTH);

        for index in 0..3usize {
            let row_top = layout.row_y(index);

            assert!(row_top >= 0);
            assert!(row_top + layout.row_height <= REFERENCE_HEIGHT);
        }

        let row0_bottom = layout.row_y(0) + layout.row_height;
        let row1_top = layout.row_y(1);
        let row1_bottom = layout.row_y(1) + layout.row_height;
        let row2_top = layout.row_y(2);

        assert!(row0_bottom <= row1_top);
        assert!(row1_bottom <= row2_top);
    }

    #[test]
    fn longest_known_row_text_fits_within_row_width() {
        let longest_label_width = text_width("NEXT LEVEL", ROW_SCALE);

        let content_width = longest_label_width + 2 * ROW_PADDING;

        assert!(content_width <= ROW_WIDTH);
    }

    #[test]
    fn tiny_framebuffer_layout_does_not_panic() {
        let layout = compute_layout(1, 1);

        let _ = layout.title_x;
        let _ = layout.row_x;

        assert_eq!(grid_dimension(0, CELL_SIZE), 0);
        assert_eq!(grid_dimension(1, 0), 0);

        let _screen = VictoryScreen::new(1, 1);
        let _screen_zero = VictoryScreen::new(0, 0);
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
    fn seed_is_vertically_symmetric() {
        let grid_width = grid_dimension(REFERENCE_WIDTH, CELL_SIZE);

        let grid_height = grid_dimension(REFERENCE_HEIGHT, CELL_SIZE);

        let cells: HashSet<(usize, usize)> = deterministic_seed_cells(grid_width, grid_height)
            .into_iter()
            .collect();

        for &(row, column) in &cells {
            let mirrored_row = grid_height - 1 - row;

            assert!(
                cells.contains(&(mirrored_row, column)),
                "falta el reflejo vertical de ({row},{column})"
            );
        }
    }

    #[test]
    fn seed_is_horizontally_symmetric() {
        let grid_width = grid_dimension(REFERENCE_WIDTH, CELL_SIZE);

        let grid_height = grid_dimension(REFERENCE_HEIGHT, CELL_SIZE);

        let cells: HashSet<(usize, usize)> = deterministic_seed_cells(grid_width, grid_height)
            .into_iter()
            .collect();

        for &(row, column) in &cells {
            let mirrored_column = grid_width - 1 - column;

            assert!(
                cells.contains(&(row, mirrored_column)),
                "falta el reflejo horizontal de ({row},{column})"
            );
        }
    }

    #[test]
    fn seed_coordinates_remain_in_bounds() {
        let grid_width = grid_dimension(REFERENCE_WIDTH, CELL_SIZE);

        let grid_height = grid_dimension(REFERENCE_HEIGHT, CELL_SIZE);

        for &(row, column) in &deterministic_seed_cells(grid_width, grid_height) {
            assert!(row < grid_height);
            assert!(column < grid_width);
        }
    }
}
