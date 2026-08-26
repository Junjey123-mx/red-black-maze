/// Generador congruente simple determinista (SplitMix64) — sin
/// dependencia externa (`rand` u otro crate). Misma semilla produce
/// exactamente la misma secuencia siempre.
///
/// Compartido entre `world::level_generator` (maze/entidades/pickups
/// de "The Dealer's True Maze") y `game::hand` (spawns de Hands
/// adicionales en los cuatro niveles): única implementación de RNG
/// determinista del proyecto — nunca duplicada.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);

        let mut z = self.0;

        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);

        z ^ (z >> 31)
    }

    /// Entero uniforme en `[0, upper)`. `upper == 0` retorna `0` sin
    /// entrar en pánico (no hay elección posible).
    pub(crate) fn gen_range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }

        (self.next_u64() % upper as u64) as usize
    }

    pub(crate) fn choice<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.gen_range(items.len())]
    }

    /// Baraja `items` in-place (Fisher-Yates), determinista.
    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.gen_range(i + 1);

            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_the_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);

        for _ in 0..20 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);

        let sequence_a: Vec<u64> = (0..10).map(|_| a.next_u64()).collect();
        let sequence_b: Vec<u64> = (0..10).map(|_| b.next_u64()).collect();

        assert_ne!(sequence_a, sequence_b);
    }

    #[test]
    fn gen_range_zero_upper_never_panics() {
        let mut rng = Rng::new(7);

        assert_eq!(rng.gen_range(0), 0);
    }

    #[test]
    fn shuffle_preserves_the_same_multiset_of_elements() {
        let mut rng = Rng::new(99);

        let mut items = vec![1, 2, 3, 4, 5, 6, 7, 8];

        let original = items.clone();

        rng.shuffle(&mut items);

        let mut sorted_after = items.clone();
        sorted_after.sort();

        let mut sorted_before = original;
        sorted_before.sort();

        assert_eq!(sorted_after, sorted_before);
    }
}
