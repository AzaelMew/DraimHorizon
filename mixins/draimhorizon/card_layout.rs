// Wrapped rows of one chat card and the cache that keeps them until an input changes.
//
// The cache is keyed by the structural inputs of the layout (not by a hash alone): a hash only
// selects a bucket and the stored inputs are compared before rows are reused.
use super::{
    claim_borders_chat_feed::{self as model, AuctionNotice, Entry, Kind},
    claim_borders_chat_preferences::PresenceStyle,
};
use claim_borders_relay_client::translation::Language;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::Arc,
};

const MAX_ENTRIES: usize = 384;
const MAX_CHARS: usize = 192 * 1024;
const MAX_ENTRY_CHARS: usize = 16 * 1024;

/// Identity of the font the rows were measured with. `epoch` already differs for every font ever
/// built, but the atlas revision is part of the identity too so a rebuilt atlas can never reuse
/// rows measured under another one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FontId {
    pub atlas: u64,
    pub epoch: u64,
}

/// Which rows of a card are asked for: all wrapped rows, or the rows cut to a row budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Variant {
    Full,
    Cut(u32),
}

/// How the card is presented; everything here changes the text of the rows.
#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub language: Language,
    /// The language `current_language()` reports for labels.
    pub ui_language: Language,
    pub translate: bool,
    pub presence_style: PresenceStyle,
    pub compact: bool,
    pub inline: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Key {
    font: FontId,
    variant: Variant,
    flags: u8,
    kind: u8,
    joined: Option<bool>,
    repetitions: u32,
    language_ru: bool,
    ui_ru: bool,
    width: u32,
    auction: Option<AuctionNotice>,
    name: String,
    original: String,
    translated: Option<String>,
}

/// The inputs of a layout, borrowed from the entry.
pub struct Inputs<'a> {
    font: FontId,
    variant: Variant,
    flags: u8,
    kind: u8,
    joined: Option<bool>,
    repetitions: u32,
    language_ru: bool,
    ui_ru: bool,
    width: u32,
    auction: Option<AuctionNotice>,
    name: &'a str,
    original: &'a str,
    translated: Option<&'a str>,
    /// Digest of the three strings above, prepared once per card query and shared by every
    /// variant: it only selects a bucket, the strings themselves are compared on a lookup.
    content: u64,
}
fn content_digest(name: &str, original: &str, translated: Option<&str>) -> u64 {
    #[cfg(test)]
    content_hashes::count();
    let mut h = DefaultHasher::new();
    (name, original, translated).hash(&mut h);
    h.finish()
}
fn kind_code(kind: Kind) -> u8 {
    match kind {
        Kind::Message => 0,
        Kind::Whisper(model::Whisper::Incoming) => 1,
        Kind::Whisper(model::Whisper::Outgoing) => 2,
        Kind::Presence => 3,
        Kind::Death => 4,
        Kind::System => 5,
    }
}
impl<'a> Inputs<'a> {
    pub fn new(e: &'a Entry, ctx: &Context, font: FontId, width: f32, variant: Variant) -> Self {
        let translated = e.translated.as_deref();
        Self {
            font,
            variant,
            flags: u8::from(ctx.compact)
                | u8::from(ctx.inline) << 1
                | u8::from(ctx.translate) << 2
                | u8::from(e.show_original) << 3
                | u8::from(ctx.presence_style == PresenceStyle::Icons) << 4,
            kind: kind_code(e.kind),
            joined: e.joined,
            repetitions: e.repetitions,
            language_ru: ctx.language == Language::Russian,
            ui_ru: ctx.ui_language == Language::Russian,
            width: width.to_bits(),
            auction: e.auction,
            name: &e.name,
            original: &e.original,
            translated,
            content: content_digest(&e.name, &e.original, translated),
        }
    }
    /// The same inputs for another variant of the same card.
    pub fn variant(&self, variant: Variant) -> Self {
        Self { variant, ..*self }
    }
    fn hash(&self) -> u64 {
        let mut h = DefaultHasher::new();
        (self.font, self.variant, self.flags, self.kind, self.joined).hash(&mut h);
        let auction = self.auction.map(|a| a.label_in(Language::English));
        (
            self.repetitions,
            self.language_ru,
            self.ui_ru,
            self.width,
            auction,
        )
            .hash(&mut h);
        self.content.hash(&mut h);
        h.finish()
    }
    fn matches(&self, key: &Key) -> bool {
        self.font == key.font
            && self.variant == key.variant
            && self.flags == key.flags
            && self.kind == key.kind
            && self.joined == key.joined
            && self.repetitions == key.repetitions
            && self.language_ru == key.language_ru
            && self.ui_ru == key.ui_ru
            && self.width == key.width
            && self.auction == key.auction
            && self.name == key.name
            && self.original == key.original
            && self.translated == key.translated.as_deref()
    }
    fn into_key(self) -> Key {
        Key {
            font: self.font,
            variant: self.variant,
            flags: self.flags,
            kind: self.kind,
            joined: self.joined,
            repetitions: self.repetitions,
            language_ru: self.language_ru,
            ui_ru: self.ui_ru,
            width: self.width,
            auction: self.auction,
            name: self.name.to_owned(),
            original: self.original.to_owned(),
            translated: self.translated.map(str::to_owned),
        }
    }
    fn size(&self) -> usize {
        self.name.len() + self.original.len() + self.translated.map_or(0, str::len)
    }
}
impl Clone for Inputs<'_> {
    fn clone(&self) -> Self {
        *self
    }
}
impl Copy for Inputs<'_> {}

