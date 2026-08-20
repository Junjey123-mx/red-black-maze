/// Estados posibles del ciclo visual del arma.
///
/// Este enum representa ÚNICAMENTE el estado visual/de disparo del
/// arma; no conoce texturas, framebuffer, ni raycasting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeaponState {
    Idle,
    Fire,
    Recoil,
    Reload,
}

/// Duración del estado visual de disparo.
const FIRE_DURATION: f32 = 0.05;

/// Duración del estado visual de retroceso.
const RECOIL_DURATION: f32 = 0.10;

/// Intervalo mínimo entre disparos aceptados.
const FIRE_COOLDOWN: f32 = 0.25;

/// Capacidad máxima del cargador.
const MAGAZINE_CAPACITY: u32 = 6;

/// Munición de reserva inicial (fuera del cargador).
const INITIAL_RESERVE_AMMO: u32 = 18;

/// Duración de la recarga, en segundos.
const RELOAD_DURATION: f32 = 0.8;

/// Estado de PARTIDA del arma: máquina de estados visual, su
/// temporizado, el enfriamiento entre disparos aceptados y su
/// munición real (cargador + reserva).
///
/// No posee daño, alcance, dispersión, referencia a enemigos,
/// sonido ni textura: eso pertenece a otros módulos (hitscan,
/// renderer, TextureManager). La munición SÍ pertenece aquí: es
/// otra invariante de si el arma puede disparar, igual que el
/// estado/enfriamiento.
pub(crate) struct Weapon {
    state: WeaponState,
    state_elapsed: f32,
    cooldown_remaining: f32,
    magazine: u32,
    reserve: u32,
}

impl Weapon {
    /// Crea un arma lista para disparar: `Idle`, sin tiempo
    /// acumulado, sin enfriamiento pendiente, cargador lleno y
    /// reserva inicial completa.
    pub(crate) fn new() -> Self {
        Self {
            state: WeaponState::Idle,
            state_elapsed: 0.0,
            cooldown_remaining: 0.0,
            magazine: MAGAZINE_CAPACITY,
            reserve: INITIAL_RESERVE_AMMO,
        }
    }

    /// Estado visual actualmente activo.
    pub(crate) fn state(&self) -> WeaponState {
        self.state
    }

    /// Tiempo de enfriamiento restante antes de aceptar otro
    /// disparo.
    #[allow(dead_code)]
    pub(crate) fn cooldown_remaining(&self) -> f32 {
        self.cooldown_remaining
    }

    /// Munición restante en el cargador.
    pub(crate) fn ammo(&self) -> u32 {
        self.magazine
    }

    /// Munición restante en la reserva (fuera del cargador).
    pub(crate) fn reserve_ammo(&self) -> u32 {
        self.reserve
    }

    /// Progreso normalizado de la recarga en curso, o `None` si el
    /// arma no está en `WeaponState::Reload`.
    ///
    /// `Some(0.0)` justo al aceptarse la recarga (`state_elapsed ==
    /// 0.0`), acercándose a `Some(1.0)` justo antes de completarse.
    /// Deriva directamente de `state_elapsed`/`RELOAD_DURATION` — el
    /// MISMO temporizador que decide cuándo termina la recarga real
    /// (`update`); no existe un segundo timer visual independiente
    /// que pueda desincronizarse. `state_elapsed` nunca es negativo
    /// (solo se incrementa desde `0.0` dentro de `update`) y
    /// `RELOAD_DURATION` es una constante positiva, así que el
    /// resultado siempre cae en `[0.0, 1.0]` sin necesitar recortar
    /// activamente — el `clamp` se conserva de todas formas como
    /// garantía defensiva ante cualquier cambio futuro de esa
    /// invariante.
    ///
    /// Rendering LEE este valor; nunca escribe el estado interno del
    /// arma a través de él.
    pub(crate) fn reload_progress(&self) -> Option<f32> {
        if self.state != WeaponState::Reload {
            return None;
        }

        Some((self.state_elapsed / RELOAD_DURATION).clamp(0.0, 1.0))
    }

