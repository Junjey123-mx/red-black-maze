use raylib::prelude::Color;

use super::framebuffer::Framebuffer;
use super::palette::palette_for_theme;
use crate::world::LevelTheme;

/// Margen izquierdo entre el borde del framebuffer y el HUD.
const LEFT_MARGIN: i32 = 16;

/// Margen inferior entre el borde del framebuffer y el HUD.
const BOTTOM_MARGIN: i32 = 16;

/// Escala entera de dibujo de cada píxel lógico de glifo.
const GLYPH_SCALE: i32 = 3;

/// Ancho/alto, en píxeles lógicos (sin escalar), de un dígito o de
/// un ícono.
const GLYPH_WIDTH: i32 = 5;
const GLYPH_HEIGHT: i32 = 5;

/// Separación horizontal, en píxeles lógicos, entre glifos
/// consecutivos dentro del mismo grupo (ícono->dígitos,
/// dígito->dígito).
const GLYPH_GAP: i32 = 1;

/// Separación horizontal, en píxeles lógicos, entre el grupo de
/// vida y el grupo de munición.
const GROUP_GAP: i32 = 4;

/// Cantidad mínima de dígitos con los que se representa la
/// munición (rellenada con ceros a la izquierda).
const AMMO_MIN_DIGITS: usize = 2;

/// Cantidad mínima de dígitos con los que se representa la vida
/// (sin relleno forzado: un solo dígito es válido).
const HEALTH_MIN_DIGITS: usize = 1;

/// Cantidad mínima de dígitos con los que se representa el contador
/// de FPS (sin relleno forzado: valores de un dígito son válidos).
const FPS_MIN_DIGITS: usize = 1;

/// Margen entre el borde del framebuffer y el contador de FPS,
/// anclado arriba-izquierda (deliberadamente distinto del HUD de
/// vida/munición, anclado abajo-izquierda: no se solapan).
const FPS_LEFT_MARGIN: i32 = 8;
const FPS_TOP_MARGIN: i32 = 8;

/// Marfil/crema neutro, independiente del `LevelTheme` activo (no
/// forma parte de la familia de acento): los dígitos de vida/
/// munición/FPS siguen siendo marfil sin importar si el nivel es
/// rojo/naranja/violeta.
const HUD_IVORY: Color = Color::new(214, 208, 196, 255);

/// Fuente bitmap 3x5 (ancho x alto) para los dígitos `0`-`9`.
///
/// Cada fila es una máscara de bits de 3 columnas (bit más
/// significativo = columna izquierda). Es la única tabla de glifos
/// que Tarea 26 necesita: no se implementan letras ni un motor de
/// texto genérico.
const DIGIT_WIDTH: i32 = 3;
const DIGIT_FONT: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

/// Fuente bitmap 5x5 del ícono de corazón (vida).
const HEART_ICON: [u8; 5] = [0b01010, 0b11111, 0b11111, 0b01110, 0b00100];

/// Fuente bitmap 5x5 del ícono de diamante de naipe (munición).
const DIAMOND_ICON: [u8; 5] = [0b00100, 0b01110, 0b11111, 0b01110, 0b00100];

/// Ancho lógico del glifo "/" que separa cargador de reserva.
const SLASH_WIDTH: i32 = 3;

/// Glifo bitmap 3x5 de "/", en la misma fuente/estilo que
/// `DIGIT_FONT`: reutiliza `draw_glyph`, no carga ninguna textura de
/// fuente nueva.
const SLASH_GLYPH: [u8; 5] = [0b001, 0b001, 0b010, 0b100, 0b100];

/// Descompone `value` en sus dígitos decimales, rellenando con
/// ceros a la izquierda hasta alcanzar `min_digits`.
///
/// Función pura, sin asignación más allá del `Vec` de dígitos
/// resultante; comprobable sin abrir Raylib.
fn digits_of(value: i64, min_digits: usize) -> Vec<u8> {
    let magnitude = value.max(0);

    let mut digits = Vec::new();

    if magnitude == 0 {
        digits.push(0);
    } else {
        let mut remaining = magnitude;

        while remaining > 0 {
            digits.push((remaining % 10) as u8);

            remaining /= 10;
        }

        digits.reverse();
    }

    while digits.len() < min_digits {
        digits.insert(0, 0);
    }

    digits
}

