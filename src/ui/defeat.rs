use raylib::prelude::Color;

use crate::rendering::framebuffer::Framebuffer;
use crate::rendering::palette_for_theme;
use crate::ui::Hitbox;
use crate::world::LevelTheme;

/// Fondo casi negro, deliberadamente oscuro (Tarea 46): a
/// diferencia de Victoria (`victory.rs`), Derrota NO usa un Juego de
/// la Vida animado — una pantalla simple y estable es justamente lo
/// que esta tarea pide.
const BACKGROUND_COLOR: Color = Color::new(4, 4, 6, 255);

const TITLE_COLOR: Color = Color::new(200, 195, 188, 255);

const UNSELECTED_PANEL_COLOR: Color = Color::new(14, 12, 14, 255);
const UNSELECTED_TEXT_COLOR: Color = Color::new(140, 134, 128, 255);

const SELECTED_PANEL_COLOR: Color = Color::new(26, 12, 14, 255);
const SELECTED_TEXT_COLOR: Color = Color::new(235, 230, 218, 255);

const TITLE_TEXT: &str = "YOU LOST THE HAND";
const TITLE_SCALE: i32 = 3;
const TITLE_TOP_MARGIN: i32 = 48;
const TITLE_TO_ROWS_GAP: i32 = 36;

/// Etiquetas de fila, en el orden visual exacto requerido:
/// `0 -> RETRY`, `1 -> MAIN MENU`. `RETRY` es siempre la selección
/// inicial (Tarea 46, sección 10).
const ROW_LABELS: [&str; 2] = ["RETRY", "MAIN MENU"];

const ROW_SCALE: i32 = 3;
const ROW_PADDING: i32 = 10;
const ROW_BORDER_THICKNESS: i32 = 2;
const ROW_SPACING: i32 = 14;

/// Ancho fijo de cada fila, suficientemente amplio para la etiqueta
/// más larga (`MAIN MENU`); verificado por
/// `longest_known_row_text_fits_within_row_width`.
const ROW_WIDTH: i32 = 260;

/// Ancho/alto, en píxeles lógicos (sin escalar), de un glifo.
const GLYPH_WIDTH: i32 = 5;
const GLYPH_HEIGHT: i32 = 7;

/// Separación horizontal, en píxeles lógicos, entre glifos.
const GLYPH_GAP: i32 = 1;

/// Elemento seleccionable de la pantalla de Derrota (Tarea 46).
///
/// Con solo dos filas, este mismo tipo sirve a la vez de estado de
/// selección (`DefeatScreen::selected`) y de acción semántica que
/// `App` ejecuta — exactamente el mismo patrón que `PauseMenuItem`
/// (dos elementos, alternancia simple) en vez de introducir un
/// `DefeatAction` separado que solo duplicaría la misma información.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefeatMenuItem {
    Retry,
    MainMenu,
}

impl DefeatMenuItem {
    /// El otro elemento — con solo dos filas, "anterior" y
    /// "siguiente" son siempre la misma alternancia.
    fn toggled(self) -> Self {
        match self {
            DefeatMenuItem::Retry => DefeatMenuItem::MainMenu,
            DefeatMenuItem::MainMenu => DefeatMenuItem::Retry,
        }
    }

    fn row_index(self) -> usize {
        match self {
            DefeatMenuItem::Retry => 0,
            DefeatMenuItem::MainMenu => 1,
        }
    }
}

