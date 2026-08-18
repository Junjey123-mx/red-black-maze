//! Pruebas de integración del contrato público de colisión (Tarea
//! 36): `world::can_occupy` sobre un `world::Level` real cargado
//! desde un archivo temporal.
//!
//! Compilan como un crate separado, viendo solo la API pública de
//! `red_black_maze`. No abren ninguna ventana de Raylib, no
//! inicializan audio, y no sintetizan entrada de teclado/mouse: la
//! matemática de colisión se invoca directamente.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use red_black_maze::config::BLOCK_SIZE;
use red_black_maze::world::{Level, can_occupy};

/// Contador global para nombres de archivo temporal únicos, mismo
/// mecanismo independiente que `tests/level_loading.rs` (evitar un
/// tercer archivo compartido, según Tarea 36).
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Guardia RAII: mismo patrón que `TempLevelFile` en
/// `tests/level_loading.rs`, duplicado deliberadamente aquí (una
/// utilidad de dos líneas no justifica un tercer archivo de soporte
/// compartido).
struct TempLevelFile {
    path: PathBuf,
}

impl TempLevelFile {
    fn write(contents: &str) -> Self {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let file_name = format!(
            "red_black_maze_collision_test_{}_{counter}.txt",
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

/// Mapa determinista de colisión: paredes exteriores sólidas, una
/// pared interior horizontal de tres celdas (fila 2, columnas 3-5)
/// que deja un pasillo abierto de siete celdas de ancho justo encima
/// (fila 1) y una habitación interior completamente abierta debajo
/// (filas 3-4), con `p` y `g` en esquinas opuestas.
const COLLISION_MAP: &str = "\
#########
#p      #
#  ###  #
#       #
#       #
#      g#
#########
";

fn load_collision_level() -> (TempLevelFile, Level) {
    let file = TempLevelFile::write(COLLISION_MAP);

    let level = Level::load(file.path_str()).expect("el mapa de colisión debe cargar");

    (file, level)
}

/// Centro en píxeles de mundo de la celda `(row, column)`, para
/// construir posiciones de prueba sin adivinar coordenadas.
fn cell_center(row: usize, column: usize) -> (f32, f32) {
    let half_block = BLOCK_SIZE as f32 / 2.0;

    (
        column as f32 * BLOCK_SIZE as f32 + half_block,
        row as f32 * BLOCK_SIZE as f32 + half_block,
    )
}

#[test]
fn clear_interior_point_is_occupiable() {
    let (_file, level) = load_collision_level();

    // Fila 3, columna 4: centro de la habitación interior abierta,
    // lejos de cualquier pared (la pared interior más cercana está
    // en la fila 2, a 24px de distancia vertical: el radio de
    // colisión real no puede alcanzarla).
    let (x, y) = cell_center(3, 4);

    assert!(can_occupy(&level, x, y, BLOCK_SIZE));
}

#[test]
fn wall_center_is_not_occupiable() {
    let (_file, level) = load_collision_level();

    // Fila 2, columna 4: centro exacto de la pared interior.
    let (x, y) = cell_center(2, 4);

    assert!(!can_occupy(&level, x, y, BLOCK_SIZE));
}

#[test]
fn collision_radius_prevents_clipping_through_a_nearby_wall() {
    let (_file, level) = load_collision_level();

    // El centro (fila 1, columna 4) es técnicamente transitable, pero
    // se desplaza deliberadamente cerca del borde inferior de la
    // celda (a 2px de la frontera con la fila 2, que en la columna 4
    // es pared): el radio de colisión real debe alcanzar esa pared y
    // rechazar la posición, aunque el punto central esté en piso
    // válido.
    let (x, _) = cell_center(1, 4);

    let y_near_wall_boundary = 2.0 * BLOCK_SIZE as f32 - 2.0;

    assert!(level.is_walkable(1, 4));
    assert!(!can_occupy(&level, x, y_near_wall_boundary, BLOCK_SIZE));
}

#[test]
fn corridor_center_with_full_clearance_is_occupiable() {
    let (_file, level) = load_collision_level();

    // Fila 1, columna 4: mismo pasillo de la prueba anterior, pero
    // centrado exactamente en la celda, con margen completo respecto
    // a la pared interior de la fila 2 y al techo exterior de la
    // fila 0.
    let (x, y) = cell_center(1, 4);

    assert!(can_occupy(&level, x, y, BLOCK_SIZE));
}

#[test]
fn negative_x_and_negative_y_are_rejected_without_panicking() {
    let (_file, level) = load_collision_level();

    let (_, safe_y) = cell_center(1, 4);

    assert!(!can_occupy(&level, -5.0, safe_y, BLOCK_SIZE));

    let (safe_x, _) = cell_center(1, 4);

    assert!(!can_occupy(&level, safe_x, -5.0, BLOCK_SIZE));

    // Caso extremo: ambas coordenadas muy negativas, sin pánico ni
    // subdesbordamiento de `usize`.
    assert!(!can_occupy(&level, -1_000_000.0, -1_000_000.0, BLOCK_SIZE));
}

#[test]
fn beyond_right_and_beyond_bottom_edges_are_rejected() {
    let (_file, level) = load_collision_level();

    let world_width = level.width() as f32 * BLOCK_SIZE as f32;
    let world_height = level.height() as f32 * BLOCK_SIZE as f32;

    let (_, safe_y) = cell_center(1, 4);

    assert!(!can_occupy(&level, world_width + 50.0, safe_y, BLOCK_SIZE));

    let (safe_x, _) = cell_center(1, 4);

    assert!(!can_occupy(&level, safe_x, world_height + 50.0, BLOCK_SIZE));
}

#[test]
fn exact_and_near_map_edges_do_not_panic() {
    let (_file, level) = load_collision_level();

    let world_width = level.width() as f32 * BLOCK_SIZE as f32;
    let world_height = level.height() as f32 * BLOCK_SIZE as f32;

    // Exactamente en el borde (fuera de cualquier celda válida) y
    // apenas más allá: ambos deben resolverse de forma segura, sin
    // entrar en pánico, independientemente del resultado booleano
    // exacto.
    let _ = can_occupy(&level, world_width, world_height, BLOCK_SIZE);
    let _ = can_occupy(&level, world_width + 1.0, world_height + 1.0, BLOCK_SIZE);
    let _ = can_occupy(&level, 0.0, 0.0, BLOCK_SIZE);
}
