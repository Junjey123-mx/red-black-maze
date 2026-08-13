use raylib::prelude::Color;

use crate::rendering::framebuffer::Framebuffer;

/// Intervalo de paso de reserva usado únicamente si el llamador
/// suministra un `step_interval` inválido (no finito o `<= 0.0`) al
/// construir la simulación.
const DEFAULT_STEP_INTERVAL: f32 = 0.15;

/// Simulación independiente del Juego de la Vida de Conway,
/// reutilizable como fondo animado de pantallas de UI (Bienvenida,
/// Selección de Nivel, Victoria).
///
/// No depende de `Level`, `Tile`, `Player`, `Entity`, `Weapon`,
/// `GameSession`, `LevelManager` ni de ningún tipo de raycasting: su
/// cuadrícula es un modelo matemático puro, completamente ajeno al
/// mundo del laberinto. `width`/`height` son dimensiones propias de
/// la simulación, nunca derivadas de `Level` ni de `BLOCK_SIZE`.
pub(crate) struct GameOfLife {
    width: usize,
    height: usize,
    cells: Vec<bool>,
    next_cells: Vec<bool>,
    accumulator: f32,
    step_interval: f32,
}

impl GameOfLife {
    /// Crea una simulación `width x height`, con todas las celdas
    /// inicialmente muertas.
    ///
    /// `width == 0` y/o `height == 0` son válidos: producen una
    /// simulación vacía que nunca dibuja ni evoluciona nada, sin
    /// entrar en pánico.
    ///
    /// Un `step_interval` no finito o `<= 0.0` se reemplaza de forma
    /// determinista por `DEFAULT_STEP_INTERVAL`, en vez de producir
    /// un intervalo inválido que corrompería el temporizado.
    pub(crate) fn new(width: usize, height: usize, step_interval: f32) -> Self {
        let cell_count = width * height;

        let step_interval = if step_interval.is_finite() && step_interval > 0.0 {
            step_interval
        } else {
            DEFAULT_STEP_INTERVAL
        };

        Self {
            width,
            height,
            cells: vec![false; cell_count],
            next_cells: vec![false; cell_count],
            accumulator: 0.0,
            step_interval,
        }
    }

    /// Ancho de la cuadrícula, en celdas.
    pub(crate) fn width(&self) -> usize {
        self.width
    }

    /// Alto de la cuadrícula, en celdas.
    pub(crate) fn height(&self) -> usize {
        self.height
    }

    /// Índice plano de `(row, column)` dentro de `cells`/
    /// `next_cells`.
    ///
    /// Privado: solo se invoca después de verificar
    /// `row < height && column < width`, por lo que el resultado
    /// siempre es un índice válido.
    fn index(&self, row: usize, column: usize) -> usize {
        row * self.width + column
    }

    /// Estado de la celda `(row, column)`, o `None` si está fuera de
    /// los límites de la cuadrícula.
    pub(crate) fn cell(&self, row: usize, column: usize) -> Option<bool> {
        if row >= self.height || column >= self.width {
            return None;
        }

        Some(self.cells[self.index(row, column)])
    }

    /// Igual que `cell`, pero trata fuera-de-límites como "muerta"
    /// en vez de `None`; conveniente para el renderer.
    pub(crate) fn is_alive(&self, row: usize, column: usize) -> bool {
        self.cell(row, column).unwrap_or(false)
    }

    /// Establece el estado de `(row, column)`. Fuera de límites se
    /// ignora de forma segura y retorna `false`; en caso contrario
    /// aplica el cambio y retorna `true`.
    ///
    /// No expone la cuadrícula mutable en bruto: esta es la ÚNICA
    /// forma controlada de sembrar/editar celdas desde fuera del
    /// módulo.
    pub(crate) fn set_cell(&mut self, row: usize, column: usize, alive: bool) -> bool {
        if row >= self.height || column >= self.width {
            return false;
        }

        let index = self.index(row, column);

        self.cells[index] = alive;

        true
    }

    /// Marca como vivas todas las coordenadas suministradas.
    /// Coordenadas fuera de límites se ignoran de forma segura.
    pub(crate) fn seed(&mut self, alive_cells: &[(usize, usize)]) {
        for &(row, column) in alive_cells {
            self.set_cell(row, column, true);
        }
    }

    /// Pone todas las celdas en estado muerto.
    pub(crate) fn clear(&mut self) {
        for cell in self.cells.iter_mut() {
            *cell = false;
        }

        for cell in self.next_cells.iter_mut() {
            *cell = false;
        }
    }

