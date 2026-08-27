use std::collections::HashMap;
use std::fmt;

use raylib::core::error::InvalidImageError;
use raylib::prelude::{Color, Image};

use super::palette::{ThemePalette, palette_for_theme, remap_accent_pixel};
use crate::player::{WeaponState, WeaponTier};
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

/// Clave y ruta congeladas de la textura del pickup de munición
/// (Tarea 44). Único asset base, igual que el resto de sprites
/// temáticos: NO existen `ammo_pickup_red.png`/`_orange.png`/
/// `_violet.png` — las tres variantes se generan a partir de este
/// único PNG mediante el mismo pipeline (`generate_themed_variants`)
/// que ya usan paredes/arma/Dealer/antorchas/meta.
const AMMO_PICKUP_TEXTURE_KEY: &str = "sprite-ammo-pickup";
const AMMO_PICKUP_TEXTURE_PATH: &str = "assets/textures/sprites/ammo_pickup.png";

/// Clave y ruta congeladas de la textura del pickup de vida (Health
/// Pickup). Único asset base, igual que el resto de sprites temáticos
/// (pared/arma/Dealer/antorchas/meta/pickup de munición): las tres
/// variantes se generan a partir de este único PNG mediante el mismo
/// pipeline (`generate_themed_variants`), para que el corazón adopte
/// la identidad cromática del nivel activo exactamente igual que
/// cualquier otro elemento temático del juego.
const HEALTH_PICKUP_TEXTURE_KEY: &str = "sprite-health-pickup";
const HEALTH_PICKUP_TEXTURE_PATH: &str = "assets/textures/sprites/health_pickup.png";

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

/// Catálogo congelado de las tres texturas de The Royal Flush en
/// primera persona (Bloque 2, Commit 16), una por cada `WeaponState`
/// visible — misma correspondencia estado -> clave que el arma
/// Standard, con su propio conjunto de assets dorados.
///
/// A diferencia de `WEAPON_TEXTURES`, estas NO pasan por
/// `generate_themed_variants`: la identidad negro+oro de The Royal
/// Flush es permanente y NO se recolorea por `LevelTheme` (Crimson/
/// Black Club/House of Cards) — el dorado es la señal universal de
/// "conseguiste el arma especial".
const ROYAL_WEAPON_TEXTURES: [(&str, &str); 3] = [
    (
        "royal-weapon-idle",
        "assets/textures/weapon/royal_weapon_idle.png",
    ),
    (
        "royal-weapon-fire",
        "assets/textures/weapon/royal_weapon_fire.png",
    ),
    (
        "royal-weapon-recoil",
        "assets/textures/weapon/royal_weapon_recoil.png",
    ),
];