/// Dibuja un glifo de `width x height` píxeles lógicos, descrito
/// por `rows` (una máscara de bits por fila, bit más significativo
/// = columna izquierda), escalado por `GLYPH_SCALE` y anclado en
/// `(origin_x, origin_y)`.
fn draw_glyph(
    framebuffer: &mut Framebuffer,
    rows: &[u8],
    width: i32,
    origin_x: i32,
    origin_y: i32,
    color: Color,
) {
    framebuffer.set_current_color(color);

    for (row_index, row_bits) in rows.iter().enumerate() {
        for column in 0..width {
            let bit_position = width - 1 - column;

            if (row_bits >> bit_position) & 1 == 0 {
                continue;
            }

            let pixel_x = origin_x + column * GLYPH_SCALE;

            let pixel_y = origin_y + row_index as i32 * GLYPH_SCALE;

            for offset_y in 0..GLYPH_SCALE {
                for offset_x in 0..GLYPH_SCALE {
                    framebuffer.point(pixel_x + offset_x, pixel_y + offset_y);
                }
            }
        }
    }
}

/// Dibuja una secuencia de dígitos decimales una junto a otra,
/// comenzando en `(origin_x, origin_y)`.
///
/// Retorna la coordenada X lógica inmediatamente después del último
/// dígito dibujado (sin escalar por `GLYPH_SCALE`), útil para
/// encadenar el siguiente grupo del HUD.
fn draw_digits(
    framebuffer: &mut Framebuffer,
    digits: &[u8],
    origin_x: i32,
    origin_y: i32,
    color: Color,
) -> i32 {
    let mut cursor_x = origin_x;

    for &digit in digits {
        let rows = &DIGIT_FONT[digit as usize % 10];

        draw_glyph(framebuffer, rows, DIGIT_WIDTH, cursor_x, origin_y, color);

        cursor_x += (DIGIT_WIDTH + GLYPH_GAP) * GLYPH_SCALE;
    }

    cursor_x
}

/// Dibuja la superposición de estado del jugador (HUD): corazón +
/// vida a la izquierda, diamante + munición a continuación, anclada
/// abajo-izquierda sobre la vista `World3D`.
///
/// Es puramente presentación: recibe instantáneas primitivas
/// (`health`, `magazine_ammo`, `reserve_ammo`) ya leídas por el
/// llamador; no posee ni modifica `Player`, `Weapon` ni
/// `GameSession`, no lee entrada, y no dispara ni cura ni recarga.
/// Estos valores son los ÚNICOS que determinan lo que se dibuja: no
/// existe ningún literal `100`/`06`/`18` de visualización en este
/// módulo, solo constantes de geometría de glifo y la vida/munición
/// inicial del dominio (que viven en `player.rs`/`weapon.rs`, no
/// aquí).
pub(crate) fn render_hud(
    framebuffer: &mut Framebuffer,
    health: i32,
    magazine_ammo: u32,
    reserve_ammo: u32,
    theme: LevelTheme,
) {
    let framebuffer_width = framebuffer.width();

    let framebuffer_height = framebuffer.height();

    if framebuffer_width <= 0 || framebuffer_height <= 0 {
        return;
    }

    /*
     * Resuelto una única vez por cuadro (no por glifo): el `match`
     * de `palette_for_theme` es una construcción de struct en pila,
     * sin asignación ni E/S, por lo que no hay costo real en
     * resolverlo aquí en vez de recibir un `&ThemePalette` ya
     * calculado por el llamador.
     */
    let palette = palette_for_theme(theme);

    let row_height = GLYPH_HEIGHT * GLYPH_SCALE;

    let origin_x = LEFT_MARGIN;

    let origin_y = framebuffer_height - BOTTOM_MARGIN - row_height;

    draw_glyph(
        framebuffer,
        &HEART_ICON,
        GLYPH_WIDTH,
        origin_x,
        origin_y,
        palette.hud_accent,
    );

    let health_digits_x = origin_x + (GLYPH_WIDTH + GLYPH_GAP) * GLYPH_SCALE;

    let health_digits = digits_of(health as i64, HEALTH_MIN_DIGITS);

    let after_health_x = draw_digits(
        framebuffer,
        &health_digits,
        health_digits_x,
        origin_y,
        HUD_IVORY,
    );

    let diamond_x = after_health_x + GROUP_GAP * GLYPH_SCALE;

    draw_glyph(
        framebuffer,
        &DIAMOND_ICON,
        GLYPH_WIDTH,
        diamond_x,
        origin_y,
        palette.hud_accent,
    );

    let magazine_digits_x = diamond_x + (GLYPH_WIDTH + GLYPH_GAP) * GLYPH_SCALE;

    let magazine_digits = digits_of(magazine_ammo as i64, AMMO_MIN_DIGITS);

    let after_magazine_x = draw_digits(
        framebuffer,
        &magazine_digits,
        magazine_digits_x,
        origin_y,
        HUD_IVORY,
    );

    let slash_x = after_magazine_x + GLYPH_GAP * GLYPH_SCALE;

    draw_glyph(
        framebuffer,
        &SLASH_GLYPH,
        SLASH_WIDTH,
        slash_x,
        origin_y,
        HUD_IVORY,
    );

    let reserve_digits_x = slash_x + (SLASH_WIDTH + GLYPH_GAP) * GLYPH_SCALE;

    let reserve_digits = digits_of(reserve_ammo as i64, AMMO_MIN_DIGITS);

    draw_digits(
        framebuffer,
        &reserve_digits,
        reserve_digits_x,
        origin_y,
        HUD_IVORY,
    );
}

