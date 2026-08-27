use raylib::prelude::Color;

use crate::rendering::framebuffer::Framebuffer;
use crate::ui::Hitbox;

/// Opción seleccionable del menú de pausa.
///
/// `PauseScreen` solo produce/recuerda este valor; `App` decide qué
/// transición de `GameState` corresponde a cada uno (`Continue ->
/// Playing`, `ExitToMenu -> Welcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PauseMenuItem {
    Continue,
    ExitToMenu,
}

impl PauseMenuItem {
    /// Con solo dos opciones, "anterior" y "siguiente" son la MISMA
    /// operación (alternar). Se mantienen como dos métodos
    /// separados únicamente para preservar la misma convención de
    /// navegación (arriba/abajo) que `LevelSelectScreen`/
    /// `VictoryScreen`, no porque el comportamiento difiera.
    fn toggled(self) -> Self {
        match self {
            PauseMenuItem::Continue => PauseMenuItem::ExitToMenu,
            PauseMenuItem::ExitToMenu => PauseMenuItem::Continue,
        }
    }
}

const OVERLAY_COLOR: Color = Color::new(0, 0, 0, 255);

const PANEL_COLOR: Color = Color::new(14, 12, 14, 255);
const PANEL_BORDER_COLOR: Color = Color::new(120, 18, 24, 255);

const TITLE_COLOR: Color = Color::new(224, 218, 205, 255);

const SELECTED_PANEL_COLOR: Color = Color::new(40, 10, 14, 255);
const SELECTED_BORDER_COLOR: Color = Color::new(210, 40, 50, 255);
const SELECTED_TEXT_COLOR: Color = Color::new(235, 230, 215, 255);

const UNSELECTED_PANEL_COLOR: Color = Color::new(20, 17, 20, 255);
const UNSELECTED_TEXT_COLOR: Color = Color::new(150, 144, 132, 255);

const TITLE_TEXT: &str = "PAUSED";
const TITLE_SCALE: i32 = 4;

/// Etiquetas en el orden visual exacto requerido: `0 -> CONTINUE`,
/// `1 -> EXIT TO MENU`.
const ITEM_LABELS: [&str; 2] = ["CONTINUE", "EXIT TO MENU"];
const CONTINUE_ROW: usize = 0;
const EXIT_TO_MENU_ROW: usize = 1;

const ROW_SCALE: i32 = 3;
const ROW_PADDING: i32 = 10;
const ROW_BORDER_THICKNESS: i32 = 2;
const ROW_SPACING: i32 = 12;

/// Ancho fijo del panel, suficientemente amplio para la etiqueta más
/// larga (`EXIT TO MENU`); verificado por
/// `longest_known_row_text_fits_within_row_width`.
const ROW_WIDTH: i32 = 300;

/// Separación entre el borde exterior del panel y su contenido
/// (título/filas), y entre el título y la primera fila.
const PANEL_PADDING: i32 = 18;
const PANEL_BORDER_THICKNESS: i32 = 3;
const TITLE_TO_ROWS_GAP: i32 = 20;

/// Ancho/alto, en píxeles lógicos (sin escalar), de un glifo.
const GLYPH_WIDTH: i32 = 5;
const GLYPH_HEIGHT: i32 = 7;

/// Separación horizontal, en píxeles lógicos, entre glifos.
const GLYPH_GAP: i32 = 1;

