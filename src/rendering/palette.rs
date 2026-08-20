use crate::world::LevelTheme;
use raylib::prelude::Color;

/// Traducción ÚNICA de la identidad semántica de un nivel
/// (`LevelTheme`, en `world::level_manager`) hacia los colores de
/// acento concretos que el renderer necesita.
///
/// `world` no conoce `raylib::Color`; esa traducción vive
/// exclusivamente aquí, en la capa de rendering — igual que
/// `rendering::background::palette_for_theme` ya hace desde Tarea 34
/// para el cielo/suelo, con el que este módulo coexiste
/// deliberadamente en vez de reemplazarlo: el degradado de
/// cielo/suelo tiene paradas y patrones propios de esa vista que no
/// son directamente reutilizables como matices de acento genéricos.
/// Este módulo es, en cambio, la fuente de verdad para TODOS los
/// demás consumidores del acento del nivel: el respaldo de pared sin
/// textura (`world_3d::wall_color`, desde Tarea 39), las variantes
/// temáticas de textura (`textures::TextureManager`, Tarea 39.B) y
/// los acentos directos de HUD/minimapa (Tarea 39.B).
///
/// Solo expone los campos que YA tienen un consumidor real: Tarea
/// 39/39.B son infraestructura, no una reescritura especulativa.
pub(crate) struct ThemePalette {
    /// Matiz (verde, azul) del respaldo de pared vertical (`'|'`).
    ///
    /// El canal rojo de este `Color` NUNCA se lee: `wall_color`
    /// sigue calculándolo dinámicamente según la distancia, exacto
    /// como antes de Tarea 39. Es un valor no significativo
    /// (`0`), nunca consumido — solo se usan `.g`/`.b`.
    pub(crate) accent_vertical: Color,

    /// Matiz (verde, azul) del respaldo de pared horizontal (`'-'`).
    pub(crate) accent_horizontal: Color,

    /// Matiz (verde, azul) del respaldo de esquina (`'+'`).
    pub(crate) accent_corner: Color,

    /// Matiz (verde, azul) del respaldo de pared de carácter
    /// alternativo (`'#'`).
    pub(crate) accent_alt: Color,

    /// Tono de acento "brillante" canónico de la identidad del
    /// nivel. Consumido por `textures::remap_image_accent` (Tarea
    /// 39.B) para generar las variantes temáticas de textura
    /// (paredes con textura, arma, Dealer, antorchas, meta): todo
    /// píxel de un asset base que coincida EXACTAMENTE con una de
    /// las entradas "brillantes" de `LEGACY_ACCENT_TABLE` se
    /// reemplaza por este color.
    pub(crate) accent_bright: Color,

    /// Tono de acento "medio" canónico. Ver `accent_bright`.
    pub(crate) accent_medium: Color,

    /// Tono de acento "oscuro" canónico. Ver `accent_bright`.
    pub(crate) accent_dark: Color,

    /// Acento propio del HUD (corazón/diamante y sus dígitos).
    ///
    /// Deliberadamente SEPARADO de `accent_bright`: el HUD ya tenía
    /// su propio rojo afinado a mano (`205, 30, 42`), ligeramente
    /// distinto del canónico de paredes/portal/antorchas
    /// (`210, 31, 43`). Unificarlos habría alterado el HUD en
    /// Crimson Entrance por un valor imperceptible pero real; Tarea
    /// 39.B preserva el valor exacto en vez de forzar una
    /// convergencia no solicitada.
    pub(crate) hud_accent: Color,

    /// Acento del borde de la caja del minimapa.
    pub(crate) minimap_border_accent: Color,

    /// Acento de las celdas de pared dibujadas dentro del minimapa.
    pub(crate) minimap_wall_accent: Color,
}

/// Nivel de intensidad de un color de acento "legado" encontrado en
/// los assets PNG actuales.
///
/// Corresponde 1:1 a `ThemePalette::accent_bright/medium/dark`: cada
/// entrada de `LEGACY_ACCENT_TABLE` se etiqueta con el nivel al que
/// debe remapearse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccentTier {
    Bright,
    Medium,
    Dark,
}

