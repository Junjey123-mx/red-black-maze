mod game_of_life;
mod level_select;
mod victory;
mod welcome;

pub(crate) use game_of_life::{GameOfLife, GameOfLifeRenderConfig};
pub(crate) use level_select::LevelSelectScreen;
pub(crate) use victory::{VictoryAction, VictoryScreen};
pub(crate) use welcome::WelcomeScreen;