struct Slot {
    key: Key,
    rows: Arc<Vec<String>>,
    size: usize,
    /// Position in `Cache::order`: the tick of the last hit or insert.
    used: u64,
}
/// Rows kept until an input changes, bounded by count and by characters (rows and key text).
/// Pressure evicts the least recently used rows one by one, so rows that are still painted
/// survive a burst of one-off layouts (e.g. every width of a resize drag).
#[derive(Default)]
pub struct Cache {
    buckets: HashMap<u64, Vec<Slot>>,
    /// Slots by last use (tick -> bucket hash): the oldest is evicted first in O(log n).
    order: BTreeMap<u64, u64>,
    tick: u64,
    entries: usize,
    chars: usize,
    pub hits: u64,
    pub misses: u64,
}
impl Cache {
    pub fn get(&mut self, inputs: &Inputs) -> Option<Arc<Vec<String>>> {
        self.get_in(inputs.hash(), inputs)
    }
    pub fn insert(&mut self, inputs: &Inputs, rows: Arc<Vec<String>>) {
        self.insert_in(inputs.hash(), inputs, rows);
    }
    fn get_in(&mut self, hash: u64, inputs: &Inputs) -> Option<Arc<Vec<String>>> {
        let rows = self.touch(hash, inputs)?;
        self.hits += 1;
        Some(rows)
    }
    /// Marks the matching slot as just used and returns its rows.
    fn touch(&mut self, hash: u64, inputs: &Inputs) -> Option<Arc<Vec<String>>> {
        let slot = self
            .buckets
            .get_mut(&hash)?
            .iter_mut()
            .find(|slot| inputs.matches(&slot.key))?;
        self.tick += 1;
        self.order.remove(&slot.used);
        self.order.insert(self.tick, hash);
        slot.used = self.tick;
        Some(slot.rows.clone())
    }
    fn insert_in(&mut self, hash: u64, inputs: &Inputs, rows: Arc<Vec<String>>) {
        self.misses += 1;
        let size = rows.iter().map(String::len).sum::<usize>() + inputs.size();
        if size > MAX_ENTRY_CHARS || self.touch(hash, inputs).is_some() {
            return;
        }
        while self.entries >= MAX_ENTRIES || self.chars + size > MAX_CHARS {
            if !self.evict_oldest() {
                break;
            }
        }
        self.tick += 1;
        self.order.insert(self.tick, hash);
        self.buckets.entry(hash).or_default().push(Slot {
            key: (*inputs).into_key(),
            rows,
            size,
            used: self.tick,
        });
        self.entries += 1;
        self.chars += size;
    }
    fn evict_oldest(&mut self) -> bool {
        let Some((used, hash)) = self.order.pop_first() else {
            return false;
        };
        if let Some(bucket) = self.buckets.get_mut(&hash) {
            if let Some(at) = bucket.iter().position(|slot| slot.used == used) {
                let slot = bucket.swap_remove(at);
                self.entries -= 1;
                self.chars -= slot.size;
            }
            if bucket.is_empty() {
                self.buckets.remove(&hash);
            }
        }
        true
    }
    pub fn clear(&mut self) {
        self.buckets.clear();
        self.order.clear();
        self.entries = 0;
        self.chars = 0;
    }
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }
    /// Every counter and index recomputed from the stored slots; panics when any has drifted.
    #[cfg(test)]
    fn audit(&self) {
        let (mut entries, mut chars) = (0, 0);
        for slot in self.buckets.values().flatten() {
            entries += 1;
            chars += slot.size;
            assert_eq!(slot.size, chars_of(slot));
        }
        assert_eq!((self.entries, self.chars), (entries, chars));
        assert_eq!(self.order.len(), entries);
        assert!(entries <= MAX_ENTRIES && chars <= MAX_CHARS);
    }
}
#[cfg(test)]
fn chars_of(slot: &Slot) -> usize {
    slot.key.name.len()
        + slot.key.original.len()
        + slot.key.translated.as_ref().map_or(0, String::len)
        + slot.rows.iter().map(String::len).sum::<usize>()
}