/// Dibuja el contador de FPS en tiempo real, anclado arriba-
/// izquierda sobre la vista activa (World3D o Map2D).
///
/// Reutiliza la MISMA fuente bitmap de dígitos que `render_hud`
/// (`digits_of`/`draw_digits`/`DIGIT_FONT`): no existe un segundo
/// renderer de texto para esto. Recibe `fps` ya leído por el
/// llamador (`App::update`, vía `RaylibHandle::get_fps`); este
/// módulo no conoce `RaylibHandle` y no decide CÓMO se midió el
/// valor, solo lo dibuja. No asigna ningún `String`/`format!`: los
/// dígitos se descomponen numéricamente y se dibujan directamente
/// como píxeles, igual que `render_hud`.
pub(crate) fn render_fps(framebuffer: &mut Framebuffer, fps: u32) {
    let digits = digits_of(fps as i64, FPS_MIN_DIGITS);

    draw_digits(
        framebuffer,
        &digits,
        FPS_LEFT_MARGIN,
        FPS_TOP_MARGIN,
        HUD_IVORY,
    );
}

// --- Mensajes del sistema de Hands (Dealer Hands) ---
//
// "THE HOUSE IS RELOADING...", "NEXT HAND IN 3/2/1...", "HAND N".
// Escala/anclaje/paleta propios de este mensaje, deliberadamente
// separados de `render_hud`/`render_fps` (misma filosofía de
// composición que ya existe entre ambos).

/// Ancho/alto, en píxeles lógicos, de un glifo de LETRA (distinto del
/// glifo de DÍGITO de `DIGIT_FONT`, que sigue siendo 3 de ancho): un
/// mensaje puede mezclar letras y dígitos en la misma línea (p. ej.
/// "NEXT HAND IN 3..."), cada uno con su propio ancho.
const LETTER_WIDTH: i32 = 5;

/// Escala de dibujo de los mensajes de Hand — mayor que `GLYPH_SCALE`
/// (vida/munición/FPS) porque este texto debe leerse con claridad
/// desde el centro de la pantalla durante el gameplay activo.
const HAND_MESSAGE_SCALE: i32 = 3;

/// Margen superior del mensaje de Hand, anclado arriba-centro: no
/// compite con el HUD de vida/munición (abajo-izquierda) ni con el
/// contador de FPS (arriba-izquierda).
const HAND_MESSAGE_TOP_MARGIN: i32 = 40;

/// Marfil cálido, ligeramente dorado — distinto del `HUD_IVORY`
/// neutro de vida/munición, para que el mensaje de la casa se
/// perciba como un acento propio sin depender del `LevelTheme`
/// activo (aparece en los cuatro niveles por igual).
const HAND_MESSAGE_COLOR: Color = Color::new(224, 196, 120, 255);

