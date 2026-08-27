/// Modo de juego elegido para una sesión.
///
/// `Portal` es el juego tradicional ya existente (portal activo, sin
/// progresión de Dealer Hands); `Horde` reemplaza el portal por la
/// progresión de Hands. Se introduce como tipo explícito, en vez de
/// banderas booleanas (`is_horde`, `portal_enabled`, ...), para que
/// cada sistema que dependa del modo consulte una única fuente de
/// verdad en vez de varias condiciones dispersas que podrían divergir
/// entre sí.
///
/// Este tipo todavía no se propaga a `GameSession` ni a ningún otro
/// sistema — eso llega en un commit posterior. Por ahora solo existe
/// como concepto de dominio, preparado para que la selección de nivel
/// y la sesión puedan adoptarlo sin cambiar su forma otra vez.
/// `#[allow(dead_code)]`: este commit únicamente introduce el tipo —
/// todavía no lo consume ninguna pantalla ni `GameSession`. El
/// siguiente commit (selección de modo en Level Select) elimina esta
/// anotación al empezar a usarlo de verdad.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GameMode {
    Portal,
    Horde,
}

#[allow(dead_code)]
impl GameMode {
    /// El otro modo. Con solo dos variantes, "anterior" y "siguiente"
    /// son la misma alternancia — mismo patrón ya establecido por
    /// `PauseMenuItem`/`DefeatMenuItem` para un selector de dos
    /// opciones navegable en ambas direcciones.
    pub(crate) fn toggled(self) -> Self {
        match self {
            GameMode::Portal => GameMode::Horde,
            GameMode::Horde => GameMode::Portal,
        }
    }

    /// Etiqueta de presentación en mayúsculas, para la futura UI de
    /// selección de modo.
    pub(crate) fn label(self) -> &'static str {
        match self {
            GameMode::Portal => "PORTAL",
            GameMode::Horde => "HORDE",
        }
    }
}

impl Default for GameMode {
    /// Portal Mode es el juego tradicional ya existente: el valor por
    /// defecto antes de que el jugador elija explícitamente un modo.
    fn default() -> Self {
        GameMode::Portal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_portal() {
        assert_eq!(GameMode::default(), GameMode::Portal);
    }

    #[test]
    fn toggled_alternates_between_the_two_modes() {
        assert_eq!(GameMode::Portal.toggled(), GameMode::Horde);
        assert_eq!(GameMode::Horde.toggled(), GameMode::Portal);
    }

    #[test]
    fn toggled_twice_returns_to_the_original_mode() {
        assert_eq!(GameMode::Portal.toggled().toggled(), GameMode::Portal);
        assert_eq!(GameMode::Horde.toggled().toggled(), GameMode::Horde);
    }

    #[test]
    fn labels_are_distinct_and_uppercase() {
        assert_eq!(GameMode::Portal.label(), "PORTAL");
        assert_eq!(GameMode::Horde.label(), "HORDE");

        assert_ne!(GameMode::Portal.label(), GameMode::Horde.label());
    }
}