/// The name shown before a chat body.
pub fn label(entry: &Entry, ui_language: Language) -> Cow<'_, str> {
    if let Some(notice) = entry.auction {
        notice.label_in(ui_language).into()
    } else if let Kind::Whisper(direction) = entry.kind {
        format!("{} {}", direction.label_in(ui_language), entry.name).into()
    } else if entry.name.is_empty() {
        ui_language.text("Система", "System").into()
    } else {
        entry.name.as_str().into()
    }
}

/// All rows of a card: its formatted body wrapped to `width` (compact cards are one line).
pub fn build(e: &Entry, ctx: &Context, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let count = if e.repetitions > 1 {
        format!(" ×{}", e.repetitions)
    } else {
        String::new()
    };
    let body = if ctx.compact {
        format!(
            "{}{count}",
            if e.kind == Kind::Presence {
                model::presence_summary(e, ctx.presence_style, ctx.language)
            } else {
                model::event_summary(e, ctx.translate, ctx.language)
            }
        )
    } else if ctx.inline {
        let label = label(e, ctx.ui_language);
        format!(
            "{}{}{count}",
            e.chat_prefix(&label, ctx.language),
            e.display_body(ctx.translate, ctx.language)
        )
    } else {
        format!("{}{count}", e.display_body(ctx.translate, ctx.language))
    };
    if ctx.compact {
        vec![model::single_line(&body, width, measure)]
    } else {
        model::wrap(&body, width, measure)
    }
}

/// `full` cut to `max_rows`, the last row ending in an ellipsis that still fits.
pub fn cut(
    full: &[String],
    max_rows: usize,
    width: f32,
    measure: impl Fn(&str) -> f32,
) -> Vec<String> {
    let mut rows = full.to_vec();
    rows.truncate(max_rows);
    if let Some(last) = rows.last_mut() {
        *last = model::single_line(&format!("{last}…"), width, measure);
    }
    rows
}

