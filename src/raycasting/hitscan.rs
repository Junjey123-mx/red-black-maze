use raylib::prelude::Vector2;

use crate::player::Player;
use crate::world::Level;

use super::{RayHit, cast_ray};

/// Descriptor geométrico de un blanco disparable, exclusivo del
/// hitscan.
///
/// Esto NO es una entidad de partida: no conoce vida, daño, IA,
/// textura, animación ni tipo de enemigo. Solo describe un círculo
/// en el plano del laberinto (espacio de mundo) contra el cual se
/// puede probar intersección de rayo. Tarea 23 construirá estos
/// valores a partir de entidades reales.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HitscanTarget {
    /// Centro del círculo blanco, en píxeles de mundo.
    pub(crate) center: Vector2,

    /// Radio del círculo blanco, en píxeles de mundo.
    pub(crate) radius: f32,
}

/// Resultado de un disparo hitscan: o bien impactó el blanco
/// geométrico más cercano (identificado por su índice dentro del
/// slice suministrado), o bien no había ningún blanco válido antes
/// de la pared y el disparo se resuelve contra el rayo de pared ya
/// existente.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HitscanHit {
    Target { target_index: usize, distance: f32 },
    Wall(RayHit),
}

/// Calcula, si existe, la distancia del primer impacto de un rayo
/// contra un círculo blanco.
///
/// `direction` debe ser un vector unitario. Retorna `None` si el
/// blanco es geométricamente inválido, si el rayo no cruza el
/// círculo, o si el círculo completo queda detrás del origen del
/// rayo.
///
/// Si el origen del rayo ya está dentro del círculo, el impacto se
/// reporta a distancia `0.0` en lugar de la intersección de salida.
fn ray_circle_hit_distance(
    origin: Vector2,
    direction: Vector2,
    target: HitscanTarget,
) -> Option<f32> {
    if !target.radius.is_finite() || target.radius <= 0.0 {
        return None;
    }

    if !target.center.x.is_finite() || !target.center.y.is_finite() {
        return None;
    }

    let to_center_x = target.center.x - origin.x;

    let to_center_y = target.center.y - origin.y;

    let projection = to_center_x * direction.x + to_center_y * direction.y;

    let center_distance_squared = to_center_x * to_center_x + to_center_y * to_center_y;

    let perpendicular_squared = center_distance_squared - projection * projection;

    let radius_squared = target.radius * target.radius;

    if perpendicular_squared > radius_squared {
        return None;
    }

    let half_chord = (radius_squared - perpendicular_squared).max(0.0).sqrt();

    let near_distance = projection - half_chord;

    let far_distance = projection + half_chord;

    if far_distance < 0.0 {
        return None;
    }

    if near_distance >= 0.0 {
        Some(near_distance)
    } else {
        // El origen está dentro del círculo: el impacto ocurre en
        // el propio origen, nunca en la intersección de salida.
        Some(0.0)
    }
}

/// Elige, de entre los blancos suministrados, el de menor distancia
/// de impacto que además quede estrictamente antes de la pared.
///
/// Un blanco cuya distancia de impacto sea igual a la distancia de
/// pared NO gana: la pared tiene prioridad exacta en el empate. Ante
/// un empate exacto de distancia entre dos blancos válidos se
/// conserva el de menor índice, por ser el primero encontrado
/// durante el recorrido.
fn nearest_target_before_wall(
    origin: Vector2,
    direction: Vector2,
    targets: &[HitscanTarget],
    wall_distance: f32,
) -> Option<(usize, f32)> {
    let mut best: Option<(usize, f32)> = None;

    for (target_index, target) in targets.iter().enumerate() {
        let Some(distance) = ray_circle_hit_distance(origin, direction, *target) else {
            continue;
        };

        if !(distance < wall_distance) {
            continue;
        }

        match best {
            Some((_, best_distance)) if distance >= best_distance => {}
            _ => best = Some((target_index, distance)),
        }
    }

    best
}