/// Fuente bitmap 5x7 mínima, privada de este módulo: solo las letras
/// que los mensajes de Hand requieren realmente (T,H,E,O,U,S,I,R,L,
/// D,A,N,X,V,G) más un punto. Mismo patrón ya establecido por
/// `welcome.rs`/`level_select.rs`/`victory.rs`/`pause.rs`/
/// `defeat.rs`: cada pantalla posee su propia fuente mínima privada,
/// deliberadamente sin compartir implementación entre ellas. Los
/// dígitos NO se redefinen aquí: `draw_hand_message` reutiliza
/// `DIGIT_FONT` (3 de ancho) ya existente para cualquier carácter
/// `'0'..='9'` dentro del mismo mensaje.
fn letter_glyph_rows(character: char) -> [u8; 7] {
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
        'G' => [
            0b01111, 0b10000, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111,
        ],
        'H' => [
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ],
        'I' => [
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b11111,
        ],

        // 'K' añadido para la barra de vida de The King (Bloque 3,
        // Commit 25) — mismo bitmap 5x7 que el resto de esta fuente.
        'K' => [
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ],
        'L' => [
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ],

        // 'M'/':' añadidos para el HUD de progreso de Horde ("HAND
        // N/M", "ENEMIES: K") — mismo bitmap 5x7 que el resto de esta
        // fuente mínima local.
        'M' => [
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ],
        ':' => [0, 0b00100, 0, 0, 0b00100, 0, 0],
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
        'X' => [
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ],
        '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],

        // 'C'/'F'/'J'/'!' añadidos para los avisos de invocación de
        // The King (Bloque 5, Commits 54/55: "THE KING CALLS HIS
        // HAND!", "5 DEALERS JOIN THE HAND") — mismo bitmap 5x7 que
        // el resto de esta fuente mínima local.
        'C' => [
            0b01111, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b01111,
        ],
        'F' => [
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ],
        'J' => [
            0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100,
        ],
        '!' => [
            0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100,
        ],

        // El espacio, y cualquier carácter no soportado por esta
        // fuente mínima, se dibuja en blanco en vez de entrar en
        // pánico o intentar una fuente completa.
        _ => [0; 7],
    }
}

/// Ancho total, en píxeles ya escalados, que ocupará `text` al
/// dibujarse con `draw_hand_message`: cada dígito ASCII pesa
/// `DIGIT_WIDTH`, cualquier otro carácter (letra, punto, espacio)
/// pesa `LETTER_WIDTH`.
fn hand_message_width(text: &str, scale: i32) -> i32 {
    let mut width = 0;

    for character in text.chars() {
        let glyph_width = if character.is_ascii_digit() {
            DIGIT_WIDTH
        } else {
            LETTER_WIDTH
        };

        width += (glyph_width + GLYPH_GAP) * scale;
    }

    if width > 0 {
        width -= GLYPH_GAP * scale;
    }

    width
}

/// Dibuja `text` de izquierda a derecha comenzando en `(origin_x,
/// origin_y)`, mezclando `DIGIT_FONT` (3 de ancho) para cualquier
/// carácter `'0'..='9'` con la fuente de letras local (`LETTER_WIDTH`
/// de ancho) para el resto — la MISMA regla que `hand_message_width`
/// ya usa para medir. Único punto de este bucle de dibujo: tanto
/// `render_hand_message` como `render_horde_progress` lo reutilizan
/// en vez de reimplementarlo cada una con su propio anclaje.
fn draw_mixed_text(
    framebuffer: &mut Framebuffer,
    text: &str,
    origin_x: i32,
    origin_y: i32,
    scale: i32,
    color: Color,
) {
    let mut cursor_x = origin_x;

    for character in text.chars() {
        if character.is_ascii_digit() {
            let digit = character.to_digit(10).unwrap_or(0) as u8;

            draw_glyph(
                framebuffer,
                &DIGIT_FONT[digit as usize % 10],
                DIGIT_WIDTH,
                cursor_x,
                origin_y,
                color,
            );

            cursor_x += (DIGIT_WIDTH + GLYPH_GAP) * scale;
        } else {
            let rows = letter_glyph_rows(character);

            draw_glyph(framebuffer, &rows, LETTER_WIDTH, cursor_x, origin_y, color);

            cursor_x += (LETTER_WIDTH + GLYPH_GAP) * scale;
        }
    }
}

/// Dibuja un mensaje del sistema de Hands, centrado horizontalmente,
/// anclado cerca de la parte superior del framebuffer.
///
/// Presentación pura: recibe el texto ya resuelto por el llamador
/// (`App`, a partir de `game::hand::HandHudMessage`) — este módulo no
/// conoce `GameSession`/`HandState`, solo dibuja la cadena que se le
/// da, mezclando la fuente de letras local con `DIGIT_FONT` ya
/// existente para cualquier dígito embebido (p. ej. "NEXT HAND IN
/// 3...").
pub(crate) fn render_hand_message(framebuffer: &mut Framebuffer, text: &str) {
    let framebuffer_width = framebuffer.width();

    if framebuffer_width <= 0 || text.is_empty() {
        return;
    }

    let content_width = hand_message_width(text, HAND_MESSAGE_SCALE);

    let cursor_x = (framebuffer_width - content_width) / 2;

    draw_mixed_text(
        framebuffer,
        text,
        cursor_x,
        HAND_MESSAGE_TOP_MARGIN,
        HAND_MESSAGE_SCALE,
        HAND_MESSAGE_COLOR,
    );
}

