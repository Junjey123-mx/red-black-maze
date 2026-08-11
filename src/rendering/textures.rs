use std::collections::HashMap;
use std::fmt;

use raylib::core::error::InvalidImageError;
use raylib::prelude::{Color, Image};

/// Recurso de textura decodificado y retenido en memoria de CPU,
/// listo para ser muestreado píxel a píxel por el framebuffer
/// de software sin volver a leer el archivo.
pub(crate) struct TextureAsset {
    path: String,
    image: Image,
}

impl TextureAsset {
    fn load(path: &str) -> Result<Self, InvalidImageError> {
        let image = Image::load_image(path)?;

        Ok(Self {
            path: path.to_string(),
            image,
        })
    }

    /// Ancho de la textura en píxeles.
    pub(crate) fn width(&self) -> i32 {
        self.image.width()
    }

    /// Alto de la textura en píxeles.
    pub(crate) fn height(&self) -> i32 {
        self.image.height()
    }

    /// Acceso seguro al color de un píxel decodificado.
    ///
    /// No realiza ninguna normalización de coordenadas de pared;
    /// simplemente retorna el píxel solicitado si está dentro de
    /// los límites de la textura.
    pub(crate) fn pixel_at(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x >= self.image.width() || y >= self.image.height() {
            return None;
        }

        Some(self.image.get_color(x, y))
    }
}

/// Error al cargar o registrar un recurso de textura.
#[derive(Debug)]
pub(crate) enum TextureError {
    /// El archivo no pudo abrirse o decodificarse como imagen.
    Load {
        key: String,
        path: String,
        source: InvalidImageError,
    },

    /// La clave ya está asociada a una ruta distinta.
    KeyPathConflict {
        key: String,
        existing_path: String,
        requested_path: String,
    },
}

impl fmt::Display for TextureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextureError::Load { key, path, source } => write!(
                formatter,
                "No se pudo cargar la textura '{key}' desde '{path}': {source}",
            ),

            TextureError::KeyPathConflict {
                key,
                existing_path,
                requested_path,
            } => write!(
                formatter,
                "La clave de textura '{key}' ya está cargada desde '{existing_path}' \
                 y no puede volver a cargarse desde '{requested_path}'",
            ),
        }
    }
}

/// Administra la carga única y el acceso a los recursos de
/// textura utilizados por el renderer.
pub(crate) struct TextureManager {
    textures: HashMap<String, TextureAsset>,
}

impl TextureManager {
    /// Crea un administrador de texturas vacío.
    pub(crate) fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    /// Carga una textura la primera vez que se solicita `key`.
    ///
    /// Una solicitud repetida con la MISMA clave y la MISMA ruta
    /// es idempotente y no vuelve a leer ni decodificar el
    /// archivo. Reutilizar una clave ya cargada con una ruta
    /// distinta es un error determinista.
    pub(crate) fn load(&mut self, key: &str, path: &str) -> Result<(), TextureError> {
        if let Some(existing) = self.textures.get(key) {
            if existing.path == path {
                return Ok(());
            }

            return Err(TextureError::KeyPathConflict {
                key: key.to_string(),
                existing_path: existing.path.clone(),
                requested_path: path.to_string(),
            });
        }

        let asset = TextureAsset::load(path).map_err(|source| TextureError::Load {
            key: key.to_string(),
            path: path.to_string(),
            source,
        })?;

        self.textures.insert(key.to_string(), asset);

        Ok(())
    }

    /// Acceso inmutable a una textura ya cargada.
    pub(crate) fn get(&self, key: &str) -> Option<&TextureAsset> {
        self.textures.get(key)
    }
}