    /// Cuenta cuántos de los ocho vecinos de `(row, column)` están
    /// vivos, usando semántica de cuadrícula FINITA: cualquier
    /// vecino fuera de los límites cuenta como muerto. NO existe
    /// envoltura toroidal (el borde izquierdo nunca es vecino del
    /// borde derecho, ni el superior del inferior).
    ///
    /// Usa desplazamientos con signo (`isize`) y una comprobación de
    /// límites explícita antes de convertir de vuelta a `usize`,
    /// evitando cualquier resta sin signo insegura.
    fn count_alive_neighbors(&self, row: usize, column: usize) -> u8 {
        let mut count = 0u8;

        for delta_row in -1isize..=1 {
            for delta_column in -1isize..=1 {
                if delta_row == 0 && delta_column == 0 {
                    continue;
                }

                let neighbor_row = row as isize + delta_row;

                let neighbor_column = column as isize + delta_column;

                if neighbor_row < 0 || neighbor_column < 0 {
                    continue;
                }

                let neighbor_row = neighbor_row as usize;

                let neighbor_column = neighbor_column as usize;

                if neighbor_row >= self.height || neighbor_column >= self.width {
                    continue;
                }

                if self.cells[self.index(neighbor_row, neighbor_column)] {
                    count += 1;
                }
            }
        }

        count
    }

    /// Avanza exactamente una generación de Conway.
    ///
    /// Reglas estándar:
    ///
    /// - Celda viva con menos de 2 vecinas vivas -> muere
    ///   (subpoblación).
    /// - Celda viva con 2 o 3 vecinas vivas -> sobrevive.
    /// - Celda viva con más de 3 vecinas vivas -> muere
    ///   (sobrepoblación).
    /// - Celda muerta con exactamente 3 vecinas vivas -> nace.
    /// - Cualquier otro caso de celda muerta -> permanece muerta.
    ///
    /// Cada celda se evalúa contra la generación ANTERIOR completa:
    /// los resultados se escriben en `next_cells` (nunca en
    /// `cells` mientras se recorre), y solo al final se
    /// intercambian los dos buffers. Esto evita que una celda ya
    /// actualizada contamine la evaluación de sus vecinas dentro de
    /// la misma generación.
    pub(crate) fn step(&mut self) {
        if self.width == 0 || self.height == 0 {
            return;
        }

        for row in 0..self.height {
            for column in 0..self.width {
                let alive = self.cells[self.index(row, column)];

                let neighbors = self.count_alive_neighbors(row, column);

                let next_alive = matches!((alive, neighbors), (true, 2) | (true, 3) | (false, 3));

                let index = self.index(row, column);

                self.next_cells[index] = next_alive;
            }
        }

        std::mem::swap(&mut self.cells, &mut self.next_cells);
    }

    /// Avanza la simulación según el tiempo real transcurrido,
    /// usando un acumulador: cada `step_interval` acumulado dispara
    /// exactamente una generación (`step`), y el remanente
    /// fraccional se conserva para la siguiente llamada, de modo
    /// que la velocidad de la simulación no dependa del framerate.
    ///
    /// Un `delta_time` no finito o `<= 0.0` se ignora sin alterar el
    /// acumulador ni la cuadrícula. Un `delta_time` grande puede
    /// disparar varias generaciones en una sola llamada.
    pub(crate) fn update(&mut self, delta_time: f32) {
        if !delta_time.is_finite() || delta_time <= 0.0 {
            return;
        }

        self.accumulator += delta_time;

        while self.accumulator >= self.step_interval {
            self.step();

            self.accumulator -= self.step_interval;
        }
    }

    /// Dibuja las celdas vivas de la simulación como rectángulos
    /// rellenos, según `config`.
    ///
    /// Puramente presentación: solo LEE el estado ya calculado por
    /// `step`/`update`, nunca lo modifica. Cada escritura de píxel
    /// pasa por `Framebuffer::point`, que ya recorta coordenadas
    /// fuera de rango, por lo que una región de dibujo parcial o
    /// totalmente fuera de pantalla no puede entrar en pánico ni
    /// producir una escritura fuera de límites.
    pub(crate) fn render(&self, framebuffer: &mut Framebuffer, config: &GameOfLifeRenderConfig) {
        if config.cell_size <= 0 {
            return;
        }

        let gap = config.cell_gap.clamp(0, config.cell_size - 1);

        let draw_size = config.cell_size - gap;

        framebuffer.set_current_color(config.alive_color);

        for row in 0..self.height {
            for column in 0..self.width {
                if !self.cells[self.index(row, column)] {
                    continue;
                }

                let x0 = config.origin_x + column as i32 * config.cell_size;

                let y0 = config.origin_y + row as i32 * config.cell_size;

                for offset_y in 0..draw_size {
                    for offset_x in 0..draw_size {
                        framebuffer.point(x0 + offset_x, y0 + offset_y);
                    }
                }
            }
        }
    }
}