/// Fuente bitmap 5x7 mínima, privada de esta pantalla: solo los
/// glifos que `PAUSED`/`CONTINUE`/`EXIT TO MENU` requieren
/// realmente. Deliberadamente NO comparte implementación con las
/// fuentes de `welcome.rs`, `level_select.rs` ni `victory.rs`
/// (mismo patrón ya establecido entre esas tres); los bitmaps en sí
/// coinciden por ser la misma familia visual 5x7 del proyecto.
fn glyph_rows(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
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
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
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
        'S' => [
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ],
        'T' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ],
        'U' => [
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
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

/// Coordenada que centra un contenido de `content_size` píxeles
/// dentro de un espacio de `container_size` píxeles.
fn centered(container_size: i32, content_size: i32) -> i32 {
    (container_size - content_size) / 2
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

/// Oscurece TODO el framebuffer con un dithering pixelado (tablero
/// de ajedrez de un píxel, alternando el color original con negro
/// opaco), en vez de mezcla alfa/blur moderno.
///
/// `Framebuffer::point` escribe de forma opaca (no mezcla canales
/// alfa) y no expone lectura de píxel — un oscurecimiento
/// "verdadero" por transparencia requeriría una capacidad nueva que
/// esta tarea no necesita. El dithering logra el mismo efecto de
/// legibilidad (el mundo congelado se sigue reconociendo debajo,
/// pero claramente atenuado) con la técnica retro/pixel-art nativa
/// del proyecto, sin tocar `Framebuffer`.
fn draw_dither_overlay(framebuffer: &mut Framebuffer) {
    framebuffer.set_current_color(OVERLAY_COLOR);

    let width = framebuffer.width();

    let height = framebuffer.height();

    for y in 0..height {
        for x in 0..width {
            if (x + y) % 2 == 0 {
                framebuffer.point(x, y);
            }
        }
    }
}

/// Disposición geométrica ya resuelta del panel de pausa para un
/// framebuffer concreto.
///
/// Cálculo puro, sin dependencia de `Framebuffer`: el panel tiene
/// tamaño fijo, para poder probarse sin abrir una ventana.
struct PauseLayout {
    panel_left: i32,
    panel_top: i32,
    panel_right: i32,
    panel_bottom: i32,
    title_x: i32,
    title_y: i32,
    row_x: i32,
    row_width: i32,
    row_height: i32,
    rows_top: i32,
    row_spacing: i32,
}

impl PauseLayout {
    /// Coordenada Y superior de la fila `row_index` (0-based).
    fn row_y(&self, row_index: usize) -> i32 {
        self.rows_top + row_index as i32 * (self.row_height + self.row_spacing)
    }
}

fn compute_layout(framebuffer_width: i32, framebuffer_height: i32) -> PauseLayout {
    let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

    let row_width = ROW_WIDTH;

    let row_height = GLYPH_HEIGHT * ROW_SCALE + 2 * ROW_PADDING;

    let content_width = row_width.max(title_width);

    let title_height = GLYPH_HEIGHT * TITLE_SCALE;

    let rows_total_height = 2 * row_height + ROW_SPACING + title_height + TITLE_TO_ROWS_GAP;

    let panel_width = content_width + 2 * PANEL_PADDING;

    let panel_height = rows_total_height + 2 * PANEL_PADDING;

    let panel_left = centered(framebuffer_width, panel_width);

    let panel_top = centered(framebuffer_height, panel_height);

    let title_x = panel_left + centered(panel_width, title_width);

    let title_y = panel_top + PANEL_PADDING;

    let row_x = panel_left + centered(panel_width, row_width);

    let rows_top = title_y + title_height + TITLE_TO_ROWS_GAP;

    PauseLayout {
        panel_left,
        panel_top,
        panel_right: panel_left + panel_width,
        panel_bottom: panel_top + panel_height,
        title_x,
        title_y,
        row_x,
        row_width,
        row_height,
        rows_top,
        row_spacing: ROW_SPACING,
    }
}

/// Estado de la superposición de pausa: únicamente cuál de las dos
/// opciones está resaltada. Sin Juego de la Vida propio ni ningún
/// otro estado animado — es un overlay, no una pantalla completa.
pub(crate) struct PauseScreen {
    selected: PauseMenuItem,
}

impl PauseScreen {
    pub(crate) fn new() -> Self {
        Self {
            selected: PauseMenuItem::Continue,
        }
    }

    /// Opción actualmente resaltada.
    pub(crate) fn selected_item(&self) -> PauseMenuItem {
        self.selected
    }

    /// Debe llamarse cada vez que `App` entra a `Paused` desde
    /// `Playing`, para que la pausa SIEMPRE se abra con `CONTINUE`
    /// resaltado, sin importar qué opción hubiera quedado
    /// seleccionada la última vez que se cerró el menú.
    pub(crate) fn on_enter(&mut self) {
        self.selected = PauseMenuItem::Continue;
    }

    pub(crate) fn select_previous(&mut self) {
        self.selected = self.selected.toggled();
    }

    pub(crate) fn select_next(&mut self) {
        self.selected = self.selected.toggled();
    }

    /// Fija la selección directamente a `item`, sin pasar por
    /// `toggled()`. Usado exclusivamente por el hover/clic de mouse
    /// (`App::update_paused`): el teclado sigue usando
    /// `select_previous`/`select_next` sin cambios.
    pub(crate) fn set_selected(&mut self, item: PauseMenuItem) {
        self.selected = item;
    }

    /// Elemento, si lo hay, cuya hitbox contiene `(mouse_x, mouse_y)`
    /// (coordenadas lógicas del framebuffer).
    ///
    /// Recalcula el MISMO `compute_layout` que `render` usa para
    /// dibujar las dos filas, para que la hitbox siempre coincida
    /// exactamente con la posición visual actual.
    pub(crate) fn hit_test(
        &self,
        framebuffer_width: i32,
        framebuffer_height: i32,
        mouse_x: i32,
        mouse_y: i32,
    ) -> Option<PauseMenuItem> {
        let layout = compute_layout(framebuffer_width, framebuffer_height);

        for (index, item) in [
            (CONTINUE_ROW, PauseMenuItem::Continue),
            (EXIT_TO_MENU_ROW, PauseMenuItem::ExitToMenu),
        ] {
            let row_y = layout.row_y(index);

            let hitbox = Hitbox {
                x0: layout.row_x,
                y0: row_y,
                x1: layout.row_x + layout.row_width,
                y1: row_y + layout.row_height,
            };

            if hitbox.contains(mouse_x, mouse_y) {
                return Some(item);
            }
        }

        None
    }

    /// Dibuja la superposición completa: oscurece TODO lo ya
    /// dibujado en `framebuffer` (el mundo/HUD/minimapa/arma
    /// congelados que `App::render_paused` ya dibujó antes de
    /// llamar aquí) y encima el panel `PAUSED`/`CONTINUE`/`EXIT TO
    /// MENU`, con la opción resaltada según `selected_item()`.
    ///
    /// Puramente de presentación: no lee entrada, no muta ningún
    /// estado de sesión/jugador/entidad.
    pub(crate) fn render(&self, framebuffer: &mut Framebuffer) {
        draw_dither_overlay(framebuffer);

        let layout = compute_layout(framebuffer.width(), framebuffer.height());

        fill_rect(
            framebuffer,
            layout.panel_left,
            layout.panel_top,
            layout.panel_right,
            layout.panel_bottom,
            PANEL_BORDER_COLOR,
        );

        fill_rect(
            framebuffer,
            layout.panel_left + PANEL_BORDER_THICKNESS,
            layout.panel_top + PANEL_BORDER_THICKNESS,
            layout.panel_right - PANEL_BORDER_THICKNESS,
            layout.panel_bottom - PANEL_BORDER_THICKNESS,
            PANEL_COLOR,
        );

        draw_text(
            framebuffer,
            TITLE_TEXT,
            layout.title_x,
            layout.title_y,
            TITLE_SCALE,
            TITLE_COLOR,
        );

        for (index, label) in ITEM_LABELS.iter().enumerate() {
            let selected = index == CONTINUE_ROW && self.selected == PauseMenuItem::Continue
                || index == EXIT_TO_MENU_ROW && self.selected == PauseMenuItem::ExitToMenu;

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

            let label_width = text_width(label, ROW_SCALE);

            draw_text(
                framebuffer,
                label,
                layout.row_x + centered(layout.row_width, label_width),
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
    fn opens_with_continue_selected() {
        let screen = PauseScreen::new();

        assert_eq!(screen.selected_item(), PauseMenuItem::Continue);
    }

    #[test]
    fn on_enter_always_resets_to_continue() {
        let mut screen = PauseScreen::new();

        screen.select_next();
        assert_eq!(screen.selected_item(), PauseMenuItem::ExitToMenu);

        screen.on_enter();
        assert_eq!(screen.selected_item(), PauseMenuItem::Continue);
    }

    #[test]
    fn down_from_continue_selects_exit_to_menu() {
        let mut screen = PauseScreen::new();

        screen.select_next();

        assert_eq!(screen.selected_item(), PauseMenuItem::ExitToMenu);
    }

    #[test]
    fn up_from_exit_to_menu_selects_continue() {
        let mut screen = PauseScreen::new();

        screen.select_next();
        screen.select_previous();

        assert_eq!(screen.selected_item(), PauseMenuItem::Continue);
    }

    #[test]
    fn navigation_toggles_back_and_forth_indefinitely() {
        let mut screen = PauseScreen::new();

        for expected in [
            PauseMenuItem::ExitToMenu,
            PauseMenuItem::Continue,
            PauseMenuItem::ExitToMenu,
            PauseMenuItem::Continue,
        ] {
            screen.select_next();
            assert_eq!(screen.selected_item(), expected);
        }
    }

    #[test]
    fn up_and_down_are_equivalent_for_a_two_item_menu() {
        let mut up_screen = PauseScreen::new();
        let mut down_screen = PauseScreen::new();

        up_screen.select_previous();
        down_screen.select_next();

        assert_eq!(up_screen.selected_item(), down_screen.selected_item());
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
    fn longest_known_row_text_fits_within_row_width() {
        let longest_label_width = text_width("EXIT TO MENU", ROW_SCALE);

        let content_width = longest_label_width + 2 * ROW_PADDING;

        assert!(content_width <= ROW_WIDTH);
    }

    #[test]
    fn panel_fits_within_reference_framebuffer_bounds() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        assert!(layout.panel_left >= 0);
        assert!(layout.panel_top >= 0);
        assert!(layout.panel_right <= REFERENCE_WIDTH);
        assert!(layout.panel_bottom <= REFERENCE_HEIGHT);
    }

    #[test]
    fn two_rows_do_not_overlap_and_stay_inside_the_panel() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let row0_bottom = layout.row_y(0) + layout.row_height;

        let row1_top = layout.row_y(1);

        let row1_bottom = layout.row_y(1) + layout.row_height;

        assert!(row0_bottom <= row1_top);
        assert!(row1_bottom <= layout.panel_bottom);
    }

    #[test]
    fn hit_test_matches_each_row_and_none_outside_the_panel() {
        let screen = PauseScreen::new();

        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let continue_center_y = layout.row_y(CONTINUE_ROW) + layout.row_height / 2;
        let exit_center_y = layout.row_y(EXIT_TO_MENU_ROW) + layout.row_height / 2;
        let center_x = layout.row_x + layout.row_width / 2;

        assert_eq!(
            screen.hit_test(
                REFERENCE_WIDTH,
                REFERENCE_HEIGHT,
                center_x,
                continue_center_y
            ),
            Some(PauseMenuItem::Continue)
        );

        assert_eq!(
            screen.hit_test(REFERENCE_WIDTH, REFERENCE_HEIGHT, center_x, exit_center_y),
            Some(PauseMenuItem::ExitToMenu)
        );

        assert_eq!(
            screen.hit_test(REFERENCE_WIDTH, REFERENCE_HEIGHT, 0, 0),
            None
        );
    }

    #[test]
    fn set_selected_overrides_the_current_selection_directly() {
        let mut screen = PauseScreen::new();

        screen.set_selected(PauseMenuItem::ExitToMenu);

        assert_eq!(screen.selected_item(), PauseMenuItem::ExitToMenu);
    }

    #[test]
    fn tiny_framebuffer_layout_does_not_panic() {
        let layout = compute_layout(1, 1);

        let _ = layout.title_x;
        let _ = layout.row_x;
    }
}
