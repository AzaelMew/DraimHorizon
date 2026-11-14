// Vertex runs of static interface shapes (sidebar backgrounds, stripes, Lucide strokes).
//
// A run is replayed only for the exact same shape, colour, rectangle and atlas; the vertices are the
// ones drawing produced, so a hit is indistinguishable from drawing again. Text and values that
// change at runtime are never stored here.
use std::{collections::HashMap, sync::Arc};

const MAX_RUNS: usize = 256;
const MAX_VERTICES: usize = 48 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    /// Identity of the atlas the shape was drawn against (its white texel and UVs).
    pub atlas: u64,
    /// Which drawing routine and shape (rounded fill, a Lucide icon, ...).
    pub shape: u32,
    pub rect: [u32; 4],
    pub argb: u32,
    /// Anything else the routine reads (theme accent, alpha, shadows).
    pub extra: u64,
}
impl Key {
    pub fn new(atlas: u64, shape: u32, rect: [f32; 4], argb: u32, extra: u64) -> Self {
        Self {
            atlas,
            shape,
            rect: rect.map(f32::to_bits),
            argb,
            extra,
        }
    }
}

pub struct Shapes<V: Copy> {
    runs: HashMap<Key, Arc<[V]>>,
    vertices: usize,
    pub hits: u64,
    pub misses: u64,
}
impl<V: Copy> Default for Shapes<V> {
    fn default() -> Self {
        Self {
            runs: HashMap::new(),
            vertices: 0,
            hits: 0,
            misses: 0,
        }
    }
}
impl<V: Copy> Shapes<V> {
    pub fn get(&mut self, key: &Key) -> Option<Arc<[V]>> {
        let run = self.runs.get(key).cloned();
        if run.is_some() {
            self.hits += 1;
        }
        run
    }
    pub fn insert(&mut self, key: Key, run: &[V]) {
        self.misses += 1;
        // The old run of this key leaves the budget first, whatever the new length is, so a
        // replacement is never counted as a new run and never leaves a stale run behind.
        if let Some(old) = self.runs.remove(&key) {
            self.vertices -= old.len();
        }
        if run.is_empty() || run.len() > MAX_VERTICES / 4 {
            return;
        }
        if self.runs.len() >= MAX_RUNS || self.vertices + run.len() > MAX_VERTICES {
            self.runs.clear();
            self.vertices = 0;
        }
        self.runs.insert(key, run.into());
        self.vertices += run.len();
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.runs.len()
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct V([f32; 2], u32);

    /// A stand-in for a drawing routine whose output depends on every key field.
    fn draw(key: &Key, out: &mut Vec<V>) {
        let [x, y, w, h] = key.rect.map(f32::from_bits);
        for i in 0..20 {
            out.push(V(
                [x + i as f32, y + w * h],
                key.argb ^ key.extra as u32 ^ key.atlas as u32,
            ));
        }
    }
    fn frame(cache: &mut Shapes<V>, key: Key, batch: &mut Vec<V>) {
        match cache.get(&key) {
            Some(run) => batch.extend_from_slice(&run),
            None => {
                let start = batch.len();
                draw(&key, batch);
                cache.insert(key, &batch[start..]);
            }
        }
    }

    #[test]
    fn a_replayed_run_equals_drawing_again_in_order_with_other_geometry() {
        let mut cache = Shapes::default();
        let keys: Vec<_> = (0..5)
            .map(|i| {
                Key::new(
                    1,
                    i,
                    [4.0, 8.0 + i as f32 * 14.0, 90.0, 12.0],
                    0x14ffffff,
                    7,
                )
            })
            .collect();
        let (mut first, mut second) = (Vec::new(), Vec::new());
        for pass in [&mut first, &mut second] {
            for key in &keys {
                // Dynamic geometry between static runs is drawn live and stays in order.
                pass.push(V([-1.0, -1.0], 0xdead));
                frame(&mut cache, *key, pass);
            }
        }
        assert_eq!(first, second);
        let mut live = Vec::new();
        for key in &keys {
            live.push(V([-1.0, -1.0], 0xdead));
            draw(key, &mut live);
        }
        assert_eq!(second, live);
        assert_eq!((cache.misses, cache.hits), (5, 5));
    }

    #[test]
    fn atlas_colour_rect_theme_and_shape_each_select_their_own_run() {
        let base = Key::new(1, 3, [4.0, 8.0, 90.0, 12.0], 0xff8da5b8, 5);
        let mut cache = Shapes::default();
        cache.insert(base, &[V([0.0, 0.0], 1)]);
        assert!(cache.get(&base).is_some());
        for other in [
            Key { atlas: 2, ..base },
            Key { shape: 4, ..base },
            Key {
                argb: 0xff000000,
                ..base
            },
            Key { extra: 6, ..base },
            Key::new(1, 3, [4.0, 8.5, 90.0, 12.0], 0xff8da5b8, 5),
            Key::new(1, 3, [4.0, 8.0, 90.0, 12.0], 0xff8da5b8, 5).with_x(5.0),
        ] {
            assert!(cache.get(&other).is_none(), "{other:?}");
        }
    }
    impl Key {
        fn with_x(mut self, x: f32) -> Self {
            self.rect[0] = x.to_bits();
            self
        }
    }

    #[test]
    fn the_cache_stays_bounded_however_many_distinct_shapes_are_drawn() {
        let mut cache = Shapes::default();
        for i in 0..5_000u32 {
            cache.insert(
                Key::new(1, 0, [i as f32, 0.0, 1.0, 1.0], 0, 0),
                &[V([0.0; 2], 0); 120],
            );
            assert!(cache.len() <= MAX_RUNS);
            assert!(cache.vertices <= MAX_VERTICES);
        }
        cache.insert(
            Key::new(1, 1, [0.0; 4], 0, 0),
            &vec![V([0.0; 2], 0); MAX_VERTICES],
        );
        assert!(cache.get(&Key::new(1, 1, [0.0; 4], 0, 0)).is_none());
    }

    fn key(i: u32) -> Key {
        Key::new(1, i, [0.0; 4], 0, 0)
    }
    fn counted(cache: &Shapes<V>) -> usize {
        cache.runs.values().map(|run| run.len()).sum()
    }

    #[test]
    fn replacing_a_run_adjusts_the_budget_for_growth_and_shrinkage() {
        let mut cache = Shapes::default();
        cache.insert(key(1), &[V([0.0; 2], 1); 10]);
        cache.insert(key(2), &[V([0.0; 2], 2); 5]);
        cache.insert(key(1), &[V([0.0; 2], 3); 30]);
        assert_eq!((cache.vertices, counted(&cache)), (35, 35));
        cache.insert(key(1), &[V([0.0; 2], 4); 4]);
        assert_eq!((cache.vertices, counted(&cache)), (9, 9));
        // Identical replacement: no drift, the new vertices are the stored ones.
        for _ in 0..1000 {
            cache.insert(key(1), &[V([0.0; 2], 4); 4]);
        }
        assert_eq!((cache.vertices, counted(&cache), cache.len()), (9, 9, 2));
        assert_eq!(&*cache.get(&key(1)).unwrap(), &[V([0.0; 2], 4); 4]);
        assert_eq!(&*cache.get(&key(2)).unwrap(), &[V([0.0; 2], 2); 5]);
    }

    #[test]
    fn shrinking_replacements_never_underflow_the_budget() {
        let mut cache = Shapes::default();
        cache.insert(key(1), &[V([0.0; 2], 0); 100]);
        for len in [90, 50, 1, 70, 2] {
            cache.insert(key(1), &vec![V([0.0; 2], 0); len]);
            assert_eq!((cache.vertices, counted(&cache)), (len, len));
        }
    }

    #[test]
    fn replacement_at_the_count_limit_keeps_the_other_runs() {
        let mut cache = Shapes::default();
        for i in 0..MAX_RUNS as u32 {
            cache.insert(key(i), &[V([0.0; 2], 0); 3]);
        }
        cache.insert(key(7), &[V([0.0; 2], 9); 5]);
        assert_eq!(cache.len(), MAX_RUNS, "a replacement is not a new run");
        assert_eq!(cache.vertices, MAX_RUNS * 3 + 2);
        assert!(cache.get(&key(0)).is_some() && cache.get(&key(255)).is_some());
        assert_eq!(&*cache.get(&key(7)).unwrap(), &[V([0.0; 2], 9); 5]);
    }

    #[test]
    fn growth_beyond_the_remaining_budget_evicts_instead_of_overflowing() {
        let mut cache = Shapes::default();
        let part = MAX_VERTICES / 4;
        for i in 1..4 {
            cache.insert(key(i), &vec![V([0.0; 2], 0); part]);
        }
        cache.insert(key(0), &[V([0.0; 2], 0); 10]);
        cache.insert(key(4), &[V([0.0; 2], 0); 10]);
        // Growing key 0 to a full part no longer fits next to the others.
        cache.insert(key(0), &vec![V([0.0; 2], 1); part]);
        assert!(cache.vertices <= MAX_VERTICES);
        assert_eq!(cache.vertices, counted(&cache));
        assert_eq!(
            &*cache.get(&key(0)).unwrap(),
            &vec![V([0.0; 2], 1); part][..]
        );
    }
}