/// Configuración de dibujo del `GameOfLife`, independiente de la
/// simulación en sí.
///
/// Deliberadamente genérica (origen, tamaño de celda, separación,
/// color de celda viva): NO contiene campos específicos de pantalla
/// como `welcome_color` o `selected_level`, para que Bienvenida,
/// Selección de Nivel y Victoria puedan reutilizarla sin
/// modificaciones, simplemente variando estos valores.
pub(crate) struct GameOfLifeRenderConfig {
    /// Coordenada X del framebuffer donde comienza la cuadrícula.
    pub(crate) origin_x: i32,

    /// Coordenada Y del framebuffer donde comienza la cuadrícula.
    pub(crate) origin_y: i32,

    /// Tamaño, en píxeles, de cada celda (incluyendo cualquier
    /// separación).
    pub(crate) cell_size: i32,

    /// Separación, en píxeles, entre celdas contiguas. Se recorta
    /// de forma segura a `[0, cell_size - 1]` antes de dibujar.
    pub(crate) cell_gap: i32,

    /// Color de las celdas vivas. Las celdas muertas nunca se
    /// dibujan, para que el llamador conserve el control total del
    /// fondo (p. ej. negro sólido de pantalla de UI).
    pub(crate) alive_color: Color,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rojo de ejemplo dentro de la paleta sugerida del proyecto;
    /// las pruebas no verifican color, solo lo necesitan para
    /// construir una configuración válida.
    fn test_render_config() -> GameOfLifeRenderConfig {
        GameOfLifeRenderConfig {
            origin_x: 0,
            origin_y: 0,
            cell_size: 4,
            cell_gap: 0,
            alive_color: Color::new(200, 30, 40, 255),
        }
    }

    #[test]
    fn zero_sized_grid_never_panics() {
        let mut game = GameOfLife::new(0, 0, 0.1);

        game.step();
        game.update(1.0);
        game.seed(&[(0, 0)]);
        game.clear();

        assert_eq!(game.cell(0, 0), None);

        let _config = test_render_config();
    }

    #[test]
    fn out_of_bounds_cell_query_is_none() {
        let game = GameOfLife::new(3, 3, 0.1);

        assert_eq!(game.cell(3, 0), None);
        assert_eq!(game.cell(0, 3), None);
        assert_eq!(game.cell(100, 100), None);
    }

    #[test]
    fn out_of_bounds_set_cell_is_ignored_safely() {
        let mut game = GameOfLife::new(3, 3, 0.1);

        assert!(!game.set_cell(3, 3, true));
        assert_eq!(game.cell(3, 3), None);
    }

    #[test]
    fn underpopulation_kills_isolated_live_cell() {
        let mut game = GameOfLife::new(3, 3, 0.1);

        game.seed(&[(1, 1)]);

        game.step();

        assert!(!game.is_alive(1, 1));
    }

    #[test]
    fn survives_with_two_live_neighbors() {
        // Fila central de un blinker: (1,1),(1,2),(1,3) vivas. La
        // celda (1,2) tiene exactamente 2 vecinas vivas ((1,1) y
        // (1,3)); ninguna diagonal/vertical está viva.
        let mut game = GameOfLife::new(5, 3, 0.1);

        game.seed(&[(1, 1), (1, 2), (1, 3)]);

        assert_eq!(game.count_alive_neighbors(1, 2), 2);

        game.step();

        assert!(game.is_alive(1, 2));
    }

    #[test]
    fn survives_with_three_live_neighbors() {
        // Bloque 2x2: cada celda tiene exactamente 3 vecinas vivas
        // dentro del bloque.
        let mut game = GameOfLife::new(4, 4, 0.1);

        game.seed(&[(1, 1), (1, 2), (2, 1), (2, 2)]);

        assert_eq!(game.count_alive_neighbors(1, 1), 3);

        game.step();

        assert!(game.is_alive(1, 1));
    }

    #[test]
    fn overpopulation_kills_live_cell_with_more_than_three_neighbors() {
        // Centro vivo rodeado de 4 vivas (cruz completa alrededor).
        let mut game = GameOfLife::new(3, 3, 0.1);

        game.seed(&[(0, 1), (1, 0), (1, 1), (1, 2), (2, 1)]);

        assert_eq!(game.count_alive_neighbors(1, 1), 4);

        game.step();

        assert!(!game.is_alive(1, 1));
    }

    #[test]
    fn birth_occurs_with_exactly_three_live_neighbors() {
        let mut game = GameOfLife::new(3, 3, 0.1);

        // (1,1) muerta, con exactamente 3 vecinas vivas.
        game.seed(&[(0, 0), (0, 1), (0, 2)]);

        assert!(!game.is_alive(1, 1));
        assert_eq!(game.count_alive_neighbors(1, 1), 3);

        game.step();

        assert!(game.is_alive(1, 1));
    }