/// Dispara un único hitscan a lo largo de la dirección central de
/// cámara del jugador (`player.a`, sin desplazamiento de FOV ni
/// dispersión).
///
/// Reutiliza el trazador de rayos de pared existente (`cast_ray`)
/// como autoridad geométrica de paredes, y resuelve el blanco
/// geométrico más cercano de `targets` que quede estrictamente antes
/// de esa distancia de pared. Si ningún blanco cumple esa condición,
/// el disparo se resuelve contra la pared.
pub(crate) fn cast_hitscan(
    level: &Level,
    player: &Player,
    targets: &[HitscanTarget],
) -> HitscanHit {
    let wall_hit = cast_ray(level, player, player.a);

    let direction = Vector2::new(player.a.cos(), player.a.sin());

    match nearest_target_before_wall(player.pos, direction, targets, wall_hit.distance) {
        Some((target_index, distance)) => HitscanHit::Target {
            target_index,
            distance,
        },

        None => HitscanHit::Wall(wall_hit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Comparación aproximada de punto flotante para las
    /// aserciones de estas pruebas, sin depender de una crate
    /// externa de comparación float.
    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    fn forward_x() -> Vector2 {
        Vector2::new(1.0, 0.0)
    }

    #[test]
    fn target_directly_on_ray_hits_at_near_edge() {
        let origin = Vector2::new(0.0, 0.0);

        let target = HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: 1.0,
        };

        let distance = ray_circle_hit_distance(origin, forward_x(), target);

        assert!(distance.is_some_and(|distance| approx_eq(distance, 9.0)));
    }

    #[test]
    fn target_off_ray_in_y_misses() {
        let origin = Vector2::new(0.0, 0.0);

        let target = HitscanTarget {
            center: Vector2::new(10.0, 50.0),
            radius: 1.0,
        };

        let distance = ray_circle_hit_distance(origin, forward_x(), target);

        assert!(distance.is_none());
    }

    #[test]
    fn target_behind_player_misses() {
        let origin = Vector2::new(0.0, 0.0);

        let target = HitscanTarget {
            center: Vector2::new(-10.0, 0.0),
            radius: 1.0,
        };

        let distance = ray_circle_hit_distance(origin, forward_x(), target);

        assert!(distance.is_none());
    }

    #[test]
    fn target_before_wall_wins() {
        let origin = Vector2::new(0.0, 0.0);

        let targets = [HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: 1.0,
        }];

        let result = nearest_target_before_wall(origin, forward_x(), &targets, 20.0);

        assert!(result.is_some_and(|(index, distance)| index == 0 && approx_eq(distance, 9.0)));
    }

    #[test]
    fn target_behind_wall_loses() {
        let origin = Vector2::new(0.0, 0.0);

        let targets = [HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: 1.0,
        }];

        let result = nearest_target_before_wall(origin, forward_x(), &targets, 5.0);

        assert!(result.is_none());
    }

    #[test]
    fn nearest_of_two_targets_wins() {
        let origin = Vector2::new(0.0, 0.0);

        let targets = [
            HitscanTarget {
                center: Vector2::new(10.0, 0.0),
                radius: 1.0,
            },
            HitscanTarget {
                center: Vector2::new(5.0, 0.0),
                radius: 1.0,
            },
        ];

        let result = nearest_target_before_wall(origin, forward_x(), &targets, 20.0);

        assert!(result.is_some_and(|(index, distance)| index == 1 && approx_eq(distance, 4.0)));
    }

    #[test]
    fn exact_wall_distance_tie_wall_wins() {
        let origin = Vector2::new(0.0, 0.0);

        let targets = [HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: 1.0,
        }];

        // La entrada del círculo ocurre exactamente en distancia 9.0.
        let result = nearest_target_before_wall(origin, forward_x(), &targets, 9.0);

        assert!(result.is_none());
    }

    #[test]
    fn player_inside_target_hits_at_zero() {
        let origin = Vector2::new(0.0, 0.0);

        let target = HitscanTarget {
            center: Vector2::new(0.0, 0.0),
            radius: 5.0,
        };

        let distance = ray_circle_hit_distance(origin, forward_x(), target);

        assert!(distance.is_some_and(|distance| approx_eq(distance, 0.0)));
    }

    #[test]
    fn invalid_target_geometry_is_ignored_safely() {
        let origin = Vector2::new(0.0, 0.0);

        let zero_radius = HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: 0.0,
        };

        let negative_radius = HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: -1.0,
        };

        let non_finite_radius = HitscanTarget {
            center: Vector2::new(10.0, 0.0),
            radius: f32::NAN,
        };

        let non_finite_center = HitscanTarget {
            center: Vector2::new(f32::INFINITY, 0.0),
            radius: 1.0,
        };

        assert!(ray_circle_hit_distance(origin, forward_x(), zero_radius).is_none());
        assert!(ray_circle_hit_distance(origin, forward_x(), negative_radius).is_none());
        assert!(ray_circle_hit_distance(origin, forward_x(), non_finite_radius).is_none());
        assert!(ray_circle_hit_distance(origin, forward_x(), non_finite_center).is_none());
    }
}
