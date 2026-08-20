use raylib::prelude::Vector2;

use crate::player::{Player, Weapon, WeaponState};
use crate::world::{
    DistanceField, Entity, EntityDamageOutcome, EntityState, EntityStateTransition, Level,
};

/// Modos de visualización disponibles.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ViewMode {
    Map2D,
    World3D,
}

/// Daño aplicado a un Dealer por cada disparo aceptado que lo
/// impacta. Con `DEALER_MAX_HEALTH = 100` (definido en
/// `world::entity`), un Dealer muere tras exactamente dos golpes.
const DEALER_DAMAGE_PER_HIT: i32 = 50;

/// Duración aproximada de cada cuadro de la animación de antorcha.
const TORCH_FRAME_DURATION: f32 = 0.1;

/// Número total de cuadros de la animación de antorcha.
const TORCH_FRAME_COUNT: usize = 4;

/// Estado de la animación de antorcha: cuadro actual en
/// reproducción y tiempo acumulado hacia el siguiente cambio de
/// cuadro.
///
/// Esto es estado de PARTIDA, no un recurso de textura: pertenece
/// a `GameSession`, no a `TextureManager`.
struct TorchAnimationState {
    frame_index: usize,
    elapsed_seconds: f32,
}

impl TorchAnimationState {
    fn new() -> Self {
        Self {
            frame_index: 0,
            elapsed_seconds: 0.0,
        }
    }

    /// Avanza la animación según el tiempo transcurrido desde el
    /// cuadro anterior.
    ///
    /// Un `delta_time` no finito o no positivo se ignora sin
    /// alterar el estado. El tiempo excedente sobre una duración de
    /// cuadro se conserva (no se descarta) para no perder tiempo
    /// fraccional acumulado, y un `delta_time` suficientemente
    /// grande avanza tantos cuadros como corresponda.
    fn update(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }

        self.elapsed_seconds += delta_time;

        while self.elapsed_seconds >= TORCH_FRAME_DURATION {
            self.elapsed_seconds -= TORCH_FRAME_DURATION;

            self.frame_index = (self.frame_index + 1) % TORCH_FRAME_COUNT;
        }
    }
}

/// Estado en tiempo de ejecución de la partida activa.
pub(crate) struct GameSession {
    pub(crate) level: Level,
    pub(crate) player: Player,
    pub(crate) view_mode: ViewMode,
    torch_animation: TorchAnimationState,
    weapon: Weapon,
    entities: Vec<Entity>,
}

impl GameSession {
    /// Crea una sesión a partir de un nivel y un jugador
    /// ya construidos.
    ///
    /// Inicia mostrando el mapa 2D, con la animación de antorcha en
    /// su cuadro inicial, y crea exactamente un Dealer por cada
    /// marcador `e` que el nivel haya descubierto, centrado en su
    /// celda de aparición.
    pub(crate) fn new(level: Level, player: Player, block_size: usize) -> Self {
        let entities = level
            .enemy_spawns()
            .iter()
            .map(|&(row, column)| Entity::dealer_at_cell(row, column, block_size))
            .collect();

        Self {
            level,
            player,
            view_mode: ViewMode::Map2D,
            torch_animation: TorchAnimationState::new(),
            weapon: Weapon::new(),
            entities,
        }
    }