/// Escala del aviso de The King. DEBE ser `GLYPH_SCALE`: `draw_glyph`
/// pinta siempre a `GLYPH_SCALE` px por píxel lógico, mientras que el
/// avance del cursor en `draw_mixed_text` usa la escala pasada — con
/// cualquier otro valor las letras se solapan y el texto queda
/// ilegible.
const KING_SUMMON_WARNING_SCALE: i32 = GLYPH_SCALE;

/// Ancla vertical de la primera línea del aviso: por debajo de la
/// barra de vida de The King (arriba-centro) y por encima del arma/
/// HUD, sin tapar ninguno.
const KING_SUMMON_WARNING_TOP: i32 = 116;

/// Separación vertical (píxeles lógicos, antes de escalar) entre las
/// dos líneas del aviso.
const KING_SUMMON_WARNING_LINE_GAP: i32 = 3;

/// Alto lógico de un glifo de esta fuente (`letter_glyph_rows`
/// devuelve 7 filas).
const KING_SUMMON_WARNING_LINE_HEIGHT: i32 = 7;

/// Dibuja un aviso de The King de dos líneas, centrado
/// horizontalmente y anclado en el tercio superior de la pantalla,
/// con la misma fuente/escala VGA que el resto del HUD.
///
/// Presentación pura: `App` decide CUÁNDO mostrarlo y con QUÉ textos
/// — durante la invocación ("THE KING CALLS HIS HAND!" / "5 DEALERS
/// JOIN THE HAND"), o durante el gate entre cohortes ("THE KING IS
/// SHIELDED" / "CLEAR HIS DEALERS FIRST", como aviso temporizado).
/// No es un modal y no bloquea nada.
pub(crate) fn render_king_summon_warning(
    framebuffer: &mut Framebuffer,
    line_one: &str,
    line_two: &str,
) {
    let framebuffer_width = framebuffer.width();

    if framebuffer_width <= 0 {
        return;
    }

    let line_step = (KING_SUMMON_WARNING_LINE_HEIGHT + KING_SUMMON_WARNING_LINE_GAP)
        * KING_SUMMON_WARNING_SCALE;

    for (index, text) in [line_one, line_two].into_iter().enumerate() {
        if text.is_empty() {
            continue;
        }

        let content_width = hand_message_width(text, KING_SUMMON_WARNING_SCALE);
        let cursor_x = (framebuffer_width - content_width) / 2;
        let origin_y = KING_SUMMON_WARNING_TOP + index as i32 * line_step;

        draw_mixed_text(
            framebuffer,
            text,
            cursor_x,
            origin_y,
            KING_SUMMON_WARNING_SCALE,
            HAND_MESSAGE_COLOR,
        );
    }
}

// --- HUD de progreso de Horde (Bloque 1, Commit 09) ---
//
// "HAND N/M", "ENEMIES: K" — anclado abajo-derecha, simétrico al HUD
// de vida/munición (abajo-izquierda) de `render_hud`, para que ambos
// bloques nunca se solapen. Reutiliza la MISMA fuente de letras/
// dígitos que `render_hand_message`, nunca una fuente nueva.

/// Escala de las dos líneas de progreso de Horde: igual a
/// `GLYPH_SCALE` (la del HUD de vida/munición), para que ambos
/// bloques de HUD compartan la misma escala visual.
const HORDE_PROGRESS_SCALE: i32 = GLYPH_SCALE;

/// Alto lógico (sin escalar) de un glifo de LETRA/DÍGITO dentro de
/// este bloque — mismo valor que ya usa `letter_glyph_rows`/
/// `DIGIT_FONT` (ambos 7 y 5 filas respectivamente, pero `draw_glyph`
/// ya itera por `rows.len()` real; este valor es solo para el
/// ESPACIADO vertical entre líneas, que usa el más alto de los dos).
const HORDE_PROGRESS_LINE_HEIGHT: i32 = 7;

