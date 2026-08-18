//! Pruebas de integración de la matemática de raycasting (Tarea 37):
//! `raycasting::cast_ray` sobre un `world::Level` real, y el
//! distribuidor puro de ángulos de FOV `raycasting::ray_angle_for_column`.
//!
//! Compilan como un crate separado, viendo solo la API pública de
//! `red_black_maze`. No abren ninguna ventana de Raylib, no
//! inicializan audio y no construyen ningún tipo de rendering
//! (`Framebuffer`/`TextureManager`).

use std::f32::consts::PI;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use raylib::prelude::Vector2;

use red_black_maze::config::BLOCK_SIZE;
use red_black_maze::player::Player;
use red_black_maze::raycasting::{cast_ray, ray_angle_for_column};
use red_black_maze::world::Level;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempLevelFile {
    path: PathBuf,
}

impl TempLevelFile {
    fn write(contents: &str) -> Self {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let file_name = format!(
            "red_black_maze_raycasting_test_{}_{counter}.txt",
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

/// Sala rectangular cerrada y determinista: bordes superior/inferior
/// con Spade (`#`), bordes izquierdo/derecho con Heart (`+`),
/// interior completamente abierto. `BLOCK_SIZE = 48`.
///
/// Incluye un `p`/`g` de relleno (esquinas interiores, lejos de
/// cualquier trayectoria de rayo probada) únicamente para satisfacer
/// la validación estructural de `Level::load`; el jugador de estas
/// pruebas se construye con `Player::new` en posiciones exactas
/// conocidas, no vía `Player::from_level`.
const ENCLOSED_ROOM: &str = "\
#########
+p      +
+       +
+       +
+       +
+      g+
#########
";

fn assert_approx_eq(actual: f32, expected: f32, epsilon: f32) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "se esperaba aproximadamente {expected}, se obtuvo {actual} (epsilon {epsilon})"
    );
}

/// Jugador determinista en el centro de la celda `(3, 4)` de
/// `ENCLOSED_ROOM` — el centro geométrico exacto de la sala interior
/// (filas 1-5, columnas 1-7) — mirando en `angle`.
fn player_at_room_center(angle: f32) -> Player {
    let half_block = BLOCK_SIZE as f32 / 2.0;

    let pos = Vector2::new(
        4.0 * BLOCK_SIZE as f32 + half_block,
        3.0 * BLOCK_SIZE as f32 + half_block,
    );

    Player::new(pos, angle, PI / 3.0)
}

#[test]
fn known_east_and_west_distances_and_wall_tiles() {
    let file = TempLevelFile::write(ENCLOSED_ROOM);
    let level = Level::load(file.path_str()).expect("la sala cerrada debe cargar");

    // Este (angle = 0): pared derecha en la columna 8, Heart ('+').
    // Distancia analítica: 8*48 - (4*48+24) = 384 - 216 = 168.
    let east_player = player_at_room_center(0.0);
    let east_hit = cast_ray(&level, &east_player, 0.0);

    assert_approx_eq(east_hit.distance(), 168.0, 0.01);
    assert_eq!(east_hit.tile(), '+');

    // Oeste (angle = PI): pared izquierda en la columna 0, Heart ('+').
    // Distancia analítica: (4*48+24) - 1*48 = 216 - 48 = 168.
    let west_player = player_at_room_center(PI);
    let west_hit = cast_ray(&level, &west_player, PI);

    assert_approx_eq(west_hit.distance(), 168.0, 0.01);
    assert_eq!(west_hit.tile(), '+');
}

#[test]
fn known_south_and_north_distances_and_wall_tiles() {
    let file = TempLevelFile::write(ENCLOSED_ROOM);
    let level = Level::load(file.path_str()).expect("la sala cerrada debe cargar");

    // Sur (angle = PI/2): pared inferior en la fila 6, Spade ('#').
    // Distancia analítica: 6*48 - (3*48+24) = 288 - 168 = 120.
    let south_player = player_at_room_center(PI / 2.0);
    let south_hit = cast_ray(&level, &south_player, PI / 2.0);

    assert_approx_eq(south_hit.distance(), 120.0, 0.01);
    assert_eq!(south_hit.tile(), '#');

    // Norte (angle = 3*PI/2, equivalente a -PI/2): pared superior en
    // la fila 0, Spade ('#').
    // Distancia analítica: (3*48+24) - 1*48 = 168 - 48 = 120.
    let north_player = player_at_room_center(3.0 * PI / 2.0);
    let north_hit = cast_ray(&level, &north_player, 3.0 * PI / 2.0);

    assert_approx_eq(north_hit.distance(), 120.0, 0.01);
    assert_eq!(north_hit.tile(), '#');
}

#[test]
fn cardinal_and_diagonal_rays_terminate_safely_inside_bounds() {
    let file = TempLevelFile::write(ENCLOSED_ROOM);
    let level = Level::load(file.path_str()).expect("la sala cerrada debe cargar");

    let world_width = level.width() as f32 * BLOCK_SIZE as f32;
    let world_height = level.height() as f32 * BLOCK_SIZE as f32;

    let angles = [0.0, PI / 2.0, PI, 3.0 * PI / 2.0, PI / 4.0, 5.0 * PI / 4.0];

    for angle in angles {
        let player = player_at_room_center(angle);
        let hit = cast_ray(&level, &player, angle);

        assert!(
            hit.distance().is_finite(),
            "distancia no finita en angle={angle}"
        );
        assert!(
            hit.distance() > 0.0,
            "distancia no positiva en angle={angle}"
        );
        assert!(
            matches!(hit.tile(), '+' | '#'),
            "carácter de pared inesperado '{}' en angle={angle}",
            hit.tile()
        );

        let position = hit.position();

        assert!(
            position.x >= -0.01 && position.x <= world_width + 0.01,
            "posición X fuera de límites en angle={angle}: {}",
            position.x
        );
        assert!(
            position.y >= -0.01 && position.y <= world_height + 0.01,
            "posición Y fuera de límites en angle={angle}: {}",
            position.y
        );
    }
}

#[test]
fn fov_center_column_points_exactly_at_the_player_angle() {
    // Ancho impar (101): la columna central (50) muestrea su propio
    // centro exactamente en ray_fraction = 0.5.
    let angle = ray_angle_for_column(1.234, 50, 101, PI / 3.0);

    assert_eq!(angle, 1.234);
}

#[test]
fn fov_left_and_right_extents_approximate_half_fov() {
    let player_angle = 1.0;
    let fov = PI / 3.0;
    let screen_width = 101usize;

    // La columna 0 y la última columna muestrean el CENTRO de su
    // propia celda, no el borde extremo del abanico: el máximo
    // desplazamiento posible respecto al extremo teórico es el ancho
    // angular de media columna (fov / (2 * screen_width)).
    let half_column_epsilon = fov / (2.0 * screen_width as f32) + 1e-4;

    let left_angle = ray_angle_for_column(player_angle, 0, screen_width, fov);
    let right_angle = ray_angle_for_column(player_angle, screen_width - 1, screen_width, fov);

    assert_approx_eq(left_angle, player_angle - fov / 2.0, half_column_epsilon);
    assert_approx_eq(right_angle, player_angle + fov / 2.0, half_column_epsilon);
}

#[test]
fn fov_angles_are_monotonically_increasing_left_to_right() {
    let player_angle = 0.0;
    let fov = PI / 3.0;
    let screen_width = 40usize;

    let mut previous = ray_angle_for_column(player_angle, 0, screen_width, fov);

    for column in 1..screen_width {
        let current = ray_angle_for_column(player_angle, column, screen_width, fov);

        assert!(
            current > previous,
            "el ángulo debe crecer estrictamente de columna {} a {}",
            column - 1,
            column
        );

        previous = current;
    }
}

/// Convención real verificada por inspección de
/// `rendering::world_3d::render_world`: la fórmula NO normaliza el
/// ángulo resultante a `[0, TAU)`. Un `player_angle` pequeño con un
/// FOV mayor que el doble de esa magnitud produce un extremo
/// izquierdo genuinamente negativo, no envuelto de vuelta a un valor
/// positivo — exactamente el mismo valor crudo que consume `cos`/
/// `sin` en `cast_ray` (funciones periódicas que no requieren
/// normalización previa).
#[test]
fn fov_does_not_normalize_negative_results_near_zero() {
    let player_angle = 0.1;
    let fov = PI / 3.0;

    let left_angle = ray_angle_for_column(player_angle, 0, 101, fov);

    let expected_raw = player_angle - fov / 2.0 + fov * (0.5 / 101.0);

    assert!(
        expected_raw < 0.0,
        "la fijación de la prueba debe producir un extremo izquierdo negativo"
    );
    assert_approx_eq(left_angle, expected_raw, 1e-4);
    assert!(left_angle < 0.0);
}