    /// Intenta iniciar un ciclo de disparo visual.
    ///
    /// Solo se acepta si el arma está en `Idle` (por lo tanto NUNCA
    /// durante `Reload`), el enfriamiento ya llegó a cero, Y queda
    /// munición en el cargador; en ese caso consume una unidad de
    /// munición del cargador, pasa a `Fire`, reinicia el tiempo del
    /// estado y recarga el enfriamiento, retornando `true`.
    ///
    /// En cualquier otro caso (estado/enfriamiento no listos, o
    /// cargador agotado) ni el estado activo ni la munición se
    /// alteran, y retorna `false`. Este booleano es el evento de
    /// disparo aceptado que el hitscan consume; la munición NUNCA
    /// se decrementa antes de esta comprobación de elegibilidad, ni
    /// desde ningún otro lugar (`App`, hitscan).
    pub(crate) fn try_fire(&mut self) -> bool {
        if self.state != WeaponState::Idle || self.cooldown_remaining > 0.0 {
            return false;
        }

        if self.magazine == 0 {
            return false;
        }

        self.magazine -= 1;
        self.state = WeaponState::Fire;
        self.state_elapsed = 0.0;
        self.cooldown_remaining = FIRE_COOLDOWN;

        true
    }

    /// Intenta iniciar una recarga.
    ///
    /// Solo se acepta desde `Idle` (nunca durante `Fire`/`Recoil`/ya
    /// `Reload`), y solo si el cargador NO está lleno y queda
    /// munición en la reserva; en ese caso pasa a `Reload` y reinicia
    /// el tiempo del estado, retornando `true`. La munición NO se
    /// transfiere aquí — solo al completarse la recarga en `update`
    /// — para que un `Retry`/interrupción antes de completarse nunca
    /// otorgue munición gratis.
    ///
    /// Si el cargador ya está lleno, si la reserva está en cero, o si
    /// el arma no está en `Idle` (incluyendo si ya está recargando),
    /// no se altera ningún estado y retorna `false`.
    pub(crate) fn try_start_reload(&mut self) -> bool {
        if self.state != WeaponState::Idle {
            return false;
        }

        if self.magazine >= MAGAZINE_CAPACITY || self.reserve == 0 {
            return false;
        }

        self.state = WeaponState::Reload;
        self.state_elapsed = 0.0;

        true
    }

    /// Transfiere de la reserva al cargador la cantidad exacta
    /// necesaria para llenarlo, sin exceder la reserva disponible.
    fn complete_reload(&mut self) {
        let needed = MAGAZINE_CAPACITY - self.magazine;

        let loaded = needed.min(self.reserve);

        self.magazine += loaded;
        self.reserve -= loaded;
    }

    /// Avanza el temporizado del arma según el tiempo transcurrido.
    ///
    /// Decrementa el enfriamiento de forma independiente del estado
    /// visual, y avanza `Fire -> Recoil -> Idle` o `Reload -> Idle`
    /// (transfiriendo munición de la reserva al cargador exactamente
    /// al completarse) conservando el remanente de tiempo entre
    /// transiciones, de modo que un `delta_time` grande pueda cruzar
    /// varios estados en una sola llamada sin perder tiempo
    /// fraccional ni quedar atascado.
    ///
    /// Un `delta_time` no finito o no positivo se ignora sin alterar
    /// el estado.
    pub(crate) fn update(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }

        self.cooldown_remaining = (self.cooldown_remaining - delta_time).max(0.0);

        let mut remaining_delta = delta_time;

        loop {
            let current_duration = match self.state {
                WeaponState::Fire => FIRE_DURATION,
                WeaponState::Recoil => RECOIL_DURATION,
                WeaponState::Reload => RELOAD_DURATION,
                WeaponState::Idle => return,
            };

            self.state_elapsed += remaining_delta;

            if self.state_elapsed < current_duration {
                return;
            }

            let overflow = self.state_elapsed - current_duration;

            match self.state {
                WeaponState::Fire => self.state = WeaponState::Recoil,

                WeaponState::Recoil => self.state = WeaponState::Idle,

                WeaponState::Reload => {
                    self.complete_reload();

                    self.state = WeaponState::Idle;
                }

                WeaponState::Idle => return,
            }

            self.state_elapsed = 0.0;
            remaining_delta = overflow;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Avanza el arma lo suficiente para que cruce `Fire -> Recoil
    /// -> Idle` y su enfriamiento llegue a cero, dejándola lista
    /// para un nuevo `try_fire`. Reutiliza la API pública `update`
    /// existente en vez de mutar campos privados directamente.
    fn advance_until_ready(weapon: &mut Weapon) {
        weapon.update(FIRE_COOLDOWN + 0.05);
    }

    #[test]
    fn new_weapon_starts_with_six_ammo() {
        let weapon = Weapon::new();

        assert_eq!(weapon.ammo(), 6);
    }

    #[test]
    fn accepted_shot_consumes_one_ammo() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        assert_eq!(weapon.ammo(), 5);
    }