    /// Entidades activas de la sesión actual (los Dealers
    /// aparecidos a partir de los marcadores `e` del nivel).
    pub(crate) fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Avanza el comportamiento de cada entidad (temporizador de
    /// `Hit`, reevaluación de proximidad `Idle`/`Alert`, y
    /// persecución mientras esté `Alert`) según la posición actual
    /// del jugador, y reporta ÚNICAMENTE las transiciones de estado
    /// que realmente ocurrieron (`Entity::update` ya distingue
    /// "cambio real" de "sin cambio").
    ///
    /// Ninguna entidad ataca: la persecución solo mueve la posición
    /// de los Dealers `Alert` hacia el jugador, respetando la
    /// geometría del laberinto vía `world::DistanceField` (BFS de 4
    /// direcciones sobre `Level`, la misma autoridad de
    /// transitabilidad que colisión/raycasting). El campo de
    /// distancias se calcula A LO SUMO una vez por cuadro,
    /// compartido entre todas las entidades `Alert` (nunca uno por
    /// Dealer), y se omite por completo si ninguna entidad está
    /// `Alert` este cuadro.
    ///
    /// El resultado reportado es dominio puro
    /// (`EntityStateTransition`), sin vocabulario de audio/
    /// presentación: quien interpreta el evento (`App`) decide qué
    /// hacer con él.
    pub(crate) fn update_entities(
        &mut self,
        delta_time: f32,
        block_size: usize,
    ) -> Vec<EntityStateTransition> {
        let player_position = self.player.pos;

        let any_alert = self
            .entities
            .iter()
            .any(|entity| entity.state() == EntityState::Alert);

        let distance_field = any_alert.then(|| {
            let player_cell = world_to_cell(player_position, block_size);

            DistanceField::from_level(&self.level, player_cell)
        });

        self.entities
            .iter_mut()
            .filter_map(|entity| {
                let pursuit_target = distance_field.as_ref().and_then(|field| {
                    let entity_cell = world_to_cell(entity.position(), block_size);

                    field
                        .step_toward_origin(entity_cell)
                        .map(|(row, column)| cell_center(row, column, block_size))
                });

                entity.update(player_position, delta_time, block_size, pursuit_target)
            })
            .collect()
    }

    /// Aplica el daño de un golpe de Dealer aceptado a la entidad
    /// indicada, con verificación segura de límites, y reporta el
    /// resultado semántico (`EntityDamageOutcome`) para que quien
    /// interpreta el evento (`App`) pueda distinguir un golpe real de
    /// un evento sin efecto sin inferirlo de `EntityState`.
    ///
    /// Un `entity_index` fuera de rango produce `EntityDamageOutcome::None`
    /// sin entrar en pánico. La cantidad de daño y la invariante de
    /// salud/estado son responsabilidad exclusiva de
    /// `Entity::apply_damage`; este método solo coordina el acceso
    /// indexado seguro.
    pub(crate) fn damage_entity(&mut self, entity_index: usize) -> EntityDamageOutcome {
        match self.entities.get_mut(entity_index) {
            Some(entity) => entity.apply_damage(DEALER_DAMAGE_PER_HIT),

            None => EntityDamageOutcome::None,
        }
    }

    /// Avanza la animación de antorcha según el tiempo transcurrido
    /// desde la última actualización.
    pub(crate) fn update_torch_animation(&mut self, delta_time: f32) {
        self.torch_animation.update(delta_time);
    }

    /// Cuadro de animación de antorcha actualmente activo.
    pub(crate) fn torch_frame_index(&self) -> usize {
        self.torch_animation.frame_index
    }

    /// Avanza la máquina de estados visual del arma según el tiempo
    /// transcurrido desde la última actualización.
    pub(crate) fn update_weapon(&mut self, delta_time: f32) {
        self.weapon.update(delta_time);
    }

    /// Estado visual actualmente activo del arma.
    pub(crate) fn weapon_state(&self) -> WeaponState {
        self.weapon.state()
    }

    /// Progreso normalizado de la recarga en curso, o `None` si el
    /// arma no está recargando. Ver `Weapon::reload_progress`; solo
    /// reenvía la lectura, no posee ningún temporizador propio.
    pub(crate) fn weapon_reload_progress(&self) -> Option<f32> {
        self.weapon.reload_progress()
    }

    /// Intenta aceptar un evento de disparo, iniciando el ciclo
    /// visual del arma.
    ///
    /// Retorna `true` si el disparo fue aceptado (útil en tareas
    /// futuras para disparar el hitscan), `false` si el arma está
    /// en enfriamiento o no está `Idle`.
    pub(crate) fn try_fire_weapon(&mut self) -> bool {
        self.weapon.try_fire()
    }

    /// Intenta iniciar una recarga del arma (tecla R).
    ///
    /// Retorna `true` si la recarga fue aceptada (cargador no lleno,
    /// reserva disponible, arma en `Idle`), `false` en cualquier
    /// otro caso. La transferencia real de munición ocurre más
    /// tarde, dentro de `update_weapon`, al completarse el
    /// temporizador — nunca aquí.
    pub(crate) fn try_start_weapon_reload(&mut self) -> bool {
        self.weapon.try_start_reload()
    }

    /// Vida actual del jugador, para presentación (HUD) u otro
    /// consumidor de solo lectura.
    pub(crate) fn player_health(&self) -> i32 {
        self.player.health()
    }

    /// Munición actual del arma, para presentación (HUD) u otro
    /// consumidor de solo lectura.
    pub(crate) fn weapon_ammo(&self) -> u32 {
        self.weapon.ammo()
    }

    /// Munición de reserva del arma (fuera del cargador), para
    /// presentación (HUD) u otro consumidor de solo lectura.
    pub(crate) fn weapon_reserve_ammo(&self) -> u32 {
        self.weapon.reserve_ammo()
    }

    /// Indica si el jugador se encuentra actualmente dentro de la
    /// celda de meta (`Level::goal`).
    ///
    /// Consulta pura de solo lectura: no modifica `Player`, `Level`
    /// ni ningún otro estado, no carga niveles y no decide la
    /// transición de estado de la aplicación (eso es
    /// responsabilidad de `App`). Es la única fuente de verdad para
    /// "¿se completó el nivel?"; no existe un booleano
    /// `victory`/`completed` duplicado en ningún otro lugar.
    pub(crate) fn has_reached_goal(&self, block_size: usize) -> bool {
        let (goal_row, goal_column) = self.level.goal();

        point_reaches_goal(
            self.player.pos.x,
            self.player.pos.y,
            goal_row,
            goal_column,
            block_size,
        )
    }
}

/// Convierte una posición de mundo (píxeles) a su celda de
/// cuadrícula `(fila, columna)`, con el mismo convenio
/// `floor(coordenada / block_size)` que usan `raycasting::caster` y
/// `world::collision`. `block_size == 0` o coordenadas no
/// finitas/negativas se resuelven de forma segura a `(0, 0)` en vez
/// de entrar en pánico: `DistanceField::from_level` ya trata
/// cualquier origen fuera de rango o no transitable como
/// "inalcanzable", así que un valor degenerado aquí nunca produce
/// persecución incorrecta, solo la desactiva con seguridad.
fn world_to_cell(position: Vector2, block_size: usize) -> (usize, usize) {
    if block_size == 0 || !position.x.is_finite() || !position.y.is_finite() {
        return (0, 0);
    }

    let column = (position.x / block_size as f32).floor().max(0.0) as usize;

    let row = (position.y / block_size as f32).floor().max(0.0) as usize;

    (row, column)
}

/// Centro, en píxeles de mundo, de la celda `(row, column)`. Misma
/// convención de centrado que `Player::from_level`/
/// `Entity::dealer_at_cell`/`rendering::sprites::cell_center`.
fn cell_center(row: usize, column: usize, block_size: usize) -> Vector2 {
    let half_block = block_size as f32 / 2.0;

    Vector2::new(
        column as f32 * block_size as f32 + half_block,
        row as f32 * block_size as f32 + half_block,
    )
}

/// Comprueba si el punto de mundo `(player_x, player_y)` cae dentro
/// de la celda de meta `(goal_row, goal_column)`, usando el mismo
/// convenio fila/columna que el resto del proyecto
/// (`column * block_size <= x < (column + 1) * block_size`, y
/// análogamente para `y`/fila).
///
/// Función pura y libre de E/S, extraída de `has_reached_goal` para
/// poder probar directamente todos los casos límite sin construir
/// un `Level`/`Player`/`GameSession` completo.
///
/// Retorna `false` de forma segura (sin pánico ni división por
/// cero) para `block_size == 0`, coordenadas no finitas, o
/// coordenadas negativas.
fn point_reaches_goal(
    player_x: f32,
    player_y: f32,
    goal_row: usize,
    goal_column: usize,
    block_size: usize,
) -> bool {
    if block_size == 0 {
        return false;
    }

    if !player_x.is_finite() || !player_y.is_finite() {
        return false;
    }

    if player_x < 0.0 || player_y < 0.0 {
        return false;
    }

    let column = (player_x / block_size as f32).floor() as usize;

    let row = (player_y / block_size as f32).floor() as usize;

    row == goal_row && column == goal_column
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    const BLOCK_SIZE: usize = 48;

    static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Guardia RAII mínima para un archivo de nivel temporal, mismo
    /// patrón std-only ya establecido en `world::pathfinding`/las
    /// pruebas de integración: nombre único vía PID + contador,
    /// limpieza automática al salir de alcance.
    struct TempLevelFile {
        path: PathBuf,
    }

    impl TempLevelFile {
        fn write(contents: &str) -> Self {
            let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);

            let file_name = format!(
                "red_black_maze_session_test_{}_{counter}.txt",
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

    fn new_test_session() -> GameSession {
        let map = "\
#######
#p   g#
#######
";

        let file = TempLevelFile::write(map);

        let level = Level::load(file.path_str()).expect("el nivel de prueba debe cargar");

        let player = Player::from_level(&level, BLOCK_SIZE);

        GameSession::new(level, player, BLOCK_SIZE)
    }

    // --- Tarea 43: propagación de la aceptación de recarga hasta el
    // punto donde `App` decide si reproducir `SoundEffect::Reload`. ---

    #[test]
    fn try_start_weapon_reload_forwards_a_valid_acceptance() {
        let mut session = new_test_session();

        assert!(session.try_fire_weapon());

        // Vuelve a `Idle` (Fire -> Recoil -> Idle) antes de intentar
        // recargar: `try_start_reload` solo se acepta desde `Idle`.
        session.update_weapon(1.0);

        assert!(session.try_start_weapon_reload());
        assert_eq!(session.weapon_state(), WeaponState::Reload);
    }

    #[test]
    fn try_start_weapon_reload_forwards_rejection_on_full_magazine() {
        let mut session = new_test_session();

        assert!(!session.try_start_weapon_reload());
        assert_eq!(session.weapon_state(), WeaponState::Idle);
    }

    #[test]
    fn try_start_weapon_reload_forwards_rejection_while_already_reloading() {
        let mut session = new_test_session();

        assert!(session.try_fire_weapon());
        session.update_weapon(1.0);

        assert!(session.try_start_weapon_reload());

        // Segunda solicitud en el mismo cuadro de recarga: debe
        // rechazarse, exactamente el evento que NO debe producir un
        // segundo `SoundEffect::Reload`.
        assert!(!session.try_start_weapon_reload());
    }

    #[test]
    fn weapon_reload_progress_forwards_none_and_some_correctly() {
        let mut session = new_test_session();

        assert_eq!(session.weapon_reload_progress(), None);

        assert!(session.try_fire_weapon());
        session.update_weapon(1.0);

        assert!(session.try_start_weapon_reload());

        assert!(session.weapon_reload_progress().is_some());
    }

    #[test]
    fn player_center_inside_goal_cell_is_true() {
        assert!(point_reaches_goal(
            3.0 * 48.0 + 24.0,
            2.0 * 48.0 + 24.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn player_center_inside_adjacent_cell_is_false() {
        assert!(!point_reaches_goal(
            4.0 * 48.0 + 24.0,
            2.0 * 48.0 + 24.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn position_just_before_goal_cell_boundary_is_false() {
        let just_before = 3.0 * 48.0 - 0.001;

        assert!(!point_reaches_goal(
            just_before,
            2.0 * 48.0 + 24.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn position_at_lower_left_inclusive_boundary_is_true() {
        assert!(point_reaches_goal(3.0 * 48.0, 2.0 * 48.0, 2, 3, BLOCK_SIZE));
    }

    #[test]
    fn position_at_upper_right_exclusive_boundary_is_false() {
        // Exactamente en el borde superior/derecho de la celda meta
        // ya pertenece, por convención [min, max), a la SIGUIENTE
        // celda (fila 3, columna 4), no a la celda meta (2, 3).
        assert!(!point_reaches_goal(
            4.0 * 48.0,
            3.0 * 48.0,
            2,
            3,
            BLOCK_SIZE
        ));
    }

    #[test]
    fn zero_block_size_is_false() {
        assert!(!point_reaches_goal(
            3.0 * 48.0 + 24.0,
            2.0 * 48.0 + 24.0,
            2,
            3,
            0
        ));
    }

    #[test]
    fn non_finite_position_is_false() {
        assert!(!point_reaches_goal(f32::NAN, 0.0, 0, 0, BLOCK_SIZE));
        assert!(!point_reaches_goal(0.0, f32::INFINITY, 0, 0, BLOCK_SIZE));
    }

    #[test]
    fn negative_position_is_false() {
        assert!(!point_reaches_goal(-1.0, 0.0, 0, 0, BLOCK_SIZE));
        assert!(!point_reaches_goal(0.0, -1.0, 0, 0, BLOCK_SIZE));
    }
}