#[cfg(test)]
mod content_hashes {
    use std::cell::Cell;
    thread_local!(static COUNT: Cell<u32> = const { Cell::new(0) });
    pub fn count() {
        COUNT.with(|c| c.set(c.get() + 1));
    }
    pub fn take() -> u32 {
        COUNT.with(|c| c.replace(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_preferences::Preferences;
    use std::cell::Cell;

    const FONT: FontId = FontId {
        atlas: 1,
        epoch: 10,
    };
    fn ctx() -> Context {
        Context {
            language: Language::English,
            ui_language: Language::English,
            translate: true,
            presence_style: PresenceStyle::default(),
            compact: false,
            inline: true,
        }
    }
    fn feed() -> model::Feed {
        let mut feed = model::Feed::default();
        let p = Preferences::default();
        feed.push_with(
            "<Alex> hello wonderful world, how are you today my friend",
            1,
            p,
        );
        feed.push_with("<Alex> привет", 2, p);
        feed.push_with("Steve joined the game", 3, p);
        feed
    }
    /// What the painter does for a card: look up, build on a miss, store.
    fn rows(
        cache: &mut Cache,
        e: &Entry,
        ctx: &Context,
        font: FontId,
        width: f32,
        measure: &dyn Fn(&str) -> f32,
        builds: &Cell<u32>,
    ) -> Arc<Vec<String>> {
        let inputs = Inputs::new(e, ctx, font, width, Variant::Full);
        if let Some(rows) = cache.get(&inputs) {
            return rows;
        }
        builds.set(builds.get() + 1);
        let rows = Arc::new(build(e, ctx, width, measure));
        cache.insert(&inputs, rows.clone());
        rows
    }
    fn narrow(s: &str) -> f32 {
        s.chars().count() as f32 * 4.0
    }
    fn wide(s: &str) -> f32 {
        s.chars().count() as f32 * 9.0
    }

    #[test]
    fn the_production_rows_match_the_original_painter_formatting() {
        let feed = feed();
        let e = &feed.entries[0];
        let body = build(e, &ctx(), 120.0, narrow);
        assert!(body[0].starts_with("Alex: hello"));
        assert_eq!(
            body,
            model::wrap(
                "Alex: hello wonderful world, how are you today my friend",
                120.0,
                narrow
            )
        );
        let mut compact = ctx();
        compact.compact = true;
        let presence = &feed.entries[2];
        assert_eq!(build(presence, &compact, 400.0, narrow).len(), 1);
        let cut_rows = cut(&body, 1, 120.0, narrow);
        assert_eq!(cut_rows.len(), 1);
        assert!(cut_rows[0].ends_with('…') && narrow(&cut_rows[0]) <= 120.0);
    }

    #[test]
    fn rows_are_reused_until_an_input_changes() {
        let feed = feed();
        let e = &feed.entries[0];
        let (mut cache, builds) = (Cache::default(), Cell::new(0));
        let first = rows(&mut cache, e, &ctx(), FONT, 120.0, &narrow, &builds);
        for _ in 0..100 {
            let again = rows(&mut cache, e, &ctx(), FONT, 120.0, &narrow, &builds);
            assert!(Arc::ptr_eq(&first, &again));
        }
        assert_eq!(builds.get(), 1);
        let mut changed = Vec::new();
        let mut probe = |label: &str, e: &Entry, c: &Context, font: FontId, width: f32| {
            let before = builds.get();
            rows(&mut cache, e, c, font, width, &narrow, &builds);
            changed.push((label.to_owned(), builds.get() == before + 1));
        };
        let mut other = ctx();
        other.translate = false;
        probe("translation toggle", e, &other, FONT, 120.0);
        let mut other = ctx();
        other.language = Language::Russian;
        probe("language", e, &other, FONT, 120.0);
        let mut other = ctx();
        other.ui_language = Language::Russian;
        probe("ui language", e, &other, FONT, 120.0);
        let mut other = ctx();
        other.compact = true;
        probe("compact", e, &other, FONT, 120.0);
        let mut other = ctx();
        other.presence_style = PresenceStyle::Text;
        probe("presence style", e, &other, FONT, 120.0);
        probe("width", e, &ctx(), FONT, 121.0);
        probe("font epoch", e, &ctx(), FontId { epoch: 11, ..FONT }, 120.0);
        probe("atlas", e, &ctx(), FontId { atlas: 2, ..FONT }, 120.0);
        let mut original = e.clone();
        original.show_original = true;
        probe("original toggle", &original, &ctx(), FONT, 120.0);
        let mut translated = e.clone();
        translated.translated = Some("привет всем, как у вас дела сегодня мой друг".into());
        probe("translation text", &translated, &ctx(), FONT, 120.0);
        let mut repeated = e.clone();
        repeated.repetitions = 2;
        probe("repetitions", &repeated, &ctx(), FONT, 120.0);
        let mut renamed = e.clone();
        renamed.name = "Alexa".into();
        probe("author", &renamed, &ctx(), FONT, 120.0);
        for (label, rebuilt) in changed {
            assert!(rebuilt, "{label} must invalidate the rows");
        }
    }

    #[test]
    fn a_rebuilt_font_with_the_same_epoch_but_other_widths_never_reuses_rows() {
        let feed = feed();
        let e = &feed.entries[0];
        let (mut cache, builds) = (Cache::default(), Cell::new(0));
        let before = rows(&mut cache, e, &ctx(), FONT, 150.0, &narrow, &builds);
        // The same epoch, a different atlas revision: wider glyphs wrap into more rows.
        let after = rows(
            &mut cache,
            e,
            &ctx(),
            FontId { atlas: 2, ..FONT },
            150.0,
            &wide,
            &builds,
        );
        assert_eq!(builds.get(), 2);
        assert!(after.len() > before.len());
        // The original font's rows are still intact.
        assert!(Arc::ptr_eq(
            &before,
            &rows(&mut cache, e, &ctx(), FONT, 150.0, &narrow, &builds)
        ));
    }

    #[test]
    fn colliding_hashes_are_told_apart_by_the_stored_inputs() {
        let feed = feed();
        let (a, b) = (&feed.entries[0], &feed.entries[1]);
        let (ia, ib) = (
            Inputs::new(a, &ctx(), FONT, 120.0, Variant::Full),
            Inputs::new(b, &ctx(), FONT, 120.0, Variant::Full),
        );
        let mut cache = Cache::default();
        let (ra, rb) = (
            Arc::new(vec!["a".to_owned()]),
            Arc::new(vec!["b".to_owned()]),
        );
        cache.insert_in(42, &ia, ra.clone());
        cache.insert_in(42, &ib, rb.clone());
        assert_eq!(cache.len(), 2);
        assert!(Arc::ptr_eq(&cache.get_in(42, &ia).unwrap(), &ra));
        assert!(Arc::ptr_eq(&cache.get_in(42, &ib).unwrap(), &rb));
        let other = Inputs::new(&feed.entries[2], &ctx(), FONT, 120.0, Variant::Full);
        assert!(
            cache.get_in(42, &other).is_none(),
            "equal hash is not equal inputs"
        );
    }

    #[test]
    fn full_and_cut_rows_are_separate_unambiguous_entries() {
        let feed = feed();
        let e = &feed.entries[0];
        let full_inputs = Inputs::new(e, &ctx(), FONT, 120.0, Variant::Full);
        let (mut cache, full) = (Cache::default(), Arc::new(build(e, &ctx(), 120.0, narrow)));
        assert!(full.len() > 1);
        cache.insert(&full_inputs, full.clone());
        let cut_inputs = full_inputs.variant(Variant::Cut(1));
        assert!(cache.get(&cut_inputs).is_none());
        let cut_rows = Arc::new(cut(&full, 1, 120.0, narrow));
        cache.insert(&cut_inputs, cut_rows.clone());
        assert!(Arc::ptr_eq(&cache.get(&cut_inputs).unwrap(), &cut_rows));
        assert!(Arc::ptr_eq(&cache.get(&full_inputs).unwrap(), &full));
        assert!(cache.get(&full_inputs.variant(Variant::Cut(2))).is_none());
    }

    #[test]
    fn feed_translation_and_toggles_reach_the_painter_inputs() {
        let mut feed = feed();
        let id = feed.entries[0].id;
        let (mut cache, builds) = (Cache::default(), Cell::new(0));
        rows(
            &mut cache,
            &feed.snapshot(Language::Russian)[0],
            &ctx(),
            FONT,
            120.0,
            &narrow,
            &builds,
        );
        feed.translate(id, "привет всем".into(), Language::Russian);
        let ru = feed.snapshot(Language::Russian);
        let rebuilt = rows(&mut cache, &ru[0], &ctx(), FONT, 120.0, &narrow, &builds);
        assert_eq!(builds.get(), 2);
        assert!(rebuilt[0].contains("привет всем"));
        // The English target hides that translation: back to the untranslated rows (a hit).
        let en = feed.snapshot(Language::English);
        rows(&mut cache, &en[0], &ctx(), FONT, 120.0, &narrow, &builds);
        assert_eq!(builds.get(), 2);
        feed.toggle_original(id);
        let toggled = feed.snapshot(Language::Russian);
        let shown = rows(
            &mut cache,
            &toggled[0],
            &ctx(),
            FONT,
            120.0,
            &narrow,
            &builds,
        );
        assert_eq!(builds.get(), 3);
        assert!(shown[0].contains("[orig.]"));
    }

    #[test]
    fn the_cache_is_bounded_by_count_and_size() {
        let feed = feed();
        let e = &feed.entries[0];
        let mut cache = Cache::default();
        for width in 0..2_000 {
            let inputs = Inputs::new(e, &ctx(), FONT, 100.0 + width as f32, Variant::Full);
            cache.insert(&inputs, Arc::new(vec!["row".repeat(50)]));
            assert!(cache.len() <= MAX_ENTRIES);
        }
        let big = Inputs::new(e, &ctx(), FONT, 1.0, Variant::Full);
        cache.insert(&big, Arc::new(vec!["x".repeat(MAX_ENTRY_CHARS + 1)]));
        assert!(cache.get(&big).is_none(), "oversized rows are not retained");
    }

    fn at(e: &Entry, i: u32) -> Inputs<'_> {
        // Neighbouring widths differ by one ulp: exact bits, never rounded.
        let width = f32::from_bits(120.0f32.to_bits() + i);
        Inputs::new(e, &ctx(), FONT, width, Variant::Full)
    }
    fn small(tag: &str) -> Arc<Vec<String>> {
        Arc::new(vec![tag.to_owned()])
    }

    #[test]
    fn hot_rows_survive_count_pressure_from_a_resize_sweep() {
        let feed = feed();
        let (hot, sweeping) = (&feed.entries[1], &feed.entries[0]);
        let mut cache = Cache::default();
        let hot_rows: Vec<_> = (0..16)
            .map(|i| {
                let rows = small("hot");
                cache.insert(&at(hot, i), rows.clone());
                rows
            })
            .collect();
        for i in 0..3 * MAX_ENTRIES as u32 {
            cache.insert(&at(sweeping, i), small("sweep"));
            for (n, rows) in hot_rows.iter().enumerate() {
                let found = cache.get(&at(hot, n as u32));
                assert!(
                    found.is_some_and(|f| Arc::ptr_eq(&f, rows)),
                    "hot {n} @ {i}"
                );
            }
            cache.audit();
        }
        assert_eq!(cache.len(), MAX_ENTRIES, "only cold rows are evicted");
        // The newest sweep widths are kept, the oldest are gone; widths stay exact.
        assert!(
            cache
                .get(&at(sweeping, 3 * MAX_ENTRIES as u32 - 1))
                .is_some()
        );
        assert!(cache.get(&at(sweeping, 0)).is_none());
    }

    #[test]
    fn hot_rows_survive_byte_pressure_and_budgets_stay_exact() {
        let feed = feed();
        let (hot, sweeping) = (&feed.entries[1], &feed.entries[0]);
        let mut cache = Cache::default();
        let kept = Arc::new(vec!["h".repeat(3000)]);
        cache.insert(&at(hot, 0), kept.clone());
        for i in 0..200 {
            // Sizes vary so replacements of the budget are never aligned.
            cache.insert(
                &at(sweeping, i),
                Arc::new(vec!["x".repeat(2000 + i as usize * 7)]),
            );
            let found = cache.get(&at(hot, 0));
            assert!(found.is_some_and(|f| Arc::ptr_eq(&f, &kept)), "hot @ {i}");
            cache.audit();
        }
        assert!(cache.len() > 10, "eviction is individual, not a flush");
        assert!(cache.get(&at(sweeping, 199)).is_some());
        assert!(cache.get(&at(sweeping, 0)).is_none());
    }

    #[test]
    fn evicting_one_colliding_entry_keeps_the_other_and_the_namespaces() {
        let feed = feed();
        let (a, b) = (&feed.entries[0], &feed.entries[1]);
        let (ia, ib) = (at(a, 0), at(b, 0));
        let mut cache = Cache::default();
        let (rb, cut) = (small("b"), small("cut"));
        cache.insert_in(42, &ia, small("a"));
        cache.insert_in(42, &ib, rb.clone());
        cache.insert_in(43, &ia.variant(Variant::Cut(1)), cut.clone());
        for i in 1..MAX_ENTRIES as u32 {
            // `b` and the cut rows stay hot while `a` (the oldest) ages out.
            cache.insert(&at(&feed.entries[2], i), small("fill"));
            assert!(cache.get_in(42, &ib).is_some());
            assert!(cache.get_in(43, &ia.variant(Variant::Cut(1))).is_some());
        }
        assert!(
            cache.get_in(42, &ia).is_none(),
            "the cold collider is evicted"
        );
        assert!(Arc::ptr_eq(&cache.get_in(42, &ib).unwrap(), &rb));
        assert!(Arc::ptr_eq(
            &cache.get_in(43, &ia.variant(Variant::Cut(1))).unwrap(),
            &cut
        ));
        assert!(
            cache.get_in(43, &ia).is_none(),
            "full is not the cut namespace"
        );
        cache.audit();
    }

    #[test]
    fn oversized_rows_never_flush_valid_entries() {
        let feed = feed();
        let e = &feed.entries[0];
        let mut cache = Cache::default();
        for i in 0..MAX_ENTRIES as u32 {
            cache.insert(&at(e, i), Arc::new(vec!["row".repeat(40)]));
        }
        cache.insert(
            &at(e, 10_000),
            Arc::new(vec!["x".repeat(MAX_ENTRY_CHARS + 1)]),
        );
        assert_eq!(cache.len(), MAX_ENTRIES);
        assert!(cache.get(&at(e, 0)).is_some());
        assert!(cache.get(&at(e, 10_000)).is_none());
        cache.audit();
    }

    #[test]
    fn content_is_hashed_once_per_card_query_across_full_and_cut() {
        let feed = feed();
        let e = &feed.entries[0];
        let mut cache = Cache::default();
        content_hashes::take();
        // Cold: get + insert of Full, then get + insert of Cut.
        let full = Inputs::new(e, &ctx(), FONT, 120.0, Variant::Full);
        assert!(cache.get(&full).is_none());
        cache.insert(&full, small("full"));
        let cut = full.variant(Variant::Cut(1));
        assert!(cache.get(&cut).is_none());
        cache.insert(&cut, small("cut"));
        assert_eq!(content_hashes::take(), 1, "cold full + cut");
        // Warm: both variants are hits.
        let full = Inputs::new(e, &ctx(), FONT, 120.0, Variant::Full);
        assert!(cache.get(&full).is_some());
        assert!(cache.get(&full.variant(Variant::Cut(1))).is_some());
        assert_eq!(content_hashes::take(), 1, "warm full + cut");
        assert!(cache.get(&full.variant(Variant::Cut(2))).is_none());
        assert_ne!(
            full.hash(),
            full.variant(Variant::Cut(1)).hash(),
            "variants own different buckets"
        );
    }
}
