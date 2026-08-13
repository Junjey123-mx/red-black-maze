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

/// Estado de PARTIDA del arma: máquina de estados visual, su
/// temporizado y el enfriamiento entre disparos aceptados.
///
/// No posee munición, daño, alcance, dispersión, referencia a
/// enemigos, sonido ni textura: eso pertenece a etapas/tareas
/// posteriores o a otros módulos (renderer, TextureManager).
pub(crate) struct Weapon {
    state: WeaponState,
    state_elapsed: f32,
    cooldown_remaining: f32,
}

impl Weapon {
    /// Crea un arma lista para disparar: `Idle`, sin tiempo
    /// acumulado y sin enfriamiento pendiente.
    pub(crate) fn new() -> Self {
        Self {
            state: WeaponState::Idle,
            state_elapsed: 0.0,
            cooldown_remaining: 0.0,
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

    /// Intenta iniciar un ciclo de disparo visual.
    ///
    /// Solo se acepta si el arma está en `Idle` y el enfriamiento ya
    /// llegó a cero; en ese caso pasa a `Fire`, reinicia el tiempo
    /// del estado y recarga el enfriamiento, retornando `true`.
    ///
    /// En cualquier otro caso el estado activo no se altera y
    /// retorna `false`. Este booleano es el evento de disparo
    /// aceptado que Tareas futuras (hitscan) consumirán.
    pub(crate) fn try_fire(&mut self) -> bool {
        if self.state != WeaponState::Idle || self.cooldown_remaining > 0.0 {
            return false;
        }

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
