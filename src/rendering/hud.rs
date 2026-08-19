use raylib::prelude::Color;

use super::framebuffer::Framebuffer;

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

const HUD_RED: Color = Color::new(205, 30, 42, 255);
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
/// (`health`, `ammo`) ya leídas por el llamador; no posee ni
/// modifica `Player`, `Weapon` ni `GameSession`, no lee entrada, y
/// no dispara ni cura. `health`/`ammo` son los ÚNICOS valores que
/// determinan lo que se dibuja: no existe ningún literal `100`/`06`
/// de visualización en este módulo, solo constantes de geometría de
/// glifo y la vida/munición inicial del dominio (que viven en
/// `player.rs`/`weapon.rs`, no aquí).
pub(crate) fn render_hud(framebuffer: &mut Framebuffer, health: i32, ammo: u32) {
    let framebuffer_width = framebuffer.width();

    let framebuffer_height = framebuffer.height();

    if framebuffer_width <= 0 || framebuffer_height <= 0 {
        return;
    }

    let row_height = GLYPH_HEIGHT * GLYPH_SCALE;

    let origin_x = LEFT_MARGIN;

    let origin_y = framebuffer_height - BOTTOM_MARGIN - row_height;

    draw_glyph(
        framebuffer,
        &HEART_ICON,
        GLYPH_WIDTH,
        origin_x,
        origin_y,
        HUD_RED,
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
        HUD_RED,
    );

    let ammo_digits_x = diamond_x + (GLYPH_WIDTH + GLYPH_GAP) * GLYPH_SCALE;

    let ammo_digits = digits_of(ammo as i64, AMMO_MIN_DIGITS);

    draw_digits(
        framebuffer,
        &ammo_digits,
        ammo_digits_x,
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
}
