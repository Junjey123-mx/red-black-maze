use std::collections::HashMap;
use std::fmt;

use raylib::core::error::InvalidImageError;
use raylib::prelude::{Color, Image};

use crate::player::WeaponState;
use crate::world::{EntitySprite, EntityState};

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

/// Catálogo centralizado y congelado de las texturas de pared:
/// carácter de pared -> (clave interna, ruta del recurso).
///
/// Esta es la ÚNICA correspondencia entre un carácter de pared y
/// su textura en todo el proyecto; los renderers deben consultarla
/// a través de `TextureManager`, nunca duplicarla.
const WALL_TEXTURES: [(char, &str, &str); 4] = [
    ('+', "wall-heart", "assets/textures/walls/heart.png"),
    ('-', "wall-diamond", "assets/textures/walls/diamond.png"),
    ('|', "wall-club", "assets/textures/walls/club.png"),
    ('#', "wall-spade", "assets/textures/walls/spade.png"),
];

/// Clave y ruta congeladas de la textura del sprite de meta.
///
/// Única correspondencia identidad/ruta para este recurso en todo
/// el proyecto.
const GOAL_TEXTURE_KEY: &str = "sprite-goal";
const GOAL_TEXTURE_PATH: &str = "assets/textures/sprites/goal.png";

/// Catálogo centralizado y congelado de los cuatro cuadros de
/// animación de la antorcha, en orden de reproducción.
///
/// Esta es la ÚNICA correspondencia entre un índice de cuadro y su
/// recurso de textura en todo el proyecto. El índice/tiempo de
/// animación NO vive aquí: `TextureManager` solo posee los datos
/// decodificados de cada cuadro.
const TORCH_TEXTURES: [(&str, &str); 4] = [
    (
        "sprite-torch-01",
        "assets/textures/sprites/torch/torch_01.png",
    ),
    (
        "sprite-torch-02",
        "assets/textures/sprites/torch/torch_02.png",
    ),
    (
        "sprite-torch-03",
        "assets/textures/sprites/torch/torch_03.png",
    ),
    (
        "sprite-torch-04",
        "assets/textures/sprites/torch/torch_04.png",
    ),
];

/// Catálogo centralizado y congelado de las tres texturas del arma
/// en primera persona, una por cada `WeaponState`.
///
/// Esta es la ÚNICA correspondencia entre una ruta de recurso y su
/// clave interna para las texturas del arma. El mapeo estado ->
/// clave vive en `weapon_texture`; el temporizado de la máquina de
/// estados NO vive aquí, sino en `Weapon` (`player/weapon.rs`).
const WEAPON_TEXTURES: [(&str, &str); 3] = [
    ("weapon-idle", "assets/textures/weapon/weapon_idle.png"),
    ("weapon-fire", "assets/textures/weapon/weapon_fire.png"),
    ("weapon-recoil", "assets/textures/weapon/weapon_recoil.png"),
];