/// Fuente bitmap 5x7 mínima, privada de esta pantalla: solo los
/// glifos que `YOU LOST THE HAND`/`RETRY`/`MAIN MENU` requieren
/// realmente. Deliberadamente NO comparte implementación con las
/// fuentes de `welcome.rs`, `level_select.rs`, `victory.rs`,
/// `pause.rs` ni `rendering/hud.rs` — mismo patrón ya establecido por
/// esas pantallas.
fn glyph_rows(character: char) -> [u8; 7] {
    match character {
        'A' => [
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'D' => [
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ],
        'E' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
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
        'Y' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ],

        // El espacio, y cualquier carácter no soportado por esta
        // fuente mínima, se dibuja en blanco en vez de entrar en
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

/// Disposición geométrica ya resuelta de la pantalla de Derrota para
/// un framebuffer concreto.
///
/// Cálculo puro, sin dependencia de `Framebuffer`/Raylib: las filas
/// tienen tamaño fijo, para poder probarse sin abrir una ventana.
struct DefeatLayout {
    title_x: i32,
    title_y: i32,
    row_x: i32,
    row_width: i32,
    row_height: i32,
    rows_top: i32,
    row_spacing: i32,
}

impl DefeatLayout {
    /// Coordenada Y superior de la fila `row_index` (0-based).
    fn row_y(&self, row_index: usize) -> i32 {
        self.rows_top + row_index as i32 * (self.row_height + self.row_spacing)
    }
}

fn compute_layout(framebuffer_width: i32, framebuffer_height: i32) -> DefeatLayout {
    let _ = framebuffer_height;

    let title_width = text_width(TITLE_TEXT, TITLE_SCALE);

    let title_x = centered_x(framebuffer_width, title_width);

    let title_y = TITLE_TOP_MARGIN;

    let row_width = ROW_WIDTH;

    let row_height = GLYPH_HEIGHT * ROW_SCALE + 2 * ROW_PADDING;

    let row_x = centered_x(framebuffer_width, row_width);

    let rows_top = title_y + GLYPH_HEIGHT * TITLE_SCALE + TITLE_TO_ROWS_GAP;

    DefeatLayout {
        title_x,
        title_y,
        row_x,
        row_width,
        row_height,
        rows_top,
        row_spacing: ROW_SPACING,
    }
}

/// Acento cromático discreto de la fila seleccionada, para el nivel
/// donde murió el jugador (Tarea 46, sección 8).
///
/// Delega ENTERAMENTE en `palette_for_theme` — la MISMA fuente de
/// verdad que T39–T41 — en vez de duplicar ningún literal `#FF7A00`/
/// `#C13CFF` dentro de este módulo. Extraída como función propia
/// (en vez de resolverse inline dentro de `render`) únicamente para
/// poder probarse sin abrir una ventana.
fn accent_for_theme(theme: LevelTheme) -> Color {
    palette_for_theme(theme).accent_bright
}

/// Pantalla de Derrota (Tarea 46): estado/pantalla COMPLETO (a
/// diferencia de `PauseScreen`, que es solo un overlay sobre el
/// mundo congelado) mostrado cuando la vida del jugador llega a
/// `0`.
///
/// No posee `Level`, `GameSession`, `Player` ni `LevelManager`: solo
/// produce un `DefeatMenuItem` seleccionado; `App` es quien decide y
/// ejecuta la operación real (reconstruir la sesión para Retry, o
/// volver a `Welcome` para Main Menu).
pub(crate) struct DefeatScreen {
    selected: DefeatMenuItem,
}

impl DefeatScreen {
    pub(crate) fn new() -> Self {
        Self {
            selected: DefeatMenuItem::Retry,
        }
    }

    /// Elemento actualmente seleccionado.
    pub(crate) fn selected_item(&self) -> DefeatMenuItem {
        self.selected
    }

    /// Debe llamarse cada vez que `App` entra a esta pantalla
    /// (`Playing -> Defeat`): la selección inicial es SIEMPRE
    /// `RETRY` (Tarea 46, sección 10), sin importar cuál fuera la
    /// selección de una Derrota anterior.
    pub(crate) fn on_enter(&mut self) {
        self.selected = DefeatMenuItem::Retry;
    }

    /// Mueve la selección al elemento anterior — con solo dos
    /// filas, equivalente a `select_next`.
    pub(crate) fn select_previous(&mut self) {
        self.selected = self.selected.toggled();
    }

    /// Mueve la selección al siguiente elemento.
    pub(crate) fn select_next(&mut self) {
        self.selected = self.selected.toggled();
    }

    /// Fija la selección directamente a `item`, sin pasar por
    /// `toggled()`. Usado exclusivamente por el hover/clic de mouse
    /// (`App::update_defeat`); el teclado sigue usando
    /// `select_previous`/`select_next` sin cambios.
    pub(crate) fn set_selected(&mut self, item: DefeatMenuItem) {
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
    ) -> Option<DefeatMenuItem> {
        let layout = compute_layout(framebuffer_width, framebuffer_height);

        for item in [DefeatMenuItem::Retry, DefeatMenuItem::MainMenu] {
            let row_y = layout.row_y(item.row_index());

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

    /// Dibuja la pantalla completa: fondo casi negro, título `YOU
    /// LOST THE HAND` y las dos filas `RETRY`/`MAIN MENU`, con un
    /// acento cromático discreto tomado de `theme` (el nivel donde
    /// murió el jugador) — la MISMA fuente de verdad
    /// (`palette_for_theme`) que T39–T41, nunca un literal de color
    /// duplicado aquí.
    pub(crate) fn render(&self, framebuffer: &mut Framebuffer, theme: LevelTheme) {
        let accent = accent_for_theme(theme);

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

        let layout = compute_layout(framebuffer_width, framebuffer_height);

        draw_text(
            framebuffer,
            TITLE_TEXT,
            layout.title_x,
            layout.title_y,
            TITLE_SCALE,
            TITLE_COLOR,
        );

        for (index, label) in ROW_LABELS.iter().enumerate() {
            let selected = index == self.selected.row_index();

            let (panel_color, border_color, text_color) = if selected {
                (SELECTED_PANEL_COLOR, accent, SELECTED_TEXT_COLOR)
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
    fn new_screen_selects_retry() {
        let screen = DefeatScreen::new();

        assert_eq!(screen.selected_item(), DefeatMenuItem::Retry);
    }

    #[test]
    fn on_enter_always_resets_to_retry() {
        let mut screen = DefeatScreen::new();

        screen.select_next();
        assert_eq!(screen.selected_item(), DefeatMenuItem::MainMenu);

        screen.on_enter();
        assert_eq!(screen.selected_item(), DefeatMenuItem::Retry);
    }

    #[test]
    fn navigation_toggles_between_the_two_items() {
        let mut screen = DefeatScreen::new();

        screen.select_next();
        assert_eq!(screen.selected_item(), DefeatMenuItem::MainMenu);

        screen.select_next();
        assert_eq!(screen.selected_item(), DefeatMenuItem::Retry);

        screen.select_previous();
        assert_eq!(screen.selected_item(), DefeatMenuItem::MainMenu);

        screen.select_previous();
        assert_eq!(screen.selected_item(), DefeatMenuItem::Retry);
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
    fn hit_test_matches_each_row_and_none_outside_the_rows() {
        let screen = DefeatScreen::new();

        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        let center_x = layout.row_x + layout.row_width / 2;

        let retry_center_y =
            layout.row_y(DefeatMenuItem::Retry.row_index()) + layout.row_height / 2;
        let main_menu_center_y =
            layout.row_y(DefeatMenuItem::MainMenu.row_index()) + layout.row_height / 2;

        assert_eq!(
            screen.hit_test(REFERENCE_WIDTH, REFERENCE_HEIGHT, center_x, retry_center_y),
            Some(DefeatMenuItem::Retry)
        );

        assert_eq!(
            screen.hit_test(
                REFERENCE_WIDTH,
                REFERENCE_HEIGHT,
                center_x,
                main_menu_center_y
            ),
            Some(DefeatMenuItem::MainMenu)
        );

        assert_eq!(
            screen.hit_test(REFERENCE_WIDTH, REFERENCE_HEIGHT, 0, 0),
            None
        );
    }

    #[test]
    fn set_selected_overrides_the_current_selection_directly() {
        let mut screen = DefeatScreen::new();

        screen.set_selected(DefeatMenuItem::MainMenu);

        assert_eq!(screen.selected_item(), DefeatMenuItem::MainMenu);
    }

    #[test]
    fn two_rows_fit_and_do_not_overlap() {
        let layout = compute_layout(REFERENCE_WIDTH, REFERENCE_HEIGHT);

        assert!(layout.row_x >= 0);
        assert!(layout.row_x + layout.row_width <= REFERENCE_WIDTH);

        for index in 0..2usize {
            let row_top = layout.row_y(index);

            assert!(row_top >= 0);
            assert!(row_top + layout.row_height <= REFERENCE_HEIGHT);
        }

        let row0_bottom = layout.row_y(0) + layout.row_height;
        let row1_top = layout.row_y(1);

        assert!(row0_bottom <= row1_top);
    }

    #[test]
    fn longest_known_row_text_fits_within_row_width() {
        let longest_label_width = text_width("MAIN MENU", ROW_SCALE);

        let content_width = longest_label_width + 2 * ROW_PADDING;

        assert!(content_width <= ROW_WIDTH);
    }

    #[test]
    fn defeat_accent_matches_the_canonical_theme_palette() {
        // Compara contra la MISMA fuente de verdad (`palette_for_theme`)
        // en vez de copiar literales RGB en el test: Crimson Entrance
        // sigue rojo, Black Club sigue naranja, House of Cards sigue
        // violeta (Tarea 46, sección 8/39).
        assert_eq!(
            accent_for_theme(LevelTheme::CrimsonEntrance),
            palette_for_theme(LevelTheme::CrimsonEntrance).accent_bright
        );
        assert_eq!(
            accent_for_theme(LevelTheme::BlackClub),
            palette_for_theme(LevelTheme::BlackClub).accent_bright
        );
        assert_eq!(
            accent_for_theme(LevelTheme::HouseOfCards),
            palette_for_theme(LevelTheme::HouseOfCards).accent_bright
        );

        // Los tres acentos deben ser distintos entre sí: la
        // identidad cromática de cada nivel sigue siendo discriminable.
        let crimson = accent_for_theme(LevelTheme::CrimsonEntrance);
        let black_club = accent_for_theme(LevelTheme::BlackClub);
        let house = accent_for_theme(LevelTheme::HouseOfCards);

        assert_ne!(crimson, black_club);
        assert_ne!(crimson, house);
        assert_ne!(black_club, house);
    }

    #[test]
    fn tiny_framebuffer_layout_does_not_panic() {
        let layout = compute_layout(1, 1);

        let _ = layout.title_x;
        let _ = layout.row_x;
    }
}