/// Separación vertical entre la línea "HAND N/M" y "ENEMIES: K".
const HORDE_PROGRESS_LINE_GAP: i32 = 4;

/// Margen derecho/inferior — mismos valores que `LEFT_MARGIN`/
/// `BOTTOM_MARGIN` del HUD de vida/munición, reflejados al otro lado.
const HORDE_PROGRESS_RIGHT_MARGIN: i32 = LEFT_MARGIN;
const HORDE_PROGRESS_BOTTOM_MARGIN: i32 = BOTTOM_MARGIN;

/// Dibuja el HUD de progreso de Horde Mode: "HAND N/M" arriba,
/// "ENEMIES: K" debajo, ambas líneas alineadas a la derecha.
///
/// Presentación pura: recibe instantáneas primitivas ya resueltas por
/// el llamador (`App`, a partir de `GameSession`/`LevelManager`) — no
/// conoce `GameMode`/`HordeManager`; es responsabilidad de `App`
/// llamar aquí SOLO durante Horde Mode (Portal Mode simplemente nunca
/// invoca esta función, igual que `App::update_playing` nunca invoca
/// `GameSession::update_hand_state` en Portal).
///
/// `last_normal_hand` es la última Hand que SÍ trae Dealers (la
/// anterior a la ronda final reservada) — nunca `final_hand_number`
/// en sí, que todavía no representa una Hand jugable (Bloque 3 la
/// reemplazará por The King).
pub(crate) fn render_horde_progress(
    framebuffer: &mut Framebuffer,
    hand_number: usize,
    last_normal_hand: usize,
    alive_dealer_count: usize,
) {
    let framebuffer_width = framebuffer.width();

    let framebuffer_height = framebuffer.height();

    if framebuffer_width <= 0 || framebuffer_height <= 0 {
        return;
    }

    let hand_digits_a = digits_of(hand_number as i64, 1);
    let hand_digits_b = digits_of(last_normal_hand as i64, 1);
    let enemy_digits = digits_of(alive_dealer_count as i64, 1);

    let mut hand_line = String::new();
    hand_line.push_str("HAND ");
    for digit in &hand_digits_a {
        hand_line.push((b'0' + digit) as char);
    }
    hand_line.push('/');
    for digit in &hand_digits_b {
        hand_line.push((b'0' + digit) as char);
    }

    let mut enemies_line = String::new();
    enemies_line.push_str("ENEMIES: ");
    for digit in &enemy_digits {
        enemies_line.push((b'0' + digit) as char);
    }

    let hand_line_width = hand_message_width(&hand_line, HORDE_PROGRESS_SCALE);
    let enemies_line_width = hand_message_width(&enemies_line, HORDE_PROGRESS_SCALE);

    let row_height = HORDE_PROGRESS_LINE_HEIGHT * HORDE_PROGRESS_SCALE;

    let enemies_line_y = framebuffer_height - HORDE_PROGRESS_BOTTOM_MARGIN - row_height;

    let hand_line_y = enemies_line_y - HORDE_PROGRESS_LINE_GAP - row_height;

    draw_mixed_text(
        framebuffer,
        &hand_line,
        framebuffer_width - HORDE_PROGRESS_RIGHT_MARGIN - hand_line_width,
        hand_line_y,
        HORDE_PROGRESS_SCALE,
        HAND_MESSAGE_COLOR,
    );

    draw_mixed_text(
        framebuffer,
        &enemies_line,
        framebuffer_width - HORDE_PROGRESS_RIGHT_MARGIN - enemies_line_width,
        enemies_line_y,
        HORDE_PROGRESS_SCALE,
        HAND_MESSAGE_COLOR,
    );
}

/// Escala de la etiqueta "THE KING" de la barra de vida del jefe —
/// la misma que los mensajes de Hand, para que se lea con claridad
/// desde el centro de la pantalla durante el combate final.
const KING_BAR_LABEL_SCALE: i32 = HAND_MESSAGE_SCALE;

/// Margen superior de la etiqueta "THE KING", anclada arriba-centro.
const KING_BAR_TOP_MARGIN: i32 = 20;

/// Separación vertical (píxeles ya escalados) entre la etiqueta y la
/// barra en sí.
const KING_BAR_LABEL_GAP: i32 = 6;

/// Alto lógico (sin escalar) de la barra de vida del jefe.
const KING_BAR_HEIGHT: i32 = 6;

/// Escala de dibujo de la barra (cada píxel lógico -> este cuadrado).
const KING_BAR_SCALE: i32 = 3;

