/// Estados de alto nivel de la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Welcome,
    LevelSelect,
    Playing,
    Victory,

    /// Tarea 42: la MISMA sesión de `Playing` congelada exactamente
    /// donde estaba — nunca destruye/recarga `GameSession`. Solo se
    /// alcanza desde `Playing` (tecla ESC) y solo regresa a
    /// `Playing` (ESC/CONTINUE) o a `Welcome` (EXIT TO MENU).
    Paused,

    /// Tarea 46: la vida del jugador llegó a `0` durante `Playing`.
    /// A diferencia de `Paused`, no es una simple congelación —
    /// `App` no vuelve a llamar `update_playing` sobre la sesión
    /// muerta jamás mientras este estado siga activo (ni siquiera al
    /// reanudar): la única forma de volver a jugar es `Retry`, que
    /// reconstruye una `GameSession` enteramente NUEVA para el mismo
    /// nivel (igual que ya hace `VictoryAction::Retry`), o
    /// `MainMenu`, que regresa a `Welcome` sin tocar la sesión
    /// muerta.
    Defeat,
}

impl GameState {
    /// Regla pura de transición ante la meta alcanzada: únicamente
    /// `Playing` con `reached_goal == true` avanza a `Victory`.
    /// Cualquier otro estado (incluyendo `Victory` ya activo)
    /// permanece sin cambios, incluso si `reached_goal` es
    /// verdadero: la condición de meta solo tiene efecto mientras la
    /// partida está activa.
    ///
    /// Dominio puro: no conoce `VictoryScreen`, audio, ni ningún
    /// otro efecto secundario. `App` es quien decide QUÉ hacer
    /// además de la transición (reiniciar la selección de Victoria,
    /// reproducir el efecto de sonido); esta función decide
    /// ÚNICAMENTE el próximo `GameState`.
    pub fn after_goal_check(self, reached_goal: bool) -> Self {
        match (self, reached_goal) {
            (GameState::Playing, true) => GameState::Victory,
            _ => self,
        }
    }

    /// Regla pura de transición ante la vida del jugador (Tarea 46):
    /// únicamente `Playing` con `health <= 0` avanza a `Defeat`.
    /// Cualquier otro estado permanece sin cambios, y `health > 0`
    /// nunca produce una transición (nunca un estado "Critical"
    /// intermedio por vida baja).
    ///
    /// Dominio puro, mismo estilo que `after_goal_check`: no conoce
    /// `DefeatScreen`, audio, ni ningún otro efecto secundario.
    pub fn after_health_check(self, health: i32) -> Self {
        match (self, health <= 0) {
            (GameState::Playing, true) => GameState::Defeat,
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playing_transitions_to_victory_only_when_goal_is_reached() {
        assert_eq!(
            GameState::Playing.after_goal_check(true),
            GameState::Victory
        );
        assert_eq!(
            GameState::Playing.after_goal_check(false),
            GameState::Playing
        );
    }

    #[test]
    fn non_playing_states_never_transition_from_the_goal_condition_alone() {
        assert_eq!(
            GameState::Welcome.after_goal_check(true),
            GameState::Welcome
        );
        assert_eq!(
            GameState::LevelSelect.after_goal_check(true),
            GameState::LevelSelect
        );
        assert_eq!(
            GameState::Victory.after_goal_check(true),
            GameState::Victory
        );
        assert_eq!(GameState::Paused.after_goal_check(true), GameState::Paused);
    }

    // --- Tarea 46: GameState::after_health_check ---

    #[test]
    fn playing_transitions_to_defeat_only_when_health_reaches_zero() {
        assert_eq!(GameState::Playing.after_health_check(0), GameState::Defeat);
        assert_eq!(GameState::Playing.after_health_check(1), GameState::Playing);
        assert_eq!(
            GameState::Playing.after_health_check(100),
            GameState::Playing
        );
    }

    #[test]
    fn non_playing_states_never_transition_from_the_health_condition_alone() {
        assert_eq!(GameState::Welcome.after_health_check(0), GameState::Welcome);
        assert_eq!(
            GameState::LevelSelect.after_health_check(0),
            GameState::LevelSelect
        );
        assert_eq!(GameState::Victory.after_health_check(0), GameState::Victory);
        assert_eq!(GameState::Paused.after_health_check(0), GameState::Paused);
        assert_eq!(GameState::Defeat.after_health_check(0), GameState::Defeat);
    }
}