/// Tabla EXPLÍCITA y determinista de los colores de acento rojo
/// realmente auditados píxel-por-píxel en los assets PNG actuales
/// (paredes ♣/♦/♥/♠, portal, antorchas, Dealer, arma — Tarea 39.B),
/// cada uno asociado al nivel de intensidad semántico que representa
/// dentro de su propia familia de recursos.
///
/// Coincidencia EXACTA únicamente (sin rotación de matiz ni
/// heurística de saturación/luminancia, exactamente como pide la
/// tarea): cualquier píxel gris, negro, marfil o transparente que no
/// aparezca aquí se conserva SIN CAMBIOS por `remap_accent_pixel`.
///
/// Es intencional que esta tabla no cubra absolutamente todos los
/// rojos posibles de cada asset — cubre los tonos que recurren de
/// forma consistente dentro de cada familia (paredes/portal/
/// antorchas comparten literalmente los mismos tres tonos; Dealer y
/// arma tienen cada uno su propio par/trío afinado a mano). Tarea
/// 40/41 puede ampliar esta tabla si algún tono adicional
/// necesitara también recolorearse.
const LEGACY_ACCENT_TABLE: [(Color, AccentTier); 11] = [
    // Paredes con textura (club/diamond/heart/spade) + portal + antorchas.
    (Color::new(210, 31, 43, 255), AccentTier::Bright),
    (Color::new(122, 16, 24, 255), AccentTier::Medium),
    (Color::new(74, 11, 16, 255), AccentTier::Dark),
    (Color::new(143, 16, 24, 255), AccentTier::Medium),
    (Color::new(90, 9, 13, 255), AccentTier::Dark),
    (Color::new(58, 7, 10, 255), AccentTier::Dark),
    // Dealer (idle/alert/hit/dead).
    (Color::new(200, 30, 40, 255), AccentTier::Bright),
    (Color::new(150, 18, 26, 255), AccentTier::Medium),
    (Color::new(94, 10, 16, 255), AccentTier::Dark),
    // Arma (idle/fire/recoil).
    (Color::new(176, 24, 30, 255), AccentTier::Bright),
    (Color::new(110, 14, 20, 255), AccentTier::Dark),
];

/// Resuelve el `ThemePalette` de acento para `theme`.
///
/// Tarea 40: `BlackClub` diverge por primera vez de la familia roja
/// heredada, hacia negro + naranja neón. `CrimsonEntrance` y
/// `HouseOfCards` (todavía pendiente de Tarea 41) permanecen en el
/// brazo legacy compartido, con los mismos valores que TODOS los
/// consumidores usaban de forma incondicional antes de Tarea 39.
pub(crate) fn palette_for_theme(theme: LevelTheme) -> ThemePalette {
    match theme {
        LevelTheme::CrimsonEntrance | LevelTheme::HouseOfCards => ThemePalette {
            accent_vertical: Color::new(0, 25, 30, 255),
            accent_horizontal: Color::new(0, 18, 24, 255),
            accent_corner: Color::new(0, 12, 18, 255),
            accent_alt: Color::new(0, 20, 25, 255),

            accent_bright: Color::new(210, 31, 43, 255),
            accent_medium: Color::new(122, 16, 24, 255),
            accent_dark: Color::new(74, 11, 16, 255),

            hud_accent: Color::new(205, 30, 42, 255),

            minimap_border_accent: Color::new(120, 18, 24, 255),
            minimap_wall_accent: Color::new(150, 24, 32, 255),
        },

        /*
         * Tarea 40: identidad negro + naranja neón. Los tres tonos
         * canónicos (`accent_bright/medium/dark`) son EXACTAMENTE
         * los de referencia del Plan Maestro (`#FF7A00`/`#E94F00`/
         * `#A53600`); el resto de roles (respaldo de pared, HUD,
         * minimapa) derivan su matiz (G, B) de esos mismos tres
         * tonos, siguiendo el mismo patrón que ya usaba la familia
         * roja heredada (sus propios G/B eran, a su vez, una
         * aproximación de `accent_bright/medium/dark`) — ninguna
         * familia cromática nueva, ninguna heurística.
         */
        LevelTheme::BlackClub => ThemePalette {
            // (bright, medium, corner/dark, alt) — mismo patrón que
            // el brazo legacy: G/B tomados de accent_bright/medium/
            // dark respectivamente, con `accent_alt` ligeramente
            // distinto de `accent_horizontal` (igual que la familia
            // roja distinguía spade de diamond).
            accent_vertical: Color::new(0, 122, 0, 255),
            accent_horizontal: Color::new(0, 79, 0, 255),
            accent_corner: Color::new(0, 54, 0, 255),
            accent_alt: Color::new(0, 90, 0, 255),

            accent_bright: Color::new(0xFF, 0x7A, 0x00, 255),
            accent_medium: Color::new(0xE9, 0x4F, 0x00, 255),
            accent_dark: Color::new(0xA5, 0x36, 0x00, 255),

            // Igual que `accent_bright`: no hay razón para separar
            // el acento del HUD del canónico en una paleta nueva (a
            // diferencia del rojo legacy, donde el valor histórico
            // ya divergía y había que preservarlo).
            hud_accent: Color::new(0xFF, 0x7A, 0x00, 255),

            minimap_border_accent: Color::new(0xA5, 0x36, 0x00, 255),
            minimap_wall_accent: Color::new(0xE9, 0x4F, 0x00, 255),
        },
    }
}

