/// Estados de alto nivel de la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    Welcome,
    LevelSelect,
    Playing,
    Victory,
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
    }
}
