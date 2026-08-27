mod defeat;
mod game_of_life;
mod level_select;
mod pause;
mod victory;
mod welcome;

pub(crate) use defeat::{DefeatMenuItem, DefeatScreen};
pub(crate) use game_of_life::{GameOfLife, GameOfLifeRenderConfig};
pub(crate) use level_select::LevelSelectScreen;
pub(crate) use pause::{PauseMenuItem, PauseScreen};
pub(crate) use victory::{VictoryAction, VictoryScreen};
pub(crate) use welcome::WelcomeScreen;

/// Rectángulo de hitbox en coordenadas lógicas del framebuffer.
///
/// Único punto de la comprobación punto-rectángulo usada por
/// Bienvenida/Pausa/Victoria/Derrota para resolver hover/clic de
/// mouse: cada pantalla sigue calculando sus propias coordenadas de
/// fila a partir de su `compute_layout` privado (las MISMAS que ya
/// usa para dibujar esa fila), y solo delega en `Hitbox::contains` la
/// prueba geométrica en sí, en vez de reimplementarla en cada módulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hitbox {
    pub(crate) x0: i32,
    pub(crate) y0: i32,
    pub(crate) x1: i32,
    pub(crate) y1: i32,
}

impl Hitbox {
    pub(crate) fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x0 && x < self.x1 && y >= self.y0 && y < self.y1
    }
}

#[cfg(test)]
mod hitbox_tests {
    use super::Hitbox;

    #[test]
    fn contains_is_inclusive_on_the_low_edge_and_exclusive_on_the_high_edge() {
        let hitbox = Hitbox {
            x0: 10,
            y0: 20,
            x1: 30,
            y1: 40,
        };

        assert!(hitbox.contains(10, 20));
        assert!(hitbox.contains(29, 39));
        assert!(!hitbox.contains(30, 25));
        assert!(!hitbox.contains(15, 40));
        assert!(!hitbox.contains(9, 25));
    }
}