/// Clasifica `color` según `LEGACY_ACCENT_TABLE`, comparando
/// únicamente los canales RGB (el alfa se ignora deliberadamente:
/// un píxel de acento parcialmente transparente sigue siendo un
/// píxel de acento). Retorna `None` si `color` no coincide con
/// ninguna entrada — el caso de todo píxel neutro (gris, negro,
/// marfil) o transparente.
fn classify_legacy_accent(color: Color) -> Option<AccentTier> {
    LEGACY_ACCENT_TABLE
        .iter()
        .find(|(legacy, _)| legacy.r == color.r && legacy.g == color.g && legacy.b == color.b)
        .map(|(_, tier)| *tier)
}

/// Remapea un único píxel `color` según `LEGACY_ACCENT_TABLE` hacia
/// el `ThemePalette` de destino.
///
/// Preserva EXACTAMENTE el canal alfa original: un píxel totalmente
/// transparente (`a == 0`) permanece invisible sin importar su RGB,
/// y un píxel semitransparente conserva su transparencia. Cualquier
/// píxel que no coincida con la tabla (grises, negro, marfil,
/// transparente, o cualquier tono fuera de la familia de acento
/// auditada) se retorna SIN CAMBIOS — nunca se aplica una
/// transformación global tipo rotación de matiz.
pub(crate) fn remap_accent_pixel(color: Color, palette: &ThemePalette) -> Color {
    let Some(tier) = classify_legacy_accent(color) else {
        return color;
    };

    let target = match tier {
        AccentTier::Bright => palette.accent_bright,
        AccentTier::Medium => palette.accent_medium,
        AccentTier::Dark => palette.accent_dark,
    };

    Color::new(target.r, target.g, target.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_selection_is_deterministic() {
        let first = palette_for_theme(LevelTheme::CrimsonEntrance);
        let second = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(
            (
                first.accent_vertical,
                first.accent_horizontal,
                first.accent_corner,
                first.accent_alt
            ),
            (
                second.accent_vertical,
                second.accent_horizontal,
                second.accent_corner,
                second.accent_alt
            )
        );
    }

    #[test]
    fn crimson_and_house_still_preserve_the_exact_legacy_red_after_task_40() {
        // Tarea 40 solo debe hacer que `BlackClub` diverja; Crimson
        // Entrance y House of Cards (todavía pendiente de Tarea 41)
        // deben seguir resolviendo EXACTAMENTE los mismos valores
        // heredados que antes.
        for theme in [LevelTheme::CrimsonEntrance, LevelTheme::HouseOfCards] {
            let palette = palette_for_theme(theme);

            assert_eq!(palette.accent_vertical, Color::new(0, 25, 30, 255));
            assert_eq!(palette.accent_horizontal, Color::new(0, 18, 24, 255));
            assert_eq!(palette.accent_corner, Color::new(0, 12, 18, 255));
            assert_eq!(palette.accent_alt, Color::new(0, 20, 25, 255));

            assert_eq!(palette.accent_bright, Color::new(210, 31, 43, 255));
            assert_eq!(palette.accent_medium, Color::new(122, 16, 24, 255));
            assert_eq!(palette.accent_dark, Color::new(74, 11, 16, 255));

            assert_eq!(palette.hud_accent, Color::new(205, 30, 42, 255));

            assert_eq!(palette.minimap_border_accent, Color::new(120, 18, 24, 255));
            assert_eq!(palette.minimap_wall_accent, Color::new(150, 24, 32, 255));
        }
    }

    #[test]
    fn black_club_uses_the_neon_orange_reference_palette() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_eq!(palette.accent_bright, Color::new(0xFF, 0x7A, 0x00, 255));
        assert_eq!(palette.accent_medium, Color::new(0xE9, 0x4F, 0x00, 255));
        assert_eq!(palette.accent_dark, Color::new(0xA5, 0x36, 0x00, 255));
    }

    #[test]
    fn black_club_no_longer_uses_any_red_accent_value() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_ne!(palette.accent_bright, Color::new(210, 31, 43, 255));
        assert_ne!(palette.accent_medium, Color::new(122, 16, 24, 255));
        assert_ne!(palette.accent_dark, Color::new(74, 11, 16, 255));
        assert_ne!(palette.hud_accent, Color::new(205, 30, 42, 255));
        assert_ne!(palette.minimap_border_accent, Color::new(120, 18, 24, 255));
        assert_ne!(palette.minimap_wall_accent, Color::new(150, 24, 32, 255));
    }

    #[test]
    fn black_club_hud_and_minimap_accents_belong_to_the_orange_family() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        // "Naranja" en este proyecto significa: rojo (R) claramente
        // dominante sobre azul (B ~ 0), con verde (G) intermedio —
        // exactamente la relación de FF7A00/E94F00/A53600. Probar
        // esta relación (no solo la igualdad con una constante)
        // confirma que HUD/minimapa realmente HEREDAN la familia
        // naranja, no que coincidan con un literal aislado.
        for color in [
            palette.hud_accent,
            palette.minimap_border_accent,
            palette.minimap_wall_accent,
        ] {
            assert_eq!(color.b, 0, "el azul debe ser cero en la familia naranja");
            assert!(color.r > color.g, "el rojo debe dominar sobre el verde");
            assert!(
                color.g > 0,
                "el verde debe ser distinto de cero (no es rojo puro)"
            );
        }
    }

    #[test]
    fn bright_legacy_accent_maps_to_the_palette_bright_role() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let remapped = remap_accent_pixel(Color::new(210, 31, 43, 255), &palette);

        assert_eq!(remapped, palette.accent_bright);
    }

    #[test]
    fn medium_legacy_accent_maps_to_the_palette_medium_role() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let remapped = remap_accent_pixel(Color::new(122, 16, 24, 255), &palette);

        assert_eq!(remapped, palette.accent_medium);
    }

    #[test]
    fn dark_legacy_accent_maps_to_the_palette_dark_role() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let remapped = remap_accent_pixel(Color::new(74, 11, 16, 255), &palette);

        assert_eq!(remapped, palette.accent_dark);
    }

    #[test]
    fn dealer_and_weapon_specific_accent_tones_are_also_classified() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        assert_eq!(
            remap_accent_pixel(Color::new(200, 30, 40, 255), &palette),
            palette.accent_bright
        );

        assert_eq!(
            remap_accent_pixel(Color::new(94, 10, 16, 255), &palette),
            palette.accent_dark
        );

        assert_eq!(
            remap_accent_pixel(Color::new(176, 24, 30, 255), &palette),
            palette.accent_bright
        );
    }

    #[test]
    fn black_is_preserved_unchanged() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let black = Color::new(5, 5, 5, 255);

        assert_eq!(remap_accent_pixel(black, &palette), black);
    }

    #[test]
    fn ivory_is_preserved_unchanged() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let ivory = Color::new(214, 208, 196, 255);

        assert_eq!(remap_accent_pixel(ivory, &palette), ivory);
    }

    #[test]
    fn fully_transparent_pixel_keeps_its_alpha() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let transparent = Color::new(0, 0, 0, 0);

        let remapped = remap_accent_pixel(transparent, &palette);

        assert_eq!(remapped.a, 0);
    }

    #[test]
    fn partially_transparent_accent_pixel_preserves_its_alpha() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        let semi_transparent_accent = Color::new(210, 31, 43, 128);

        let remapped = remap_accent_pixel(semi_transparent_accent, &palette);

        assert_eq!(remapped.a, 128);
        assert_eq!((remapped.r, remapped.g, remapped.b), (210, 31, 43));
    }

    #[test]
    fn unrelated_gray_is_not_destroyed() {
        let palette = palette_for_theme(LevelTheme::CrimsonEntrance);

        // Gris de sombreado de pared/arma, no perteneciente a
        // ninguna familia de acento.
        let gray = Color::new(40, 40, 46, 255);

        assert_eq!(remap_accent_pixel(gray, &palette), gray);
    }

    #[test]
    fn black_club_remaps_legacy_accent_tones_to_the_exact_reference_orange() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        assert_eq!(
            remap_accent_pixel(Color::new(210, 31, 43, 255), &palette),
            Color::new(0xFF, 0x7A, 0x00, 255)
        );

        assert_eq!(
            remap_accent_pixel(Color::new(122, 16, 24, 255), &palette),
            Color::new(0xE9, 0x4F, 0x00, 255)
        );

        assert_eq!(
            remap_accent_pixel(Color::new(74, 11, 16, 255), &palette),
            Color::new(0xA5, 0x36, 0x00, 255)
        );
    }

    #[test]
    fn black_club_remap_still_preserves_black_ivory_and_alpha() {
        let palette = palette_for_theme(LevelTheme::BlackClub);

        let black = Color::new(5, 5, 5, 255);
        let ivory = Color::new(214, 208, 196, 255);
        let transparent = Color::new(0, 0, 0, 0);

        assert_eq!(remap_accent_pixel(black, &palette), black);
        assert_eq!(remap_accent_pixel(ivory, &palette), ivory);
        assert_eq!(remap_accent_pixel(transparent, &palette).a, 0);
    }
}