/// Catálogo centralizado y congelado de las cuatro texturas de
/// estado del Dealer: clave interna -> ruta del recurso.
///
/// Esta es la ÚNICA correspondencia identidad+estado -> ruta de
/// asset en todo el proyecto; el mapeo `(EntitySprite, EntityState)
/// -> clave` vive en `entity_texture`, nunca duplicado en
/// `world/entity.rs` ni en `rendering/sprites.rs`.
const DEALER_TEXTURES: [(&str, &str); 4] = [
    (
        "entity-dealer-idle",
        "assets/textures/sprites/enemies/dealer.png",
    ),
    (
        "entity-dealer-alert",
        "assets/textures/sprites/enemies/dealer_alert.png",
    ),
    (
        "entity-dealer-hit",
        "assets/textures/sprites/enemies/dealer_hit.png",
    ),
    (
        "entity-dealer-dead",
        "assets/textures/sprites/enemies/dealer_dead.png",
    ),
];

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

    /// Carga, una única vez, las cuatro texturas de pared del
    /// catálogo centralizado.
    ///
    /// Reutiliza la API genérica `load` existente; no introduce un
    /// mecanismo de carga paralelo.
    pub(crate) fn load_wall_textures(&mut self) -> Result<(), TextureError> {
        for (_, key, path) in WALL_TEXTURES {
            self.load(key, path)?;
        }

        Ok(())
    }

    /// Resuelve el carácter de pared golpeado por un rayo hacia su
    /// textura ya cargada, si el catálogo lo reconoce.
    ///
    /// Retorna `None` para cualquier carácter sin correspondencia
    /// (por ejemplo `'e'`, `'t'` o un carácter desconocido), sin
    /// entrar en pánico ni indexar de forma insegura.
    pub(crate) fn wall_texture(&self, tile: char) -> Option<&TextureAsset> {
        let (_, key, _) = WALL_TEXTURES.iter().find(|(cell, _, _)| *cell == tile)?;

        self.get(key)
    }

    /// Carga, una única vez, la textura del sprite de meta.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_goal_texture(&mut self) -> Result<(), TextureError> {
        self.load(GOAL_TEXTURE_KEY, GOAL_TEXTURE_PATH)
    }

    /// Textura ya cargada del sprite de meta, si está disponible.
    pub(crate) fn goal_texture(&self) -> Option<&TextureAsset> {
        self.get(GOAL_TEXTURE_KEY)
    }

    /// Carga, una única vez, los cuatro cuadros de animación de la
    /// antorcha.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_torch_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in TORCH_TEXTURES {
            self.load(key, path)?;
        }

        Ok(())
    }

    /// Textura ya cargada del cuadro de antorcha solicitado.
    ///
    /// Retorna `None` para cualquier índice fuera de rango, sin
    /// entrar en pánico. Este método NO controla el temporizado de
    /// la animación; solo resuelve un índice ya decidido por el
    /// estado de juego hacia su recurso de textura.
    pub(crate) fn torch_texture(&self, frame_index: usize) -> Option<&TextureAsset> {
        let (key, _) = TORCH_TEXTURES.get(frame_index)?;

        self.get(key)
    }

    /// Carga, una única vez, las tres texturas del arma en primera
    /// persona.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_weapon_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in WEAPON_TEXTURES {
            self.load(key, path)?;
        }

        Ok(())
    }

    /// Textura ya cargada correspondiente al estado visual del arma
    /// solicitado.
    ///
    /// Resuelve únicamente el mapeo estado -> textura ya cargada;
    /// no controla ni conoce el temporizado de la máquina de
    /// estados.
    pub(crate) fn weapon_texture(&self, state: WeaponState) -> Option<&TextureAsset> {
        let key = match state {
            WeaponState::Idle => "weapon-idle",
            WeaponState::Fire => "weapon-fire",
            WeaponState::Recoil => "weapon-recoil",
        };

        self.get(key)
    }

    /// Carga, una única vez, las cuatro texturas de estado del
    /// Dealer del catálogo centralizado.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_entity_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in DEALER_TEXTURES {
            self.load(key, path)?;
        }

        Ok(())
    }

    /// Textura ya cargada correspondiente a la identidad visual y
    /// al estado de comportamiento de la entidad solicitada.
    ///
    /// Resuelve únicamente el mapeo identidad+estado -> textura ya
    /// cargada; no controla ni conoce el temporizado o las
    /// invariantes de combate de la entidad.
    pub(crate) fn entity_texture(
        &self,
        sprite: EntitySprite,
        state: EntityState,
    ) -> Option<&TextureAsset> {
        let key = match (sprite, state) {
            (EntitySprite::Dealer, EntityState::Idle) => "entity-dealer-idle",
            (EntitySprite::Dealer, EntityState::Alert) => "entity-dealer-alert",
            (EntitySprite::Dealer, EntityState::Hit) => "entity-dealer-hit",
            (EntitySprite::Dealer, EntityState::Dead) => "entity-dealer-dead",
        };

        self.get(key)
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
