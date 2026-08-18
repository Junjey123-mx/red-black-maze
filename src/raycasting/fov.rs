/// Calcula el ángulo de un rayo individual dentro del abanico de
/// campo de visión de la cámara, a partir de la columna de pantalla
/// que representa.
///
/// Fórmula EXACTA extraída de `rendering::world_3d::render_world`
/// (Tarea 37), preservada sin cambios: distribuye los rayos
/// linealmente desde `player_angle - fov/2` hasta `player_angle +
/// fov/2`, muestreando el CENTRO de cada columna (`column + 0.5`),
/// no su borde izquierdo. `column == 0` no cae exactamente en
/// `player_angle - fov/2`, ni `column == screen_width - 1` cae
/// exactamente en `player_angle + fov/2`: ambos quedan desplazados
/// hacia el centro por medio ancho de columna.
///
/// El resultado NO se normaliza a `[0, TAU)`: puede ser negativo o
/// exceder `TAU`, exactamente como antes de esta extracción — `sin`/
/// `cos` son periódicas, por lo que el raycaster no necesita un
/// ángulo normalizado.
///
/// `screen_width == 0` retorna `player_angle` de forma segura (sin
/// dividir por cero); en producción `render_world` siempre invoca
/// esto con un ancho de framebuffer ya recortado a un mínimo de 1,
/// por lo que este caso no ocurre en la práctica.
pub fn ray_angle_for_column(
    player_angle: f32,
    column: usize,
    screen_width: usize,
    fov: f32,
) -> f32 {
    if screen_width == 0 {
        return player_angle;
    }

    let ray_fraction = (column as f32 + 0.5) / screen_width as f32;

    player_angle - fov / 2.0 + fov * ray_fraction
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn center_of_an_odd_width_screen_points_exactly_at_the_player_angle() {
        // width = 101, columna central = 50 -> ray_fraction = 50.5/101 = 0.5 exacto.
        let angle = ray_angle_for_column(1.0, 50, 101, PI / 3.0);

        assert_eq!(angle, 1.0);
    }

    #[test]
    fn zero_width_returns_the_player_angle_safely() {
        assert_eq!(ray_angle_for_column(0.7, 0, 0, PI / 3.0), 0.7);
    }

    #[test]
    fn angle_increases_monotonically_across_columns() {
        let mut previous = ray_angle_for_column(0.0, 0, 10, PI / 3.0);

        for column in 1..10 {
            let current = ray_angle_for_column(0.0, column, 10, PI / 3.0);

            assert!(current > previous);

            previous = current;
        }
    }
}
