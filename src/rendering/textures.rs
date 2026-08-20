use std::collections::HashMap;
use std::fmt;

use raylib::core::error::InvalidImageError;
use raylib::prelude::{Color, Image};

use super::palette::{ThemePalette, palette_for_theme, remap_accent_pixel};
use crate::player::WeaponState;
use crate::world::{EntitySprite, EntityState, LevelTheme};

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

    /// Construye un `TextureAsset` a partir de una `Image` ya en
    /// memoria (no leída de disco) — usado exclusivamente por
    /// `TextureManager::generate_themed_variant` (Tarea 39.B) para
    /// registrar la variante temática recoloreada de un asset base
    /// ya cargado. `label` reemplaza a `path` como identificador de
    /// depuración (no es una ruta de archivo real).
    fn from_image(image: Image, label: String) -> Self {
        Self { path: label, image }
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

/// Genera, a partir de `base`, una nueva `Image` con cada píxel de
/// acento remapeado a `palette` (`palette::remap_accent_pixel`, la
/// ÚNICA función de este proyecto que decide qué píxel es "acento" y
/// hacia qué color se traduce). Los píxeles que `remap_accent_pixel`
/// retorna sin cambios (grises, negro, marfil, transparencia, o
/// cualquier tono fuera de la tabla auditada) no se reescriben.
///
/// Se ejecuta EXCLUSIVAMENTE durante la carga de recursos
/// (`TextureManager::generate_themed_variant`), nunca dentro del
/// bucle de render: es una operación de CPU sobre una `Image` de
/// unas pocas decenas de píxeles por lado, ejecutada como máximo una
/// vez por combinación (asset, tema) durante toda la vida del
/// proceso.
fn remap_image_accent(base: &Image, palette: &ThemePalette) -> Image {
    let mut remapped = base.clone();

    let width = base.width();

    let height = base.height();

    for y in 0..height {
        for x in 0..width {
            let original = base.get_color(x, y);

            let mapped = remap_accent_pixel(original, palette);

            if mapped != original {
                remapped.draw_pixel(x, y, mapped);
            }
        }
    }

    remapped
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

    /// Las tres identidades de nivel existentes hoy. Única lista
    /// que `generate_themed_variants` recorre para pre-generar TODAS
    /// las variantes temáticas de un asset durante su carga — nunca
    /// perezosamente durante el render loop.
    const THEMES: [LevelTheme; 3] = [
        LevelTheme::CrimsonEntrance,
        LevelTheme::BlackClub,
        LevelTheme::HouseOfCards,
    ];

    /// Clave interna bajo la que se cachea la variante temática de
    /// `base_key` para `theme`. Única correspondencia
    /// `(asset, LevelTheme) -> clave` del proyecto.
    fn themed_key(base_key: &str, theme: LevelTheme) -> String {
        format!("{base_key}#{theme:?}")
    }

    /// Genera y cachea, si aún no existe, la variante temática de la
    /// textura ya cargada bajo `base_key` para `theme`.
    ///
    /// No-op silencioso si `base_key` no está cargado (nunca debería
    /// ocurrir en la práctica: siempre se invoca inmediatamente
    /// después de `load(base_key, ...)` dentro del mismo
    /// `load_*_textures`) — preferible a entrar en pánico por un
    /// recurso ausente.
    fn generate_themed_variant(&mut self, base_key: &str, theme: LevelTheme) {
        let variant_key = Self::themed_key(base_key, theme);

        if self.textures.contains_key(&variant_key) {
            return;
        }

        let Some(base) = self.textures.get(base_key) else {
            return;
        };

        let palette = palette_for_theme(theme);

        let remapped_image = remap_image_accent(&base.image, &palette);

        self.textures.insert(
            variant_key.clone(),
            TextureAsset::from_image(remapped_image, variant_key),
        );
    }

    /// Genera las tres variantes temáticas (una por `LevelTheme`) de
    /// la textura ya cargada bajo `base_key`.
    ///
    /// Se invoca una única vez por asset, al final de cada
    /// `load_*_textures` — es decir, durante la carga de recursos al
    /// inicio del programa, JAMÁS dentro del bucle de render. Tarea
    /// 39.B: esto es lo que permite que `wall_color`/las variantes
    /// de textura nunca recoloreen píxeles cuadro a cuadro.
    fn generate_themed_variants(&mut self, base_key: &str) {
        for theme in Self::THEMES {
            self.generate_themed_variant(base_key, theme);
        }
    }

    /// Carga, una única vez, las cuatro texturas de pared del
    /// catálogo centralizado, junto con sus tres variantes temáticas
    /// cada una.
    ///
    /// Reutiliza la API genérica `load` existente; no introduce un
    /// mecanismo de carga paralelo.
    pub(crate) fn load_wall_textures(&mut self) -> Result<(), TextureError> {
        for (_, key, path) in WALL_TEXTURES {
            self.load(key, path)?;

            self.generate_themed_variants(key);
        }

        Ok(())
    }

    /// Resuelve el carácter de pared golpeado por un rayo hacia la
    /// variante de su textura ya cargada correspondiente a `theme`,
    /// si el catálogo lo reconoce.
    ///
    /// Retorna `None` para cualquier carácter sin correspondencia
    /// (por ejemplo `'e'`, `'t'` o un carácter desconocido), sin
    /// entrar en pánico ni indexar de forma insegura.
    pub(crate) fn themed_wall_texture(
        &self,
        tile: char,
        theme: LevelTheme,
    ) -> Option<&TextureAsset> {
        let (_, key, _) = WALL_TEXTURES.iter().find(|(cell, _, _)| *cell == tile)?;

        self.get(&Self::themed_key(key, theme))
    }

    /// Carga, una única vez, la textura del sprite de meta, junto
    /// con sus tres variantes temáticas.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_goal_texture(&mut self) -> Result<(), TextureError> {
        self.load(GOAL_TEXTURE_KEY, GOAL_TEXTURE_PATH)?;

        self.generate_themed_variants(GOAL_TEXTURE_KEY);

        Ok(())
    }

    /// Variante temática ya cargada del sprite de meta para `theme`,
    /// si está disponible.
    pub(crate) fn themed_goal_texture(&self, theme: LevelTheme) -> Option<&TextureAsset> {
        self.get(&Self::themed_key(GOAL_TEXTURE_KEY, theme))
    }

    /// Carga, una única vez, los cuatro cuadros de animación de la
    /// antorcha, junto con sus tres variantes temáticas cada uno.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_torch_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in TORCH_TEXTURES {
            self.load(key, path)?;

            self.generate_themed_variants(key);
        }

        Ok(())
    }

    /// Variante temática ya cargada del cuadro de antorcha
    /// solicitado, para `theme`.
    ///
    /// Retorna `None` para cualquier índice fuera de rango, sin
    /// entrar en pánico. Este método NO controla el temporizado de
    /// la animación; solo resuelve un índice ya decidido por el
    /// estado de juego hacia su recurso de textura.
    pub(crate) fn themed_torch_texture(
        &self,
        frame_index: usize,
        theme: LevelTheme,
    ) -> Option<&TextureAsset> {
        let (key, _) = TORCH_TEXTURES.get(frame_index)?;

        self.get(&Self::themed_key(key, theme))
    }

    /// Carga, una única vez, las tres texturas del arma en primera
    /// persona, junto con sus tres variantes temáticas cada una.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_weapon_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in WEAPON_TEXTURES {
            self.load(key, path)?;

            self.generate_themed_variants(key);
        }

        Ok(())
    }

    /// Variante temática ya cargada correspondiente al estado visual
    /// del arma solicitado, para `theme`.
    ///
    /// Resuelve únicamente el mapeo estado -> textura ya cargada;
    /// no controla ni conoce el temporizado de la máquina de
    /// estados.
    pub(crate) fn themed_weapon_texture(
        &self,
        state: WeaponState,
        theme: LevelTheme,
    ) -> Option<&TextureAsset> {
        let key = match state {
            /*
             * `Reload` reutiliza la textura de `Idle`: Tarea 38.C
             * prioriza la recarga funcional + HUD sobre una animación
             * dedicada, y explícitamente prohíbe cargar un nuevo PNG
             * de arma. No hay ninguna textura "weapon-reload".
             */
            WeaponState::Idle | WeaponState::Reload => "weapon-idle",
            WeaponState::Fire => "weapon-fire",
            WeaponState::Recoil => "weapon-recoil",
        };

        self.get(&Self::themed_key(key, theme))
    }

    /// Carga, una única vez, las cuatro texturas de estado del
    /// Dealer del catálogo centralizado, junto con sus tres
    /// variantes temáticas cada una.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_entity_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in DEALER_TEXTURES {
            self.load(key, path)?;

            self.generate_themed_variants(key);
        }

        Ok(())
    }

    /// Variante temática ya cargada correspondiente a la identidad
    /// visual y al estado de comportamiento de la entidad
    /// solicitada, para `theme`.
    ///
    /// Resuelve únicamente el mapeo identidad+estado -> textura ya
    /// cargada; no controla ni conoce el temporizado o las
    /// invariantes de combate de la entidad.
    pub(crate) fn themed_entity_texture(
        &self,
        sprite: EntitySprite,
        state: EntityState,
        theme: LevelTheme,
    ) -> Option<&TextureAsset> {
        let key = match (sprite, state) {
            (EntitySprite::Dealer, EntityState::Idle) => "entity-dealer-idle",
            (EntitySprite::Dealer, EntityState::Alert) => "entity-dealer-alert",
            (EntitySprite::Dealer, EntityState::Hit) => "entity-dealer-hit",
            (EntitySprite::Dealer, EntityState::Dead) => "entity-dealer-dead",
        };

        self.get(&Self::themed_key(key, theme))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Ninguna de estas pruebas necesita una ventana/GPU: `Image` es
    /// un recurso puramente de CPU (decodificación de PNG), y
    /// `TextureManager` no toca `Texture2D`/Raylib gráfico en
    /// absoluto. Se cargan los assets REALES del proyecto (las
    /// mismas rutas que usa `App::run`), no fixtures sintéticos.
    #[test]
    fn themed_key_differs_per_theme_and_is_stable() {
        let crimson_key = TextureManager::themed_key("wall-club", LevelTheme::CrimsonEntrance);
        let black_club_key = TextureManager::themed_key("wall-club", LevelTheme::BlackClub);
        let house_key = TextureManager::themed_key("wall-club", LevelTheme::HouseOfCards);

        assert_ne!(crimson_key, black_club_key);
        assert_ne!(crimson_key, house_key);
        assert_ne!(black_club_key, house_key);

        // Estable: la misma combinación (asset, tema) produce
        // siempre la misma clave.
        assert_eq!(
            crimson_key,
            TextureManager::themed_key("wall-club", LevelTheme::CrimsonEntrance)
        );
    }

    #[test]
    fn loading_wall_textures_generates_a_lookup_hit_for_every_theme() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        for theme in TextureManager::THEMES {
            assert!(
                manager.themed_wall_texture('|', theme).is_some(),
                "falta la variante temática {theme:?} de la pared '|'"
            );
        }

        // Carácter sin textura registrada: sigue sin resolver nada,
        // en cualquier tema.
        assert!(
            manager
                .themed_wall_texture('?', LevelTheme::CrimsonEntrance)
                .is_none()
        );
    }

    #[test]
    fn different_themes_do_not_collide_in_the_cache() {
        let mut manager = TextureManager::new();

        manager
            .load_goal_texture()
            .expect("la textura de meta del proyecto debe cargar");

        // Cada tema se guarda bajo una clave propia: contar cuántas
        // entradas produce `generate_themed_variants` para un único
        // asset confirma que no colisionan.
        let crimson = manager.get(&TextureManager::themed_key(
            GOAL_TEXTURE_KEY,
            LevelTheme::CrimsonEntrance,
        ));

        let black_club = manager.get(&TextureManager::themed_key(
            GOAL_TEXTURE_KEY,
            LevelTheme::BlackClub,
        ));

        let house = manager.get(&TextureManager::themed_key(
            GOAL_TEXTURE_KEY,
            LevelTheme::HouseOfCards,
        ));

        assert!(crimson.is_some());
        assert!(black_club.is_some());
        assert!(house.is_some());
    }

    #[test]
    fn themed_variant_is_not_regenerated_on_a_second_request() {
        let mut manager = TextureManager::new();

        manager
            .load_weapon_textures()
            .expect("las texturas del arma del proyecto deben cargar");

        let key_before = TextureManager::themed_key("weapon-idle", LevelTheme::CrimsonEntrance);

        assert!(manager.textures.contains_key(&key_before));

        let entry_count_before = manager.textures.len();

        // Repetir la generación (como haría una segunda llamada
        // accidental) no debe insertar una segunda entrada.
        manager.generate_themed_variant("weapon-idle", LevelTheme::CrimsonEntrance);

        assert_eq!(manager.textures.len(), entry_count_before);
    }

    #[test]
    fn crimson_and_house_variants_still_match_the_base_pixel_for_pixel_after_task_40() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        let base = manager.get("wall-club").expect("base debe existir");

        // Tarea 40 solo hizo divergir a Black Club: Crimson Entrance
        // y House of Cards (todavía pendiente de Tarea 41) deben
        // seguir generando una variante IDÉNTICA píxel a píxel al
        // asset base — ninguna regresión visual para ellos.
        for theme in [LevelTheme::CrimsonEntrance, LevelTheme::HouseOfCards] {
            let variant = manager
                .themed_wall_texture('|', theme)
                .unwrap_or_else(|| panic!("la variante {theme:?} debe existir"));

            assert_eq!(base.width(), variant.width());
            assert_eq!(base.height(), variant.height());

            for y in 0..base.height() {
                for x in 0..base.width() {
                    assert_eq!(
                        base.pixel_at(x, y),
                        variant.pixel_at(x, y),
                        "diverge en ({x}, {y}) para el tema {theme:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn black_club_wall_variant_differs_from_the_legacy_red_base() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        let base = manager.get("wall-club").expect("base debe existir");

        let variant = manager
            .themed_wall_texture('|', LevelTheme::BlackClub)
            .expect("la variante Black Club debe existir");

        assert_eq!(base.width(), variant.width());
        assert_eq!(base.height(), variant.height());

        let mut saw_a_different_pixel = false;

        for y in 0..base.height() {
            for x in 0..base.width() {
                if base.pixel_at(x, y) != variant.pixel_at(x, y) {
                    saw_a_different_pixel = true;
                }
            }
        }

        assert!(
            saw_a_different_pixel,
            "la variante Black Club debe diferir del rojo heredado en al menos un píxel"
        );
    }

    #[test]
    fn black_club_wall_variant_maps_the_canonical_red_to_the_exact_reference_orange() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        // club.png contiene el rojo canónico brillante (210, 31, 43)
        // — auditado en Tarea 39.B — en varios píxeles conocidos de
        // su arte. Confirmamos que la variante Black Club lo
        // convierte EXACTAMENTE en el naranja de referencia del Plan
        // Maestro, no en una aproximación.
        let base = manager.get("wall-club").expect("base debe existir");

        let variant = manager
            .themed_wall_texture('|', LevelTheme::BlackClub)
            .expect("la variante Black Club debe existir");

        let mut checked_a_canonical_red_pixel = false;

        for y in 0..base.height() {
            for x in 0..base.width() {
                if base.pixel_at(x, y) == Some(Color::new(210, 31, 43, 255)) {
                    checked_a_canonical_red_pixel = true;

                    assert_eq!(
                        variant.pixel_at(x, y),
                        Some(Color::new(0xFF, 0x7A, 0x00, 255))
                    );
                }
            }
        }

        assert!(
            checked_a_canonical_red_pixel,
            "club.png debe contener el rojo canónico auditado en Tarea 39.B"
        );
    }

    #[test]
    fn remap_image_accent_preserves_transparency_on_a_real_asset() {
        let base_image = Image::load_image("assets/textures/sprites/goal.png")
            .expect("el asset de meta del proyecto debe cargar");

        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let remapped = remap_image_accent(&base_image, &palette);

        let mut saw_a_transparent_pixel = false;

        for y in 0..base_image.height() {
            for x in 0..base_image.width() {
                let original = base_image.get_color(x, y);

                if original.a == 0 {
                    saw_a_transparent_pixel = true;

                    assert_eq!(remapped.get_color(x, y).a, 0);
                }
            }
        }

        // El asset real de meta tiene píxeles transparentes de
        // sobra (fondo del sprite); si esto fallara silenciosamente
        // por un asset distinto, la prueba lo haría evidente.
        assert!(saw_a_transparent_pixel);
    }
}
