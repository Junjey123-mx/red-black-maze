use raylib::prelude::*;

pub struct Framebuffer {
    image: Image,
    background_color: Color,
    current_color: Color,
}

impl Framebuffer {
    /// Crea un framebuffer con las dimensiones indicadas.
    pub fn new(width: i32, height: i32) -> Self {
        let background_color = Color::BLACK;
        let current_color = Color::WHITE;

        let image = Image::gen_image_color(width, height, background_color);

        Self {
            image,
            background_color,
            current_color,
        }
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
        self.image.clear_background(self.background_color);
    }

    /// Dibuja un píxel, siempre que esté dentro del framebuffer.
    pub fn point(&mut self, x: i32, y: i32) {
        if x >= 0 && x < self.image.width() && y >= 0 && y < self.image.height() {
            self.image.draw_pixel(x, y, self.current_color);
        }
    }

    /// Retorna el ancho lógico del framebuffer.
    pub fn width(&self) -> i32 {
        self.image.width()
    }

    /// Retorna la altura lógica del framebuffer.
    pub fn height(&self) -> i32 {
        self.image.height()
    }

    /// Guarda el contenido del framebuffer en una imagen.
    #[allow(dead_code)]
    pub fn render_to_file(&self, file_name: &str) {
        self.image.export_image(file_name);
    }

    /// Copia el framebuffer a la ventana.
    pub fn swap_buffers(&self, window: &mut RaylibHandle, raylib_thread: &RaylibThread) {
        if let Ok(texture) = window.load_texture_from_image(raylib_thread, &self.image) {
            let screen_width = window.get_screen_width() as f32;
            let screen_height = window.get_screen_height() as f32;

            let mut renderer = window.begin_drawing(raylib_thread);

            renderer.clear_background(self.background_color);

            renderer.draw_texture_pro(
                &texture,
                Rectangle::new(0.0, 0.0, self.width() as f32, self.height() as f32),
                Rectangle::new(0.0, 0.0, screen_width, screen_height),
                Vector2::new(0.0, 0.0),
                0.0,
                Color::WHITE,
            );
        }
    }
}
