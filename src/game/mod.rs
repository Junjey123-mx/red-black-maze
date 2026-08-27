mod hand;
mod mode;
mod session;
mod state;

pub(crate) use hand::HandHudMessage;
pub(crate) use mode::GameMode;
pub(crate) use session::{BossMusicState, GameSession, ViewMode};
/// `GameState` se re-exporta como `pub` (Tarea 37): sus variantes
/// son un contrato de dominio legítimo de la aplicación, y la regla
/// pura `GameState::after_goal_check` es exactamente la que usa
/// `App` para decidir la transición Playing -> Victory. Ninguna
/// prueba de integración necesita `GameSession`/`ViewMode`, que
/// permanecen `pub(crate)`.
pub use state::GameState;