/// Clave/ruta del billboard de mundo de The Royal Flush (Bloque 2,
/// Commit 16). Tampoco pasa por variantes temáticas: dorada en los
/// cuatro niveles.
const ROYAL_FLUSH_PICKUP_TEXTURE_KEY: &str = "sprite-royal-flush-pickup";
const ROYAL_FLUSH_PICKUP_TEXTURE_PATH: &str = "assets/textures/sprites/royal_flush_pickup.png";

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

    /// Carga, una única vez, la textura del pickup de munición
    /// (Tarea 44), junto con sus tres variantes temáticas.
    ///
    /// Reutiliza la API genérica `load` existente.
    pub(crate) fn load_ammo_pickup_texture(&mut self) -> Result<(), TextureError> {
        self.load(AMMO_PICKUP_TEXTURE_KEY, AMMO_PICKUP_TEXTURE_PATH)?;

        self.generate_themed_variants(AMMO_PICKUP_TEXTURE_KEY);

        Ok(())
    }

    /// Variante temática ya cargada del pickup de munición para
    /// `theme`, si está disponible.
    pub(crate) fn themed_ammo_pickup_texture(&self, theme: LevelTheme) -> Option<&TextureAsset> {
        self.get(&Self::themed_key(AMMO_PICKUP_TEXTURE_KEY, theme))
    }

    /// Carga, una única vez, la textura del pickup de vida (Health
    /// Pickup), junto con sus tres variantes temáticas.
    ///
    /// Reutiliza la API genérica `load` existente — mismo patrón
    /// exacto que `load_ammo_pickup_texture`.
    pub(crate) fn load_health_pickup_texture(&mut self) -> Result<(), TextureError> {
        self.load(HEALTH_PICKUP_TEXTURE_KEY, HEALTH_PICKUP_TEXTURE_PATH)?;

        self.generate_themed_variants(HEALTH_PICKUP_TEXTURE_KEY);

        Ok(())
    }

    /// Variante temática ya cargada del pickup de vida para `theme`,
    /// si está disponible.
    pub(crate) fn themed_health_pickup_texture(&self, theme: LevelTheme) -> Option<&TextureAsset> {
        self.get(&Self::themed_key(HEALTH_PICKUP_TEXTURE_KEY, theme))
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

    /// Carga, una única vez, las tres texturas de The Royal Flush en
    /// primera persona (Bloque 2, Commit 16).
    ///
    /// Reutiliza la MISMA API genérica `load` que el arma Standard,
    /// pero SIN `generate_themed_variants`: la paleta negro+oro es
    /// permanente y no se recolorea por nivel.
    pub(crate) fn load_royal_weapon_textures(&mut self) -> Result<(), TextureError> {
        for (key, path) in ROYAL_WEAPON_TEXTURES {
            self.load(key, path)?;
        }

        Ok(())
    }

    /// Carga, una única vez, el billboard de mundo de The Royal Flush
    /// (Bloque 2, Commit 16), también sin variantes temáticas.
    pub(crate) fn load_royal_flush_pickup_texture(&mut self) -> Result<(), TextureError> {
        self.load(
            ROYAL_FLUSH_PICKUP_TEXTURE_KEY,
            ROYAL_FLUSH_PICKUP_TEXTURE_PATH,
        )
    }

    /// Textura ya cargada del billboard de mundo de The Royal Flush,
    /// si está disponible. No temática: la misma textura dorada en
    /// todos los niveles.
    pub(crate) fn royal_flush_pickup_texture(&self) -> Option<&TextureAsset> {
        self.get(ROYAL_FLUSH_PICKUP_TEXTURE_KEY)
    }

    /// Textura ya cargada correspondiente al estado visual del arma
    /// solicitado, para el `theme` y el `tier` activos.
    ///
    /// Resuelve únicamente el mapeo (estado, tier) -> textura ya
    /// cargada; no controla ni conoce el temporizado de la máquina de
    /// estados. Para `WeaponTier::Standard` devuelve la variante
    /// temática del arma base (comportamiento idéntico al de antes del
    /// Bloque 2). Para `WeaponTier::RoyalFlush` devuelve el sprite
    /// dorado dedicado, NO temático — misma pipeline de render,
    /// distinto conjunto de assets.
    pub(crate) fn themed_weapon_texture(
        &self,
        state: WeaponState,
        theme: LevelTheme,
        tier: WeaponTier,
    ) -> Option<&TextureAsset> {
        /*
         * `Reload` reutiliza la textura de `Idle` en ambos tiers:
         * Tarea 38.C prioriza la recarga funcional + HUD sobre una
         * animación dedicada. No hay ninguna textura "*-reload".
         */
        match tier {
            WeaponTier::Standard => {
                let key = match state {
                    WeaponState::Idle | WeaponState::Reload => "weapon-idle",
                    WeaponState::Fire => "weapon-fire",
                    WeaponState::Recoil => "weapon-recoil",
                };

                self.get(&Self::themed_key(key, theme))
            }

            WeaponTier::RoyalFlush => {
                let key = match state {
                    WeaponState::Idle | WeaponState::Reload => "royal-weapon-idle",
                    WeaponState::Fire => "royal-weapon-fire",
                    WeaponState::Recoil => "royal-weapon-recoil",
                };

                self.get(key)
            }
        }
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

    // --- Bloque 2, Commit 16: sprites de The Royal Flush. ---

    #[test]
    fn standard_tier_still_selects_the_themed_base_weapon_sprites() {
        let mut manager = TextureManager::new();
        manager
            .load_weapon_textures()
            .expect("arma Standard debe cargar");

        for theme in TextureManager::THEMES {
            for state in [WeaponState::Idle, WeaponState::Fire, WeaponState::Recoil] {
                let standard = manager
                    .themed_weapon_texture(state, theme, WeaponTier::Standard)
                    .expect("el arma Standard debe resolver en todos los temas");

                // Es exactamente la variante temática de la clave base.
                let expected_key = match state {
                    WeaponState::Fire => "weapon-fire",
                    WeaponState::Recoil => "weapon-recoil",
                    _ => "weapon-idle",
                };
                let expected = manager
                    .get(&TextureManager::themed_key(expected_key, theme))
                    .unwrap();
                assert_eq!(standard.path, expected.path);
            }
        }
    }

    #[test]
    fn royal_flush_tier_selects_dedicated_non_themed_sprites() {
        let mut manager = TextureManager::new();
        manager
            .load_weapon_textures()
            .expect("arma Standard debe cargar");
        manager
            .load_royal_weapon_textures()
            .expect("The Royal Flush debe cargar");

        // El mismo sprite dorado en los tres temas: paleta congelada,
        // nunca recoloreada por nivel.
        let idle_crimson = manager
            .themed_weapon_texture(
                WeaponState::Idle,
                LevelTheme::CrimsonEntrance,
                WeaponTier::RoyalFlush,
            )
            .expect("royal idle debe resolver");
        let idle_house = manager
            .themed_weapon_texture(
                WeaponState::Idle,
                LevelTheme::HouseOfCards,
                WeaponTier::RoyalFlush,
            )
            .expect("royal idle debe resolver");

        assert_eq!(idle_crimson.path, idle_house.path);
        assert!(idle_crimson.path.contains("royal_weapon_idle"));

        // Reload reutiliza el sprite idle, igual que el arma Standard.
        let reload = manager
            .themed_weapon_texture(
                WeaponState::Reload,
                LevelTheme::BlackClub,
                WeaponTier::RoyalFlush,
            )
            .expect("royal reload debe resolver al idle");
        assert_eq!(reload.path, idle_crimson.path);

        // Fire y Recoil tienen sprite propio.
        let fire = manager
            .themed_weapon_texture(
                WeaponState::Fire,
                LevelTheme::BlackClub,
                WeaponTier::RoyalFlush,
            )
            .unwrap();
        assert!(fire.path.contains("royal_weapon_fire"));
    }

    #[test]
    fn royal_flush_world_pickup_texture_loads_and_is_shared_across_levels() {
        let mut manager = TextureManager::new();
        assert!(manager.royal_flush_pickup_texture().is_none());

        manager
            .load_royal_flush_pickup_texture()
            .expect("el billboard de The Royal Flush debe cargar");

        assert!(manager.royal_flush_pickup_texture().is_some());
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
    fn crimson_variant_still_matches_the_base_pixel_for_pixel_after_task_41() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        let base = manager.get("wall-club").expect("base debe existir");

        // Tarea 41 solo hizo divergir a House of Cards: Crimson
        // Entrance es ahora el ÚNICO tema que debe seguir generando
        // una variante IDÉNTICA píxel a píxel al asset base —
        // ninguna regresión visual para él.
        let variant = manager
            .themed_wall_texture('|', LevelTheme::CrimsonEntrance)
            .expect("la variante Crimson Entrance debe existir");

        assert_eq!(base.width(), variant.width());
        assert_eq!(base.height(), variant.height());

        for y in 0..base.height() {
            for x in 0..base.width() {
                assert_eq!(
                    base.pixel_at(x, y),
                    variant.pixel_at(x, y),
                    "diverge en ({x}, {y})"
                );
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
    fn house_of_cards_wall_variant_differs_from_the_legacy_red_base() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        let base = manager.get("wall-club").expect("base debe existir");

        let variant = manager
            .themed_wall_texture('|', LevelTheme::HouseOfCards)
            .expect("la variante House of Cards debe existir");

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
            "la variante House of Cards debe diferir del rojo heredado en al menos un píxel"
        );
    }

    #[test]
    fn house_of_cards_wall_variant_maps_the_canonical_red_to_the_exact_reference_violet() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        let base = manager.get("wall-club").expect("base debe existir");

        let variant = manager
            .themed_wall_texture('|', LevelTheme::HouseOfCards)
            .expect("la variante House of Cards debe existir");

        let mut checked_a_canonical_red_pixel = false;

        for y in 0..base.height() {
            for x in 0..base.width() {
                if base.pixel_at(x, y) == Some(Color::new(210, 31, 43, 255)) {
                    checked_a_canonical_red_pixel = true;

                    assert_eq!(
                        variant.pixel_at(x, y),
                        Some(Color::new(0xC1, 0x3C, 0xFF, 255))
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
    fn all_three_theme_variants_of_the_same_asset_coexist_without_collision() {
        let mut manager = TextureManager::new();

        manager
            .load_wall_textures()
            .expect("las texturas de pared del proyecto deben cargar");

        let base = manager.get("wall-club").expect("base debe existir");

        // Localizamos un píxel real de club.png que sea el rojo
        // canónico brillante auditado en Tarea 39.B, y leemos ESA
        // MISMA coordenada bajo las tres variantes cacheadas: cada
        // tema debe devolver su propio color de acento (rojo/
        // naranja/violeta), sin que uno sobrescriba al otro en el
        // `HashMap` compartido por una colisión de clave.
        let accent_coordinate = (0..base.height())
            .flat_map(|y| (0..base.width()).map(move |x| (x, y)))
            .find(|&(x, y)| base.pixel_at(x, y) == Some(Color::new(210, 31, 43, 255)))
            .expect("club.png debe contener el rojo canónico auditado en Tarea 39.B");

        let (x, y) = accent_coordinate;

        let crimson = manager
            .themed_wall_texture('|', LevelTheme::CrimsonEntrance)
            .and_then(|texture| texture.pixel_at(x, y));

        let black_club = manager
            .themed_wall_texture('|', LevelTheme::BlackClub)
            .and_then(|texture| texture.pixel_at(x, y));

        let house = manager
            .themed_wall_texture('|', LevelTheme::HouseOfCards)
            .and_then(|texture| texture.pixel_at(x, y));

        assert_eq!(crimson, Some(Color::new(210, 31, 43, 255)));
        assert_eq!(black_club, Some(Color::new(0xFF, 0x7A, 0x00, 255)));
        assert_eq!(house, Some(Color::new(0xC1, 0x3C, 0xFF, 255)));

        assert_ne!(crimson, black_club);
        assert_ne!(crimson, house);
        assert_ne!(black_club, house);
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

    // --- Tarea 44: pickup de munición. ---

    #[test]
    fn loading_ammo_pickup_texture_generates_a_lookup_hit_for_every_theme() {
        let mut manager = TextureManager::new();

        manager
            .load_ammo_pickup_texture()
            .expect("la textura del pickup de munición del proyecto debe cargar");

        for theme in TextureManager::THEMES {
            assert!(
                manager.themed_ammo_pickup_texture(theme).is_some(),
                "falta la variante temática {theme:?} del pickup de munición"
            );
        }
    }

    #[test]
    fn ammo_pickup_variant_maps_the_canonical_red_to_the_exact_reference_orange() {
        let mut manager = TextureManager::new();

        manager
            .load_ammo_pickup_texture()
            .expect("la textura del pickup de munición del proyecto debe cargar");

        let base = manager
            .get(AMMO_PICKUP_TEXTURE_KEY)
            .expect("base debe existir");

        let variant = manager
            .themed_ammo_pickup_texture(LevelTheme::BlackClub)
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
            "ammo_pickup.png debe usar el rojo canónico bright de LEGACY_ACCENT_TABLE"
        );
    }

    #[test]
    fn ammo_pickup_variant_maps_the_canonical_red_to_the_exact_reference_violet() {
        let mut manager = TextureManager::new();

        manager
            .load_ammo_pickup_texture()
            .expect("la textura del pickup de munición del proyecto debe cargar");

        let base = manager
            .get(AMMO_PICKUP_TEXTURE_KEY)
            .expect("base debe existir");

        let variant = manager
            .themed_ammo_pickup_texture(LevelTheme::HouseOfCards)
            .expect("la variante House of Cards debe existir");

        let mut checked_a_canonical_red_pixel = false;

        for y in 0..base.height() {
            for x in 0..base.width() {
                if base.pixel_at(x, y) == Some(Color::new(210, 31, 43, 255)) {
                    checked_a_canonical_red_pixel = true;

                    assert_eq!(
                        variant.pixel_at(x, y),
                        Some(Color::new(0xC1, 0x3C, 0xFF, 255))
                    );
                }
            }
        }

        assert!(checked_a_canonical_red_pixel);
    }

    #[test]
    fn ammo_pickup_crimson_variant_matches_the_base_pixel_for_pixel() {
        let mut manager = TextureManager::new();

        manager
            .load_ammo_pickup_texture()
            .expect("la textura del pickup de munición del proyecto debe cargar");

        let base = manager
            .get(AMMO_PICKUP_TEXTURE_KEY)
            .expect("base debe existir");

        let variant = manager
            .themed_ammo_pickup_texture(LevelTheme::CrimsonEntrance)
            .expect("la variante Crimson Entrance debe existir");

        for y in 0..base.height() {
            for x in 0..base.width() {
                assert_eq!(base.pixel_at(x, y), variant.pixel_at(x, y));
            }
        }
    }

    #[test]
    fn ammo_pickup_ivory_highlight_and_transparency_are_preserved_across_themes() {
        let base_image = Image::load_image(AMMO_PICKUP_TEXTURE_PATH)
            .expect("el asset del pickup de munición del proyecto debe cargar");

        let mut saw_ivory = false;

        let mut saw_transparent = false;

        for theme in [
            LevelTheme::CrimsonEntrance,
            LevelTheme::BlackClub,
            LevelTheme::HouseOfCards,
        ] {
            let palette = palette_for_theme(theme);

            let remapped = remap_image_accent(&base_image, &palette);

            for y in 0..base_image.height() {
                for x in 0..base_image.width() {
                    let original = base_image.get_color(x, y);

                    if original == Color::new(214, 208, 196, 255) {
                        saw_ivory = true;

                        assert_eq!(remapped.get_color(x, y), original);
                    }

                    if original.a == 0 {
                        saw_transparent = true;

                        assert_eq!(remapped.get_color(x, y).a, 0);
                    }
                }
            }
        }

        assert!(
            saw_ivory,
            "ammo_pickup.png debe contener el marfil neutro del proyecto"
        );
        assert!(saw_transparent);
    }

    // --- Health Pickup: textura del corazón, con variantes por tema. ---

    #[test]
    fn loading_health_pickup_texture_generates_a_lookup_hit_for_every_theme() {
        let mut manager = TextureManager::new();

        manager
            .load_health_pickup_texture()
            .expect("la textura del pickup de vida del proyecto debe cargar");

        for theme in TextureManager::THEMES {
            assert!(
                manager.themed_health_pickup_texture(theme).is_some(),
                "falta la variante temática {theme:?} del pickup de vida"
            );
        }
    }

    #[test]
    fn health_pickup_variant_maps_the_canonical_red_to_the_exact_reference_orange() {
        let mut manager = TextureManager::new();

        manager
            .load_health_pickup_texture()
            .expect("la textura del pickup de vida del proyecto debe cargar");

        let base = manager
            .get(HEALTH_PICKUP_TEXTURE_KEY)
            .expect("base debe existir");

        let variant = manager
            .themed_health_pickup_texture(LevelTheme::BlackClub)
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
            "health_pickup.png debe usar el rojo canónico bright de LEGACY_ACCENT_TABLE"
        );
    }

    #[test]
    fn health_pickup_variant_maps_the_canonical_red_to_the_exact_reference_violet() {
        let mut manager = TextureManager::new();

        manager
            .load_health_pickup_texture()
            .expect("la textura del pickup de vida del proyecto debe cargar");

        let base = manager
            .get(HEALTH_PICKUP_TEXTURE_KEY)
            .expect("base debe existir");

        let variant = manager
            .themed_health_pickup_texture(LevelTheme::HouseOfCards)
            .expect("la variante House of Cards debe existir");

        let mut checked_a_canonical_red_pixel = false;

        for y in 0..base.height() {
            for x in 0..base.width() {
                if base.pixel_at(x, y) == Some(Color::new(210, 31, 43, 255)) {
                    checked_a_canonical_red_pixel = true;

                    assert_eq!(
                        variant.pixel_at(x, y),
                        Some(Color::new(0xC1, 0x3C, 0xFF, 255))
                    );
                }
            }
        }

        assert!(checked_a_canonical_red_pixel);
    }

    #[test]
    fn health_pickup_crimson_variant_matches_the_base_pixel_for_pixel() {
        let mut manager = TextureManager::new();

        manager
            .load_health_pickup_texture()
            .expect("la textura del pickup de vida del proyecto debe cargar");

        let base = manager
            .get(HEALTH_PICKUP_TEXTURE_KEY)
            .expect("base debe existir");

        let variant = manager
            .themed_health_pickup_texture(LevelTheme::CrimsonEntrance)
            .expect("la variante Crimson Entrance debe existir");

        for y in 0..base.height() {
            for x in 0..base.width() {
                assert_eq!(base.pixel_at(x, y), variant.pixel_at(x, y));
            }
        }
    }

    #[test]
    fn health_pickup_outline_and_highlight_are_preserved_across_themes() {
        let base_image = Image::load_image(HEALTH_PICKUP_TEXTURE_PATH)
            .expect("el asset del pickup de vida del proyecto debe cargar");

        let mut saw_highlight = false;

        let mut saw_transparent = false;

        for theme in [
            LevelTheme::CrimsonEntrance,
            LevelTheme::BlackClub,
            LevelTheme::HouseOfCards,
        ] {
            let palette = palette_for_theme(theme);

            let remapped = remap_image_accent(&base_image, &palette);

            for y in 0..base_image.height() {
                for x in 0..base_image.width() {
                    let original = base_image.get_color(x, y);

                    if original == Color::new(214, 208, 196, 255) {
                        saw_highlight = true;

                        assert_eq!(remapped.get_color(x, y), original);
                    }

                    if original.a == 0 {
                        saw_transparent = true;

                        assert_eq!(remapped.get_color(x, y).a, 0);
                    }
                }
            }
        }

        assert!(
            saw_highlight,
            "health_pickup.png debe contener el brillo marfil neutro del proyecto"
        );
        assert!(saw_transparent);
    }
}