    #[test]
    fn still_life_block_is_unchanged_after_one_generation() {
        let mut game = GameOfLife::new(4, 4, 0.1);

        let block = [(1, 1), (1, 2), (2, 1), (2, 2)];

        game.seed(&block);

        game.step();

        for row in 0..4 {
            for column in 0..4 {
                let expected = block.contains(&(row, column));

                assert_eq!(
                    game.is_alive(row, column),
                    expected,
                    "celda ({row},{column}) no coincide con el bloque estable"
                );
            }
        }
    }

    #[test]
    fn blinker_oscillates_between_horizontal_and_vertical() {
        let mut game = GameOfLife::new(5, 5, 0.1);

        // Generación A: fila horizontal centrada.
        game.seed(&[(2, 1), (2, 2), (2, 3)]);

        game.step();

        // Generación B: columna vertical centrada.
        assert!(game.is_alive(1, 2));
        assert!(game.is_alive(2, 2));
        assert!(game.is_alive(3, 2));
        assert!(!game.is_alive(2, 1));
        assert!(!game.is_alive(2, 3));

        game.step();

        // Vuelve a la generación A.
        assert!(game.is_alive(2, 1));
        assert!(game.is_alive(2, 2));
        assert!(game.is_alive(2, 3));
        assert!(!game.is_alive(1, 2));
        assert!(!game.is_alive(3, 2));
    }

    #[test]
    fn grid_boundaries_do_not_wrap() {
        // Vivas en la columna 0 (borde izquierdo) de una fila
        // interior, y en la columna width-1 (borde derecho) de la
        // MISMA fila. Si hubiera envoltura toroidal, cada una
        // contaría a la otra como vecina; sin envoltura, no.
        let mut game = GameOfLife::new(3, 3, 0.1);

        game.seed(&[(1, 0), (1, 2)]);

        assert_eq!(game.count_alive_neighbors(1, 0), 0);
        assert_eq!(game.count_alive_neighbors(1, 2), 0);

        // Esquina superior-izquierda: no debe contar vecinos fuera
        // de rango ni envolver hacia la esquina opuesta.
        let mut corner_game = GameOfLife::new(3, 3, 0.1);

        corner_game.seed(&[(2, 2)]);

        assert_eq!(corner_game.count_alive_neighbors(0, 0), 0);
    }

    #[test]
    fn update_does_not_step_before_interval_is_reached() {
        let mut game = GameOfLife::new(3, 3, 0.10);

        // Célula aislada: si se ejecutara un step(), moriría por
        // subpoblación. Confirmar que sigue viva prueba que NO se
        // ejecutó ningún step.
        game.seed(&[(1, 1)]);

        game.update(0.05);

        assert!(game.is_alive(1, 1));
    }

    #[test]
    fn update_steps_once_when_interval_threshold_is_reached() {
        let mut game = GameOfLife::new(3, 3, 0.10);

        game.seed(&[(1, 1)]);

        game.update(0.10);

        assert!(!game.is_alive(1, 1));
    }

    #[test]
    fn update_with_large_delta_advances_multiple_generations() {
        let mut game = GameOfLife::new(5, 5, 0.10);

        // Blinker horizontal; tres generaciones deben dejarlo en
        // orientación vertical (A -> B -> A -> B).
        game.seed(&[(2, 1), (2, 2), (2, 3)]);

        game.update(0.35);

        assert!(game.is_alive(1, 2));
        assert!(game.is_alive(2, 2));
        assert!(game.is_alive(3, 2));
        assert!(!game.is_alive(2, 1));
        assert!(!game.is_alive(2, 3));
    }

    #[test]
    fn invalid_delta_time_is_ignored_safely() {
        let mut game = GameOfLife::new(3, 3, 0.10);

        game.seed(&[(1, 1)]);

        game.update(-1.0);
        game.update(f32::NAN);
        game.update(f32::INFINITY);

        // Ninguna llamada inválida debe haber disparado un step ni
        // corrompido el acumulador: la célula aislada sigue viva, y
        // una actualización válida posterior por debajo del umbral
        // tampoco dispara un step.
        assert!(game.is_alive(1, 1));

        game.update(0.05);

        assert!(game.is_alive(1, 1));
    }

    #[test]
    fn invalid_step_interval_falls_back_to_default() {
        let mut game_zero = GameOfLife::new(3, 3, 0.0);

        let mut game_negative = GameOfLife::new(3, 3, -1.0);

        let mut game_nan = GameOfLife::new(3, 3, f32::NAN);

        for game in [&mut game_zero, &mut game_negative, &mut game_nan] {
            game.seed(&[(1, 1)]);

            // Un delta menor que cualquier intervalo de reserva
            // razonable no debe disparar un step.
            game.update(0.001);

            assert!(game.is_alive(1, 1));
        }
    }
}
