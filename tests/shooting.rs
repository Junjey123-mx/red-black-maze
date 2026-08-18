//! Pruebas de integración de la geometría de hitscan (Tarea 37):
//! `raycasting::cast_hitscan` sobre un `world::Level` real, probando
//! el orden enemigo-antes-de-pared / pared-antes-de-enemigo con la
//! representación de blanco real de producción (`HitscanTarget`).
//!
//! No dispara el arma, no requiere `RaylibHandle`/mouse, no
//! inicializa audio: esta suite protege únicamente la geometría de
//! selección de blanco. Las semánticas de munición/cooldown/daño ya
//! están cubiertas por las pruebas unitarias existentes de
//! `Weapon`/`GameSession`.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use raylib::prelude::Vector2;

use red_black_maze::config::BLOCK_SIZE;
use red_black_maze::player::Player;
use red_black_maze::raycasting::{HitscanHit, HitscanTarget, cast_hitscan};
use red_black_maze::world::Level;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempLevelFile {
    path: PathBuf,
}

impl TempLevelFile {
    fn write(contents: &str) -> Self {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let file_name = format!(
            "red_black_maze_shooting_test_{}_{counter}.txt",
            std::process::id()
        );

        let path = std::env::temp_dir().join(file_name);

        let mut file =
            fs::File::create(&path).expect("no se pudo crear el archivo temporal de nivel");

        file.write_all(contents.as_bytes())
            .expect("no se pudo escribir el archivo temporal de nivel");

        Self { path }
    }

    fn path_str(&self) -> &str {
        self.path
            .to_str()
            .expect("la ruta temporal debe ser UTF-8 válida")
    }
}

impl Drop for TempLevelFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Corredor recto de una sola celda de ancho: el jugador dispara
/// hacia el este (angle = 0) a lo largo de la fila 1. La pared
/// derecha (columna 8, Spade `#`) está a una distancia analítica
/// conocida de 312 unidades desde el centro del jugador.
const CORRIDOR: &str = "\
#########
#p     g#
#########
";

fn load_corridor() -> (TempLevelFile, Level) {
    let file = TempLevelFile::write(CORRIDOR);

    let level = Level::load(file.path_str()).expect("el corredor debe cargar");

    (file, level)
}

/// Jugador en el centro de la celda `(1, 1)` del corredor, mirando
/// hacia el este (angle = 0).
fn corridor_player() -> Player {
    let half_block = BLOCK_SIZE as f32 / 2.0;

    let pos = Vector2::new(
        1.0 * BLOCK_SIZE as f32 + half_block,
        1.0 * BLOCK_SIZE as f32 + half_block,
    );

    Player::new(pos, 0.0, std::f32::consts::PI / 3.0)
}

#[test]
fn enemy_before_wall_is_hit_with_a_distance_strictly_less_than_the_wall() {
    let (_file, level) = load_corridor();
    let player = corridor_player();

    // Centro a x=200 (delante de la pared en x=384), mismo eje y que
    // el jugador (y=72): sobre la trayectoria exacta del rayo.
    let targets = [HitscanTarget {
        center: Vector2::new(200.0, 72.0),
        radius: 8.0,
    }];

    let hit = cast_hitscan(&level, &player, &targets);

    match hit {
        HitscanHit::Target {
            target_index,
            distance,
        } => {
            assert_eq!(target_index, 0);

            // Distancia analítica de impacto en el círculo:
            // proyección (128) - semicuerda (8) = 120.
            assert!((distance - 120.0).abs() < 0.01);

            // La pared existe detrás del blanco (312 > 120): el
            // blanco gana porque está estrictamente antes.
            assert!(distance < 312.0);
        }

        HitscanHit::Wall(_) => panic!("se esperaba HitscanHit::Target, se obtuvo Wall"),
    }
}

#[test]
fn wall_before_enemy_wins_when_the_target_is_behind_the_wall() {
    let (_file, level) = load_corridor();
    let player = corridor_player();

    // Centro a x=450: más allá de la pared derecha (x=384). El
    // círculo del blanco existe geométricamente, pero su intersección
    // más cercana (370) queda después de la pared (312).
    let targets = [HitscanTarget {
        center: Vector2::new(450.0, 72.0),
        radius: 8.0,
    }];

    let hit = cast_hitscan(&level, &player, &targets);

    match hit {
        HitscanHit::Wall(wall_hit) => {
            assert!((wall_hit.distance() - 312.0).abs() < 0.01);
            assert_eq!(wall_hit.tile(), '#');
        }

        HitscanHit::Target { .. } => {
            panic!("se esperaba HitscanHit::Wall, se obtuvo Target (oclusión de pared rota)")
        }
    }
}

#[test]
fn off_axis_target_outside_the_ray_is_ignored_and_the_wall_wins() {
    let (_file, level) = load_corridor();
    let player = corridor_player();

    // Mismo eje X que el primer blanco válido (x=200), pero
    // desplazado 50px en Y: la distancia perpendicular al rayo (50)
    // excede el radio (8), así que el círculo nunca se cruza.
    let targets = [HitscanTarget {
        center: Vector2::new(200.0, 72.0 + 50.0),
        radius: 8.0,
    }];

    let hit = cast_hitscan(&level, &player, &targets);

    match hit {
        HitscanHit::Wall(wall_hit) => {
            assert!((wall_hit.distance() - 312.0).abs() < 0.01);
        }

        HitscanHit::Target { .. } => {
            panic!("un blanco fuera de eje no debería producir un impacto falso-positivo")
        }
    }
}

#[test]
fn nearest_of_two_valid_targets_before_the_wall_is_selected() {
    let (_file, level) = load_corridor();
    let player = corridor_player();

    // Blanco A (índice 0) más lejano (x=200 -> distancia ~120);
    // blanco B (índice 1) más cercano (x=150 -> distancia ~70).
    let targets = [
        HitscanTarget {
            center: Vector2::new(200.0, 72.0),
            radius: 8.0,
        },
        HitscanTarget {
            center: Vector2::new(150.0, 72.0),
            radius: 8.0,
        },
    ];

    let hit = cast_hitscan(&level, &player, &targets);

    match hit {
        HitscanHit::Target {
            target_index,
            distance,
        } => {
            assert_eq!(target_index, 1);
            assert!((distance - 70.0).abs() < 0.01);
        }

        HitscanHit::Wall(_) => panic!("se esperaba HitscanHit::Target, se obtuvo Wall"),
    }
}

#[test]
fn no_targets_resolves_to_the_wall() {
    let (_file, level) = load_corridor();
    let player = corridor_player();

    let hit = cast_hitscan(&level, &player, &[]);

    match hit {
        HitscanHit::Wall(wall_hit) => {
            assert!((wall_hit.distance() - 312.0).abs() < 0.01);
            assert_eq!(wall_hit.tile(), '#');
        }

        HitscanHit::Target { .. } => {
            panic!("sin blancos, cast_hitscan debe resolver siempre contra la pared")
        }
    }
}
