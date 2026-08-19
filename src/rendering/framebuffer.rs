use raylib::core::error::LoadTextureError;
use raylib::prelude::*;

/// Framebuffer lógico de software: dibuja píxeles en un búfer RGBA8
/// propio en RAM y los presenta en pantalla mediante una única
/// textura GPU persistente, actualizada in-place cada cuadro.
///
/// Tarea 38: antes, `swap_buffers` creaba (`load_texture_from_image`)
/// y destruía una `Texture2D` COMPLETA cada cuadro — la operación de
/// recursos GPU más costosa posible en un bucle de render, repetida
/// 60 veces por segundo. Ahora la textura de presentación se crea
/// UNA sola vez en `Framebuffer::new` y cada cuadro solo sube los
/// píxeles ya cambiados mediante `Texture2D::update_texture` (subida
/// de datos, sin crear/destruir el recurso GPU en sí).
pub struct Framebuffer {
    /// Búfer de píxeles propio, en RAM, formato RGBA8 (4 bytes por
    /// píxel, mismo orden que `Color { r, g, b, a }`). Persiste entre
    /// cuadros: `clear()`/`point()` escriben directamente aquí, sin
    /// pasar por ninguna abstracción de `Image` en el camino
    /// caliente.
    pixels: Vec<u8>,

    width: i32,
    height: i32,

    /// Textura de presentación GPU, creada UNA vez y reutilizada
    /// durante toda la ejecución. Se libera automáticamente (RAII,
    /// `UnloadTexture`) cuando el `Framebuffer` se destruye, antes de
    /// que `RaylibHandle`/`RaylibThread` se cierren en `run()`.
    texture: Texture2D,

    background_color: Color,
    current_color: Color,
}

impl Framebuffer {
    /// Crea un framebuffer con las dimensiones indicadas y su
    /// textura de presentación GPU (creada una única vez aquí).
    ///
    /// Requiere `window`/`raylib_thread` porque la creación de la
    /// textura persistente necesita el contexto de Raylib ya
    /// inicializado; `Framebuffer` en sí no abre ninguna ventana.
    pub fn new(
        width: i32,
        height: i32,
        window: &mut RaylibHandle,
        raylib_thread: &RaylibThread,
    ) -> Result<Self, LoadTextureError> {
        let background_color = Color::BLACK;
        let current_color = Color::WHITE;

        let pixel_count = (width.max(0) as usize) * (height.max(0) as usize);

        let mut pixels = vec![0u8; pixel_count * 4];

        fill_pixels(&mut pixels, background_color);

        /*
         * `Image` solo se usa aquí, una única vez, como semilla para
         * crear la textura persistente inicial (`gen_image_color`
         * produce el formato RGBA8 estándar de Raylib, el mismo que
         * `pixels` usa). Después de esta llamada no vuelve a existir
         * ningún `Image` en el camino de dibujo por cuadro.
         */
        let seed_image = Image::gen_image_color(width.max(1), height.max(1), background_color);

        let texture = window.load_texture_from_image(raylib_thread, &seed_image)?;

        Ok(Self {
            pixels,
            width,
            height,
            texture,
            background_color,
            current_color,
        })
    }

    /// Cambia el color utilizado por clear().
    pub fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    /// Cambia el color utilizado por point().
    pub fn set_current_color(&mut self, color: Color) {
        self.current_color = color;
    }

    /// Limpia todo el framebuffer.
    pub fn clear(&mut self) {
        fill_pixels(&mut self.pixels, self.background_color);
    }

    /// Dibuja un píxel, siempre que esté dentro del framebuffer.
    pub fn point(&mut self, x: i32, y: i32) {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            let index = ((y * self.width + x) as usize) * 4;

            write_color(&mut self.pixels[index..index + 4], self.current_color);
        }
    }

    /// Retorna el ancho lógico del framebuffer.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// Retorna la altura lógica del framebuffer.
    pub fn height(&self) -> i32 {
        self.height
    }

    /// Guarda el contenido del framebuffer en una imagen.
    ///
    /// Ruta deliberadamente NO optimizada: reconstruye un `Image`
    /// desde `pixels` bajo demanda. No se invoca en ningún camino de
    /// dibujo por cuadro (solo depuración manual), por lo que su
    /// costo por-píxel es irrelevante.
    #[allow(dead_code)]
    pub fn render_to_file(&self, file_name: &str) {
        let mut image = Image::gen_image_color(self.width.max(1), self.height.max(1), Color::BLACK);

        for y in 0..self.height {
            for x in 0..self.width {
                let index = ((y * self.width + x) as usize) * 4;

                let color = Color::new(
                    self.pixels[index],
                    self.pixels[index + 1],
                    self.pixels[index + 2],
                    self.pixels[index + 3],
                );

                image.draw_pixel(x, y, color);
            }
        }

        image.export_image(file_name);
    }

    /// Sube el framebuffer actual a la textura de presentación
    /// persistente y la dibuja escalada a pantalla completa.
    ///
    /// No crea ni destruye ningún recurso GPU: `update_texture` solo
    /// sube los bytes ya escritos por `clear()`/`point()` de este
    /// cuadro a la MISMA textura creada en `Framebuffer::new`. Un
    /// fallo de `update_texture` (tamaño de datos inesperado, que en
    /// la práctica nunca ocurre porque `pixels` siempre coincide
    /// exactamente con las dimensiones/formato de la textura) se
    /// ignora de forma segura para ese cuadro en vez de entrar en
    /// pánico.
    pub fn swap_buffers(&mut self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        let _ = self.texture.update_texture(&self.pixels);

        let screen_width = window.get_screen_width() as f32;
        let screen_height = window.get_screen_height() as f32;

        let mut renderer = window.begin_drawing(raylib_thread);

        renderer.clear_background(self.background_color);

        renderer.draw_texture_pro(
            &self.texture,
            Rectangle::new(0.0, 0.0, self.width as f32, self.height as f32),
            Rectangle::new(0.0, 0.0, screen_width, screen_height),
            Vector2::new(0.0, 0.0),
            0.0,
            Color::WHITE,
        );
    }
}

/// Escribe `color` en las 4 posiciones RGBA8 de `slice` (que debe
/// tener exactamente longitud 4).
fn write_color(slice: &mut [u8], color: Color) {
    slice[0] = color.r;
    slice[1] = color.g;
    slice[2] = color.b;
    slice[3] = color.a;
}

/// Rellena todo `pixels` (buffer RGBA8 completo) con `color`.
fn fill_pixels(pixels: &mut [u8], color: Color) {
    for chunk in pixels.chunks_exact_mut(4) {
        write_color(chunk, color);
    }
}
