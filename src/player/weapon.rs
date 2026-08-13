/// Estados posibles del ciclo visual del arma.
///
/// Este enum representa ÚNICAMENTE el estado visual/de disparo del
/// arma; no conoce texturas, framebuffer, ni raycasting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeaponState {
    Idle,
    Fire,
    Recoil,
}

/// Duración del estado visual de disparo.
const FIRE_DURATION: f32 = 0.05;

/// Duración del estado visual de retroceso.
const RECOIL_DURATION: f32 = 0.10;

/// Intervalo mínimo entre disparos aceptados.
const FIRE_COOLDOWN: f32 = 0.25;

/// Munición inicial del arma.
const INITIAL_AMMO: u32 = 6;

/// Estado de PARTIDA del arma: máquina de estados visual, su
/// temporizado, el enfriamiento entre disparos aceptados y su
/// munición real.
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
    ammo: u32,
}

impl Weapon {
    /// Crea un arma lista para disparar: `Idle`, sin tiempo
    /// acumulado, sin enfriamiento pendiente y con munición inicial
    /// completa.
    pub(crate) fn new() -> Self {
        Self {
            state: WeaponState::Idle,
            state_elapsed: 0.0,
            cooldown_remaining: 0.0,
            ammo: INITIAL_AMMO,
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

    /// Munición restante.
    pub(crate) fn ammo(&self) -> u32 {
        self.ammo
    }

    /// Intenta iniciar un ciclo de disparo visual.
    ///
    /// Solo se acepta si el arma está en `Idle`, el enfriamiento ya
    /// llegó a cero, Y queda munición; en ese caso consume una
    /// unidad de munición, pasa a `Fire`, reinicia el tiempo del
    /// estado y recarga el enfriamiento, retornando `true`.
    ///
    /// En cualquier otro caso (estado/enfriamiento no listos, o
    /// munición agotada) ni el estado activo ni la munición se
    /// alteran, y retorna `false`. Este booleano es el evento de
    /// disparo aceptado que el hitscan consume; la munición NUNCA
    /// se decrementa antes de esta comprobación de elegibilidad, ni
    /// desde ningún otro lugar (`App`, hitscan).
    pub(crate) fn try_fire(&mut self) -> bool {
        if self.state != WeaponState::Idle || self.cooldown_remaining > 0.0 {
            return false;
        }

        if self.ammo == 0 {
            return false;
        }

        self.ammo -= 1;
        self.state = WeaponState::Fire;
        self.state_elapsed = 0.0;
        self.cooldown_remaining = FIRE_COOLDOWN;

        true
    }

    /// Avanza el temporizado del arma según el tiempo transcurrido.
    ///
    /// Decrementa el enfriamiento de forma independiente del estado
    /// visual, y avanza `Fire -> Recoil -> Idle` conservando el
    /// remanente de tiempo entre transiciones, de modo que un
    /// `delta_time` grande pueda cruzar varios estados en una sola
    /// llamada sin perder tiempo fraccional ni quedar atascado.
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
                WeaponState::Idle => return,
            };

            self.state_elapsed += remaining_delta;

            if self.state_elapsed < current_duration {
                return;
            }

            let overflow = self.state_elapsed - current_duration;

            self.state = match self.state {
                WeaponState::Fire => WeaponState::Recoil,
                WeaponState::Recoil => WeaponState::Idle,
                WeaponState::Idle => return,
            };

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
}
