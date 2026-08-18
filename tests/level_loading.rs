//! Pruebas de integración del contrato público de `Level` (Tarea 36).
//!
//! Compilan como un crate separado y solo pueden ver la API pública
//! de `red_black_maze`. No abren ninguna ventana de Raylib, no
//! inicializan audio y no dependen de rendering/UI: son pruebas de
//! dominio puro sobre `world::Level`/`world::LevelError`.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use red_black_maze::world::{Level, LevelError};

/// Contador global para garantizar nombres de archivo temporal
/// únicos incluso cuando varias pruebas escriben fixtures en
/// paralelo dentro del mismo proceso.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Guardia RAII: escribe un nivel de prueba en un archivo temporal
/// único (bajo `std::env::temp_dir()`, sin depender del directorio
/// de trabajo) y lo elimina automáticamente al salir de alcance,
/// incluso si una aserción posterior entra en pánico.
struct TempLevelFile {
    path: PathBuf,
}

impl TempLevelFile {
    fn write(contents: &str) -> Self {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

        let file_name = format!(
            "red_black_maze_level_loading_test_{}_{counter}.txt",
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

/// Mapa válido compacto: caja rectangular simple con una pared
/// interior, exactamente un `p` y un `g`. Usado por las pruebas de
/// carga/dimensiones/spawn/meta/celdas.
const VALID_MAP: &str = "\
#######
#p    #
# ### #
#   g #
#######
";

/// Segundo mapa válido: incluye `e`/`t` y los cuatro tipos de pared
/// (`+`/`-`/`|`/`#`), necesario para las pruebas de accesibilidad
/// (`is_walkable`) que el mapa compacto anterior no puede cubrir por
/// sí solo.
const FEATURES_MAP: &str = "\
+-+-+-+-+
+p  e   |
-       |
#  t    #
|       |
-      g-
+-+-+-+-+
";

#[test]
fn valid_map_loads_and_preserves_dimensions_spawn_goal_and_cells() {
    let file = TempLevelFile::write(VALID_MAP);

    let level = Level::load(file.path_str()).expect("el mapa válido debe cargar correctamente");

    assert_eq!(level.width(), 7);
    assert_eq!(level.height(), 5);

    assert_eq!(level.player_spawn(), (1, 1));
    assert_eq!(level.goal(), (3, 4));

    // Celdas de pared se preservan tal cual (carácter crudo).
    assert_eq!(level.cell_at(0, 0), Some('#'));
    assert_eq!(level.cell_at(2, 2), Some('#'));

    // Celdas de spawn/meta se preservan con su carácter propio.
    assert_eq!(level.cell_at(1, 1), Some('p'));
    assert_eq!(level.cell_at(3, 4), Some('g'));

    // Celda interior transitable.
    assert_eq!(level.cell_at(1, 2), Some(' '));
    assert!(level.is_walkable(1, 2));

    // Celda de pared interior no transitable.
    assert!(!level.is_walkable(2, 2));
}

#[test]
fn empty_map_is_rejected_with_the_empty_error() {
    let file = TempLevelFile::write("");

    let result = Level::load(file.path_str());

    match result {
        Ok(_) => panic!("se esperaba LevelError::Empty, la carga tuvo éxito"),
        Err(LevelError::Empty) => {}
        Err(other) => panic!("se esperaba LevelError::Empty, se obtuvo {other:?}"),
    }
}

#[test]
fn unequal_row_widths_are_rejected_with_the_typed_inconsistent_width_error() {
    let malformed = "#####\n#p g#\n####\n";

    let file = TempLevelFile::write(malformed);

    let result = Level::load(file.path_str());

    match result {
        Ok(_) => panic!("se esperaba LevelError::InconsistentRowWidth, la carga tuvo éxito"),

        Err(LevelError::InconsistentRowWidth {
            row,
            found,
            expected,
        }) => {
            assert_eq!(row, 3);
            assert_eq!(found, 4);
            assert_eq!(expected, 5);
        }

        Err(other) => {
            panic!("se esperaba LevelError::InconsistentRowWidth, se obtuvo {other:?}")
        }
    }
}

#[test]
fn missing_player_spawn_is_rejected_with_the_typed_error() {
    let map = "#####\n#   #\n#  g#\n#####\n";

    let file = TempLevelFile::write(map);

    let result = Level::load(file.path_str());

    match result {
        Ok(_) => panic!("se esperaba LevelError::PlayerSpawnCount(0), la carga tuvo éxito"),
        Err(LevelError::PlayerSpawnCount(0)) => {}
        Err(other) => {
            panic!("se esperaba LevelError::PlayerSpawnCount(0), se obtuvo {other:?}")
        }
    }
}

#[test]
fn missing_goal_is_rejected_with_the_typed_error() {
    let map = "#####\n#p  #\n#   #\n#####\n";

    let file = TempLevelFile::write(map);

    let result = Level::load(file.path_str());

    match result {
        Ok(_) => panic!("se esperaba LevelError::GoalCount(0), la carga tuvo éxito"),
        Err(LevelError::GoalCount(0)) => {}
        Err(other) => panic!("se esperaba LevelError::GoalCount(0), se obtuvo {other:?}"),
    }
}

/// Prueba adicional (contrato ya existente del parser, no una
/// validación nueva de Tarea 36): más de un `p` es rechazado con el
/// mismo `PlayerSpawnCount`, ahora con el conteo real encontrado.
#[test]
fn multiple_player_spawns_are_rejected_with_the_typed_error() {
    let map = "#####\n#p p#\n#   #\n#  g#\n#####\n";

    let file = TempLevelFile::write(map);

    let result = Level::load(file.path_str());

    match result {
        Ok(_) => panic!("se esperaba LevelError::PlayerSpawnCount(2), la carga tuvo éxito"),
        Err(LevelError::PlayerSpawnCount(2)) => {}
        Err(other) => {
            panic!("se esperaba LevelError::PlayerSpawnCount(2), se obtuvo {other:?}")
        }
    }
}

/// Misma idea que la prueba anterior, para `g`.
#[test]
fn multiple_goals_are_rejected_with_the_typed_error() {
    let map = "#####\n#p  #\n#   #\n#g g#\n#####\n";

    let file = TempLevelFile::write(map);

    let result = Level::load(file.path_str());

    match result {
        Ok(_) => panic!("se esperaba LevelError::GoalCount(2), la carga tuvo éxito"),
        Err(LevelError::GoalCount(2)) => {}
        Err(other) => panic!("se esperaba LevelError::GoalCount(2), se obtuvo {other:?}"),
    }
}

#[test]
fn space_player_goal_torch_and_enemy_markers_are_all_walkable() {
    let file = TempLevelFile::write(FEATURES_MAP);

    let level = Level::load(file.path_str()).expect("el mapa de características debe cargar");

    // Espacio interior genérico.
    assert!(level.is_walkable(1, 2));

    // 'p' en sí mismo (celda de aparición del jugador).
    assert!(level.is_walkable(1, 1));

    // 'g' — protege la ruta de Victoria: la meta debe ser transitable.
    assert!(level.is_walkable(5, 7));

    // 'e' — marcador de aparición de Dealer, no una pared.
    assert!(level.is_walkable(1, 4));

    // 't' — marcador de aparición de antorcha, no una pared.
    assert!(level.is_walkable(3, 3));
}

#[test]
fn all_four_card_wall_suits_are_not_walkable() {
    let file = TempLevelFile::write(FEATURES_MAP);

    let level = Level::load(file.path_str()).expect("el mapa de características debe cargar");

    // Heart ('+').
    assert_eq!(level.cell_at(0, 0), Some('+'));
    assert!(!level.is_walkable(0, 0));

    // Diamond ('-').
    assert_eq!(level.cell_at(0, 1), Some('-'));
    assert!(!level.is_walkable(0, 1));

    // Club ('|').
    assert_eq!(level.cell_at(1, 8), Some('|'));
    assert!(!level.is_walkable(1, 8));

    // Spade ('#').
    assert_eq!(level.cell_at(3, 0), Some('#'));
    assert!(!level.is_walkable(3, 0));
}

#[test]
fn out_of_range_row_and_column_are_safe_and_return_none_or_false() {
    let file = TempLevelFile::write(VALID_MAP);

    let level = Level::load(file.path_str()).expect("el mapa válido debe cargar");

    // row == height, column == width: justo fuera de rango.
    assert_eq!(level.cell_at(level.height(), 0), None);
    assert_eq!(level.cell_at(0, level.width()), None);
    assert!(!level.is_walkable(level.height(), 0));
    assert!(!level.is_walkable(0, level.width()));

    // Índices muy grandes: sin pánico, resultado seguro.
    assert_eq!(level.cell_at(1_000_000, 1_000_000), None);
    assert!(!level.is_walkable(1_000_000, 1_000_000));
}

/// Prueba de humo (no un re-análisis completo de T33/T34/T35):
/// confirma que los tres niveles finales reales todavía cargan a
/// través de la API pública, y protege barato la dimensión más
/// grande de House of Cards frente a una regresión futura.
#[test]
fn real_final_levels_still_load_with_expected_dimensions() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let crimson = Level::load(&format!("{manifest_dir}/levels/level_01.txt"))
        .expect("Crimson Entrance debe cargar");
    assert_eq!(crimson.width(), 13);
    assert_eq!(crimson.height(), 9);

    let black_club = Level::load(&format!("{manifest_dir}/levels/level_02.txt"))
        .expect("Black Club debe cargar");
    assert_eq!(black_club.width(), 13);
    assert_eq!(black_club.height(), 9);

    let house_of_cards = Level::load(&format!("{manifest_dir}/levels/level_03.txt"))
        .expect("House of Cards debe cargar");
    assert_eq!(house_of_cards.width(), 17);
    assert_eq!(house_of_cards.height(), 13);
}
