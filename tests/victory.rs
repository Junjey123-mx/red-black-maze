//! Pruebas de integración de la regla de transición Playing ->
//! Victory y de la progresión del catálogo de niveles (Tarea 37).
//!
//! `GameState::after_goal_check` es la regla pura REALMENTE usada
//! por `App::update_playing` (ver `src/app.rs`): esta suite no
//! reimplementa ni asigna directamente `GameState::Victory` como
//! "prueba" — invoca la función de producción y verifica su
//! resultado. `LevelManager::restart`/`next` se ejercitan sobre el
//! catálogo real (los tres niveles finalizados en `levels/`).
//!
//! No abre ninguna ventana de Raylib, no inicializa audio, no
//! construye `GameSession`/`App`.

use red_black_maze::game::GameState;
use red_black_maze::world::{Level, LevelManager};

/// Compara dos `Level` por contenido completo (dimensiones + cada
/// carácter crudo de celda), no por identidad de objeto ni por ruta
/// de archivo (que `LevelManager`/`LevelInfo` no exponen
/// externamente). Es la única forma estable de distinguir "sigue
/// siendo el mismo nivel" sin ensanchar la API más allá de lo
/// mínimo.
fn levels_have_identical_content(a: &Level, b: &Level) -> bool {
    if a.width() != b.width() || a.height() != b.height() {
        return false;
    }

    for row in 0..a.height() {
        for column in 0..a.width() {
            if a.cell_at(row, column) != b.cell_at(row, column) {
                return false;
            }
        }
    }

    true
}

fn reference_level(relative_path: &str) -> Level {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    Level::load(&format!("{manifest_dir}/{relative_path}"))
        .unwrap_or_else(|error| panic!("no se pudo cargar {relative_path}: {error:?}"))
}

// --- Regla pura de transición Playing -> Victory ---

#[test]
fn playing_transitions_to_victory_exactly_when_the_goal_is_reached() {
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

    // Ya en Victory: la condición de meta verdadera no produce
    // ningún efecto adicional (no hay una transición "espuria"
    // Victory -> Victory que dispare efectos secundarios de nuevo).
    assert_eq!(
        GameState::Victory.after_goal_check(true),
        GameState::Victory
    );
}

// --- Progresión real de LevelManager ---

#[test]
fn next_progresses_through_all_three_static_levels_without_wrapping() {
    let mut manager = LevelManager::new();

    let level1 = manager.load(0).expect("el índice 0 debe cargar");
    assert!(levels_have_identical_content(
        &level1,
        &reference_level("levels/level_01.txt")
    ));

    let level2 = manager
        .next()
        .expect("next() no debe fallar")
        .expect("debe existir un segundo nivel");
    assert!(levels_have_identical_content(
        &level2,
        &reference_level("levels/level_02.txt")
    ));
    assert!(!levels_have_identical_content(
        &level2,
        &reference_level("levels/level_01.txt")
    ));

    let level3 = manager
        .next()
        .expect("next() no debe fallar")
        .expect("debe existir un tercer nivel");
    assert!(levels_have_identical_content(
        &level3,
        &reference_level("levels/level_03.txt")
    ));

    assert!(manager.has_next());
}

/// Tarea 48: la victoria de House of Cards YA NO es el final del
/// juego — conduce a "The Dealer's True Maze", el cuarto nivel
/// procedural. Este test solo usa la superficie `pub` de
/// `LevelManager` (igual que el resto de este archivo): no puede
/// consultar el tema/seed/`current_is_procedural` (`pub(crate)`),
/// pero SÍ puede confirmar que el contenido del cuarto nivel no
/// coincide con ninguno de los tres `.txt` estáticos, que es el
/// último de verdad (`has_next() == false`), y que un `next()`
/// adicional no envuelve de vuelta al nivel 1.
#[test]
fn house_of_cards_victory_leads_to_the_procedural_fourth_level_as_the_true_final_level() {
    let mut manager = LevelManager::new();

    manager
        .load(2)
        .expect("House of Cards (índice 2) debe cargar");
    assert!(manager.has_next());

    let level4 = manager
        .next()
        .expect("next() no debe fallar")
        .expect("debe existir un cuarto nivel: el procedural");

    for reference_path in [
        "levels/level_01.txt",
        "levels/level_02.txt",
        "levels/level_03.txt",
    ] {
        assert!(
            !levels_have_identical_content(&level4, &reference_level(reference_path)),
            "el cuarto nivel procedural no debe coincidir con {reference_path}"
        );
    }

    // Ahora sí es el nivel final: next() retorna None, sin envolver
    // de vuelta al nivel 1.
    let after_final = manager
        .next()
        .expect("next() no debe fallar en el último nivel");
    assert!(after_final.is_none());
    assert!(!manager.has_next());
}

#[test]
fn restart_reloads_the_current_level_including_after_progression() {
    let mut manager = LevelManager::new();

    let level1 = manager.load(0).expect("el índice 0 debe cargar");

    let restarted_level1 = manager
        .restart()
        .expect("restart() debe recargar el nivel 1");
    assert!(levels_have_identical_content(&level1, &restarted_level1));

    // Avanza a Black Club y confirma que restart() ahora recarga
    // Black Club, NO vuelve a Crimson Entrance.
    let level2 = manager
        .next()
        .expect("next() no debe fallar")
        .expect("debe existir un segundo nivel");

    let restarted_level2 = manager
        .restart()
        .expect("restart() debe recargar el nivel actual (2)");

    assert!(levels_have_identical_content(&level2, &restarted_level2));
    assert!(!levels_have_identical_content(
        &restarted_level2,
        &reference_level("levels/level_01.txt")
    ));
}