/// Fracción del ancho del framebuffer que ocupa la barra completa.
const KING_BAR_WIDTH_FRACTION: i32 = 2; // ancho = framebuffer_width / 2

/// Grosor (píxeles ya escalados) del marco marfil de la barra.
const KING_BAR_BORDER: i32 = KING_BAR_SCALE;

/// Rojo intenso del relleno de vida del jefe (identidad de The King,
/// independiente del `LevelTheme`).
const KING_BAR_FILL: Color = Color::new(184, 26, 34, 255);

/// Fondo del tramo ya perdido de la barra.
const KING_BAR_EMPTY: Color = Color::new(44, 20, 22, 255);

/// Dibuja la barra de vida de The King: la etiqueta "THE KING"
/// centrada arriba y, justo debajo, una barra cuyo relleno rojo
/// representa `current_health / max_health`.
///
/// Presentación pura: recibe la vida ya leída por el llamador
/// (`App`, a partir de `GameSession::king_health`) — no conoce
/// `Entity`/`GameMode`. Es responsabilidad de `App` invocarla SOLO
/// durante el combate contra el jefe en Horde Mode (nunca en Portal,
/// nunca sin un King vivo), igual que el resto del HUD de Horde.
///
/// No sustituye el HUD de vida del jugador (abajo-izquierda): vive
/// arriba-centro, en su propia franja.
pub(crate) fn render_king_health_bar(
    framebuffer: &mut Framebuffer,
    current_health: i32,
    max_health: i32,
) {
    let framebuffer_width = framebuffer.width();
    let framebuffer_height = framebuffer.height();

    if framebuffer_width <= 0 || framebuffer_height <= 0 || max_health <= 0 {
        return;
    }

    // --- Etiqueta "THE KING", centrada arriba. ---
    let label = "THE KING";
    let label_width = hand_message_width(label, KING_BAR_LABEL_SCALE);
    let label_x = (framebuffer_width - label_width) / 2;

    draw_mixed_text(
        framebuffer,
        label,
        label_x,
        KING_BAR_TOP_MARGIN,
        KING_BAR_LABEL_SCALE,
        HAND_MESSAGE_COLOR,
    );

    // --- Barra. ---
    let label_row_height = 7 * KING_BAR_LABEL_SCALE;
    let bar_top = KING_BAR_TOP_MARGIN + label_row_height + KING_BAR_LABEL_GAP;

    let bar_width = (framebuffer_width / KING_BAR_WIDTH_FRACTION).max(2 * KING_BAR_BORDER + 2);
    let bar_height = KING_BAR_HEIGHT * KING_BAR_SCALE;
    let bar_left = (framebuffer_width - bar_width) / 2;

    let fill_fraction = (current_health.max(0) as f32 / max_health as f32).clamp(0.0, 1.0);

    let inner_left = bar_left + KING_BAR_BORDER;
    let inner_top = bar_top + KING_BAR_BORDER;
    let inner_width = (bar_width - 2 * KING_BAR_BORDER).max(0);
    let inner_height = (bar_height - 2 * KING_BAR_BORDER).max(0);

    let filled_width = (inner_width as f32 * fill_fraction).round() as i32;

    for y in bar_top..bar_top + bar_height {
        if y < 0 || y >= framebuffer_height {
            continue;
        }

        for x in bar_left..bar_left + bar_width {
            if x < 0 || x >= framebuffer_width {
                continue;
            }

            let in_border = y < inner_top
                || y >= inner_top + inner_height
                || x < inner_left
                || x >= inner_left + inner_width;

            let color = if in_border {
                HUD_IVORY
            } else if x - inner_left < filled_width {
                KING_BAR_FILL
            } else {
                KING_BAR_EMPTY
            };

            framebuffer.set_current_color(color);
            framebuffer.point(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ammo_is_zero_padded_to_two_digits() {
        assert_eq!(digits_of(6, AMMO_MIN_DIGITS), vec![0, 6]);
        assert_eq!(digits_of(0, AMMO_MIN_DIGITS), vec![0, 0]);
        assert_eq!(digits_of(10, AMMO_MIN_DIGITS), vec![1, 0]);
    }

    #[test]
    fn health_is_not_forced_to_two_digits() {
        assert_eq!(digits_of(100, HEALTH_MIN_DIGITS), vec![1, 0, 0]);
        assert_eq!(digits_of(0, HEALTH_MIN_DIGITS), vec![0]);
        assert_eq!(digits_of(9, HEALTH_MIN_DIGITS), vec![9]);
    }

    #[test]
    fn negative_values_are_treated_as_zero() {
        assert_eq!(digits_of(-5, HEALTH_MIN_DIGITS), vec![0]);
        assert_eq!(digits_of(-5, AMMO_MIN_DIGITS), vec![0, 0]);
    }

    #[test]
    fn values_above_ninety_nine_grow_naturally() {
        assert_eq!(digits_of(123, AMMO_MIN_DIGITS), vec![1, 2, 3]);
    }

    // --- Bloque 3, Commit 25: barra de vida de The King. ---

    #[test]
    fn the_king_letters_all_resolve_to_a_glyph() {
        for character in "THE KING".chars() {
            if character == ' ' {
                continue;
            }
            assert_ne!(
                letter_glyph_rows(character),
                [0u8; 7],
                "falta el glifo para '{character}'"
            );
        }
    }

    #[test]
    fn king_bar_label_width_is_positive_and_measurable() {
        assert!(hand_message_width("THE KING", KING_BAR_LABEL_SCALE) > 0);
    }

    #[test]
    fn king_bar_fill_fraction_tracks_health_out_of_the_configured_maximum() {
        // Misma fórmula pura que usa `render_king_health_bar` para el
        // ancho del relleno.
        let fraction =
            |current: i32, max: i32| (current.max(0) as f32 / max as f32).clamp(0.0, 1.0);

        assert!((fraction(1000, 1000) - 1.0).abs() < 1e-6);
        assert!((fraction(500, 1000) - 0.5).abs() < 1e-6);
        assert!((fraction(0, 1000) - 0.0).abs() < 1e-6);
        assert!((fraction(-20, 1000) - 0.0).abs() < 1e-6);
    }

    // --- Bloque 5, Commits 54/55: avisos de invocación de The King. ---

    #[test]
    fn every_king_summon_warning_letter_resolves_to_a_glyph() {
        let lines = [
            "THE KING CALLS HIS HAND!",
            "5 DEALERS JOIN THE HAND",
            "THE KING CALLS HIS FINAL HAND!",
            "10 DEALERS JOIN THE HAND",
            "THE KING IS SHIELDED",
            "CLEAR HIS DEALERS FIRST",
        ];

        for line in lines {
            for character in line.chars() {
                if character == ' ' || character.is_ascii_digit() {
                    continue;
                }
                assert_ne!(
                    letter_glyph_rows(character),
                    [0u8; 7],
                    "falta el glifo para '{character}' en \"{line}\""
                );
            }
        }
    }

    #[test]
    fn king_summon_warning_lines_are_measurable_and_fit_the_framebuffer() {
        for line in [
            "THE KING CALLS HIS HAND!",
            "5 DEALERS JOIN THE HAND",
            "THE KING CALLS HIS FINAL HAND!",
            "10 DEALERS JOIN THE HAND",
            "THE KING IS SHIELDED",
            "CLEAR HIS DEALERS FIRST",
        ] {
            let width = hand_message_width(line, KING_SUMMON_WARNING_SCALE);
            assert!(width > 0);
            assert!(
                width <= crate::config::FRAMEBUFFER_WIDTH,
                "\"{line}\" no cabe centrado"
            );
        }
    }

    #[test]
    fn king_summon_warning_stays_clear_of_the_boss_health_bar_and_the_hud() {
        // La barra de vida ocupa la franja superior; el aviso arranca
        // por debajo de ella y sus dos líneas terminan bien por encima
        // del HUD de vida/munición (anclado abajo).
        let bar_bottom =
            KING_BAR_TOP_MARGIN + 7 * KING_BAR_LABEL_SCALE + KING_BAR_LABEL_GAP + KING_BAR_HEIGHT;
        assert!(KING_SUMMON_WARNING_TOP > bar_bottom);

        let line_step = (KING_SUMMON_WARNING_LINE_HEIGHT + KING_SUMMON_WARNING_LINE_GAP)
            * KING_SUMMON_WARNING_SCALE;
        let warning_bottom = KING_SUMMON_WARNING_TOP
            + line_step
            + KING_SUMMON_WARNING_LINE_HEIGHT * KING_SUMMON_WARNING_SCALE;
        assert!(warning_bottom < crate::config::FRAMEBUFFER_HEIGHT - BOTTOM_MARGIN);
    }
}