    #[test]
    fn rejected_shot_during_cooldown_does_not_consume_ammo() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        assert_eq!(weapon.ammo(), 5);

        // El arma sigue en Fire/enfriamiento inmediatamente después
        // del primer disparo: este segundo intento debe rechazarse.
        assert!(!weapon.try_fire());
        assert_eq!(weapon.ammo(), 5);
    }

    #[test]
    fn six_accepted_shots_reduce_ammo_to_zero() {
        let mut weapon = Weapon::new();

        for expected_remaining in (0..6).rev() {
            assert!(weapon.try_fire());
            assert_eq!(weapon.ammo(), expected_remaining);

            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.ammo(), 0);
    }

    #[test]
    fn zero_ammo_try_fire_returns_false() {
        let mut weapon = Weapon::new();

        for _ in 0..6 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.ammo(), 0);
        assert!(!weapon.try_fire());
    }

    #[test]
    fn zero_ammo_stays_zero_and_does_not_underflow() {
        let mut weapon = Weapon::new();

        for _ in 0..6 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        for _ in 0..3 {
            assert!(!weapon.try_fire());
            assert_eq!(weapon.ammo(), 0);
        }
    }

    #[test]
    fn rejected_zero_ammo_trigger_does_not_enter_fire() {
        let mut weapon = Weapon::new();

        for _ in 0..6 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.state(), WeaponState::Idle);

        assert!(!weapon.try_fire());

        assert_eq!(weapon.state(), WeaponState::Idle);
        assert_eq!(weapon.ammo(), 0);
    }

    #[test]
    fn new_weapon_starts_with_eighteen_reserve() {
        let weapon = Weapon::new();

        assert_eq!(weapon.ammo(), 6);
        assert_eq!(weapon.reserve_ammo(), 18);
    }

    #[test]
    fn cannot_start_reload_with_a_full_magazine() {
        let mut weapon = Weapon::new();

        assert_eq!(weapon.ammo(), 6);
        assert!(!weapon.try_start_reload());
        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn reload_starts_when_magazine_is_partial() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());
        assert_eq!(weapon.state(), WeaponState::Reload);
    }

    #[test]
    fn reload_does_not_transfer_ammo_immediately() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert_eq!(weapon.ammo(), 5);

        // El disparo aceptado ya está resuelto (`Idle`), así que
        // `try_start_reload` se acepta aquí, pero la transferencia
        // de munición NO ocurre hasta que `update` complete el
        // temporizador de recarga (probado en
        // `reload_completes_after_duration`).
        assert!(weapon.try_start_reload());
        assert_eq!(weapon.state(), WeaponState::Reload);

        assert_eq!(weapon.ammo(), 5);
        assert_eq!(weapon.reserve_ammo(), 18);
    }

    #[test]
    fn cannot_fire_while_reloading() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());
        assert_eq!(weapon.state(), WeaponState::Reload);

        assert!(!weapon.try_fire());
        assert_eq!(weapon.ammo(), 5);
    }

    #[test]
    fn reload_completes_after_duration() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        // Justo antes de completarse, sigue en Reload.
        weapon.update(RELOAD_DURATION - 0.01);
        assert_eq!(weapon.state(), WeaponState::Reload);

        weapon.update(0.02);
        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn skipping_update_calls_freezes_reload_exactly_like_a_pause_menu_would() {
        // Tarea 42: `App::update_paused` congela TODA la simulación
        // jugable simplemente NO llamando a `GameSession::update_weapon`
        // (que delega en `Weapon::update`) mientras `GameState::Paused`
        // está activo — ningún mecanismo de pausa por-subsistema. Esta
        // prueba demuestra esa garantía directamente sobre `Weapon`,
        // sin necesitar `App`/`RaylibHandle`: 10 "segundos reales" en
        // los que `update` simplemente no se invoca (el equivalente
        // exacto de estar en pausa) no hacen avanzar el temporizador
        // de recarga ni un solo milisegundo.
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        weapon.update(0.30);
        assert_eq!(weapon.state(), WeaponState::Reload);

        // "10 segundos reales pasan" mientras está en pausa: en la
        // arquitectura real esto significa que `update_playing` (y
        // por lo tanto `update_weapon`) sencillamente no se llama
        // durante esos cuadros — representado aquí por la AUSENCIA
        // de cualquier llamada a `weapon.update(...)`, no por
        // llamarlo con un `delta_time` de 10.0.

        // Al reanudar, deben faltar aproximadamente 0.8 - 0.30 = 0.50 s.
        weapon.update(0.49);
        assert_eq!(weapon.state(), WeaponState::Reload);

        weapon.update(0.02);
        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn reload_progress_is_none_while_not_reloading() {
        let weapon = Weapon::new();

        assert_eq!(weapon.reload_progress(), None);
    }

    #[test]
    fn reload_progress_is_approximately_zero_just_after_acceptance() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        let progress = weapon.reload_progress().expect("debe estar recargando");

        assert!((progress - 0.0).abs() < 1e-6);
    }

    #[test]
    fn reload_progress_is_approximately_half_at_the_midpoint() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        weapon.update(RELOAD_DURATION / 2.0);

        let progress = weapon.reload_progress().expect("debe estar recargando");

        assert!((progress - 0.5).abs() < 0.01);
    }

    #[test]
    fn reload_progress_is_none_again_once_completed() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.state(), WeaponState::Idle);
        assert_eq!(weapon.reload_progress(), None);
    }

    #[test]
    fn reload_progress_never_exceeds_the_normalized_range() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        for _ in 0..50 {
            weapon.update(0.01);

            if let Some(progress) = weapon.reload_progress() {
                assert!((0.0..=1.0).contains(&progress));
            }
        }
    }

    #[test]
    fn partial_reload_calculation_four_to_six() {
        let mut weapon = Weapon::new();

        for _ in 0..2 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.ammo(), 4);

        assert!(weapon.try_start_reload());
        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.ammo(), 6);
        assert_eq!(weapon.reserve_ammo(), 16);
    }

    #[test]
    fn empty_magazine_reload_zero_to_six() {
        let mut weapon = Weapon::new();

        for _ in 0..6 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.ammo(), 0);

        assert!(weapon.try_start_reload());
        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.ammo(), 6);
        assert_eq!(weapon.reserve_ammo(), 12);
    }

    #[test]
    fn insufficient_reserve_loads_only_what_is_available() {
        let mut weapon = Weapon::new();

        // Vaciar el cargador y la reserva por completo: cada ciclo
        // dispara el cargador lleno (6) y recarga lo que la reserva
        // aún tenga. Con 18 de reserva más el cargador inicial de 6
        // (24 balas en total = 4 cargadores completos), hacen falta
        // 4 ciclos de disparo para agotar ambos (el cuarto ciclo no
        // encuentra reserva para recargar después de disparar).
        for _ in 0..4 {
            for _ in 0..6 {
                assert!(weapon.try_fire());
                advance_until_ready(&mut weapon);
            }

            if weapon.reserve_ammo() > 0 {
                assert!(weapon.try_start_reload());
                weapon.update(RELOAD_DURATION + 0.01);
            }
        }

        assert_eq!(weapon.ammo(), 0);
        assert_eq!(weapon.reserve_ammo(), 0);
    }

    #[test]
    fn insufficient_reserve_partial_load_example() {
        let mut weapon = Weapon::new();

        // Dos ciclos completos de disparo(6)+recarga(6) llevan la
        // reserva de 18 a 6 (18-6-6=6), dejando cargador y reserva
        // llenos/parciales respectivamente en mag=6, reserve=6.
        for _ in 0..2 {
            for _ in 0..6 {
                assert!(weapon.try_fire());
                advance_until_ready(&mut weapon);
            }

            assert!(weapon.try_start_reload());
            weapon.update(RELOAD_DURATION + 0.01);
        }

        assert_eq!(weapon.ammo(), 6);
        assert_eq!(weapon.reserve_ammo(), 6);

        // Disparar solo 3 y recargar: `needed = 6 - 3 = 3`, que la
        // reserva (6) cubre por completo, dejando mag=6, reserve=3.
        for _ in 0..3 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert!(weapon.try_start_reload());
        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.ammo(), 6);
        assert_eq!(weapon.reserve_ammo(), 3);

        // Vaciar el cargador por completo: mag=0, reserve=3 — el
        // ejemplo exacto del contrato de recarga (0/3 -> 3/0).
        for _ in 0..6 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.ammo(), 0);
        assert_eq!(weapon.reserve_ammo(), 3);

        assert!(weapon.try_start_reload());
        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.ammo(), 3);
        assert_eq!(weapon.reserve_ammo(), 0);
    }

    #[test]
    fn reserve_zero_prevents_reload() {
        let mut weapon = Weapon::new();

        // 4 ciclos de disparo agotan las 24 balas totales (cargador
        // inicial de 6 + reserva de 18); el cuarto ciclo no
        // encuentra reserva disponible para recargar.
        for _ in 0..4 {
            for _ in 0..6 {
                assert!(weapon.try_fire());
                advance_until_ready(&mut weapon);
            }

            if weapon.reserve_ammo() > 0 {
                assert!(weapon.try_start_reload());
                weapon.update(RELOAD_DURATION + 0.01);
            }
        }

        assert_eq!(weapon.reserve_ammo(), 0);
        assert_eq!(weapon.ammo(), 0);

        assert!(!weapon.try_start_reload());
        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn starting_reload_again_while_reloading_does_not_reset_timer() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        weapon.update(RELOAD_DURATION - 0.01);

        // Un segundo intento de recarga mientras ya se está
        // recargando debe rechazarse y NO reiniciar el temporizador.
        assert!(!weapon.try_start_reload());

        weapon.update(0.02);
        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn reload_is_frame_time_based() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());

        // Muchos cuadros pequeños deben sumar lo mismo que un único
        // cuadro grande.
        for _ in 0..79 {
            weapon.update(0.01);
        }

        assert_eq!(weapon.state(), WeaponState::Reload);

        weapon.update(0.05);
        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn weapon_returns_to_idle_after_reload_completion() {
        let mut weapon = Weapon::new();

        assert!(weapon.try_fire());
        advance_until_ready(&mut weapon);

        assert!(weapon.try_start_reload());
        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.state(), WeaponState::Idle);
    }

    #[test]
    fn fire_works_again_after_reload() {
        let mut weapon = Weapon::new();

        for _ in 0..6 {
            assert!(weapon.try_fire());
            advance_until_ready(&mut weapon);
        }

        assert_eq!(weapon.ammo(), 0);

        assert!(weapon.try_start_reload());
        weapon.update(RELOAD_DURATION + 0.01);

        assert_eq!(weapon.ammo(), 6);

        assert!(weapon.try_fire());
        assert_eq!(weapon.ammo(), 5);
    }

    #[test]
    fn ammo_never_goes_negative_or_underflows() {
        let mut weapon = Weapon::new();

        for _ in 0..4 {
            for _ in 0..6 {
                assert!(weapon.try_fire());
                advance_until_ready(&mut weapon);
            }

            if weapon.reserve_ammo() > 0 {
                assert!(weapon.try_start_reload());
                weapon.update(RELOAD_DURATION + 0.01);
            }
        }

        // Cargador y reserva agotados; ni try_fire ni try_start_reload
        // deben producir underflow (`u32`, esto pancharía en debug).
        assert!(!weapon.try_fire());
        assert!(!weapon.try_start_reload());

        assert_eq!(weapon.ammo(), 0);
        assert_eq!(weapon.reserve_ammo(), 0);
    }
}
