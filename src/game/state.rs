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

    /// Tarea 46.B: regla ÚNICA de resolución terminal de un cuadro de
    /// `Playing`, con la precedencia real y obligatoria entre las dos
    /// condiciones de fin de partida — `Defeat` SIEMPRE gana sobre
    /// `Victory` cuando ambas se cumplen en el mismo cuadro:
    ///
    /// ```text
    /// alive + no goal -> Playing
    /// alive + goal    -> Victory
    /// dead  + no goal -> Defeat
    /// dead  + goal    -> Defeat
    /// ```
    ///
    /// Implementada componiendo `after_health_check` PRIMERO y
    /// `after_goal_check` DESPUÉS — nunca al revés — precisamente
    /// para que una vida ya en `0` deje el estado en `Defeat` ANTES
    /// de que `after_goal_check` pueda evaluarse; como
    /// `after_goal_check` solo transiciona MIENTRAS el estado sigue
    /// siendo `Playing`, una vez en `Defeat` esa segunda llamada es
    /// un no-op garantizado, nunca una sobreescritura hacia
    /// `Victory`. Esta es la ÚNICA función que `App::update_playing`
    /// debe usar para decidir la resolución terminal de un cuadro —
    /// nunca dos llamadas independientes con un `return` intermedio
    /// entre ellas, que es precisamente el patrón que permitía a
    /// Victory "ganarle la carrera" a un Defeat legítimo del mismo
    /// cuadro antes de esta tarea.
    pub fn resolve_playing_terminal_state(self, health: i32, reached_goal: bool) -> Self {
        self.after_health_check(health)
            .after_goal_check(reached_goal)
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
    fn low_but_positive_health_never_transitions() {
        // Solo `health == 0` produce Defeat; nunca un umbral de
        // "Critical" por vida baja (10, 20, ...).
        for health in [1, 2, 10, 20, 50] {
            assert_eq!(
                GameState::Playing.after_health_check(health),
                GameState::Playing
            );
        }
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

    #[test]
    fn dead_and_goal_reached_in_the_same_update_prioritizes_defeat() {
        // Componer `after_health_check` seguido de `after_goal_check`
        // (el MISMO orden que `after_goal_check` internamente usa, y
        // el que `App::update_playing` realmente ejecuta desde Tarea
        // 46.B) demuestra la prioridad: una vez que `Defeat` ya fue
        // decidido, `after_goal_check` — que solo transiciona desde
        // `Playing` — es un no-op garantizado, nunca una
        // sobreescritura hacia `Victory`.
        let after_health = GameState::Playing.after_health_check(0);

        assert_eq!(after_health, GameState::Defeat);
        assert_eq!(after_health.after_goal_check(true), GameState::Defeat);
    }

    #[test]
    fn alive_with_goal_reached_still_transitions_to_victory() {
        // Regresión de prioridad complementaria: la vida > 0 nunca
        // bloquea una victoria legítima.
        let after_health = GameState::Playing.after_health_check(100);

        assert_eq!(after_health, GameState::Playing);
        assert_eq!(after_health.after_goal_check(true), GameState::Victory);
    }

    // --- Tarea 46.B: GameState::resolve_playing_terminal_state ---
    //
    // Esta es la ÚNICA función que `App::update_playing` invoca para
    // decidir la resolución terminal de un cuadro (ver
    // `src/app.rs`); estas pruebas ejercitan exactamente esa misma
    // API de producción, no una simulación paralela.

    #[test]
    fn alive_without_goal_remains_playing() {
        assert_eq!(
            GameState::Playing.resolve_playing_terminal_state(100, false),
            GameState::Playing
        );
    }

    #[test]
    fn alive_with_goal_reaches_victory() {
        assert_eq!(
            GameState::Playing.resolve_playing_terminal_state(100, true),
            GameState::Victory
        );
    }

    #[test]
    fn dead_without_goal_reaches_defeat() {
        assert_eq!(
            GameState::Playing.resolve_playing_terminal_state(0, false),
            GameState::Defeat
        );
    }

    #[test]
    fn dead_with_goal_reaches_defeat_not_victory() {
        // El caso central de Tarea 46.B: incluso con la meta
        // alcanzada este MISMO cuadro, una vida en `0` siempre
        // produce `Defeat`, nunca `Victory`.
        assert_eq!(
            GameState::Playing.resolve_playing_terminal_state(0, true),
            GameState::Defeat
        );
    }

    #[test]
    fn non_playing_states_never_transition_via_the_combined_rule() {
        for state in [
            GameState::Welcome,
            GameState::LevelSelect,
            GameState::Victory,
            GameState::Paused,
            GameState::Defeat,
        ] {
            assert_eq!(state.resolve_playing_terminal_state(0, true), state);
            assert_eq!(state.resolve_playing_terminal_state(100, false), state);
        }
    }

    #[test]
    fn health_already_zero_at_frame_start_resolves_to_defeat_regardless_of_goal() {
        // Tarea 46.B, sección 7: robustez defensiva. Si por
        // cualquier razón `Playing` se evaluara con vida ya en `0`
        // (el flujo normal de T45/T46.B ya lo hace imposible — ver
        // el guardia al inicio de `App::update_playing` — pero esta
        // función pura debe seguir siendo correcta por sí misma sin
        // depender de esa garantía externa), el resultado es
        // `Defeat` sin importar `reached_goal`.
        assert_eq!(
            GameState::Playing.resolve_playing_terminal_state(0, false),
            GameState::Defeat
        );
        assert_eq!(
            GameState::Playing.resolve_playing_terminal_state(0, true),
            GameState::Defeat
        );
    }
}
