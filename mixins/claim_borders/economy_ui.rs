// Shared geometry and ordering for a bounded, centred trading window.
use super::claim_borders_economy::{Book, Listing, Preferences};
use claim_borders_relay_client::translation::Language;
#[derive(Clone, Copy, Debug)]
pub struct Panel {
    pub rect: [f32; 4],
}
impl Panel {
    pub fn new(viewport: [f32; 2]) -> Self {
        let w = (viewport[0] - 24.0).clamp(1.0, 520.0);
        let h = (viewport[1] - 24.0).clamp(1.0, 300.0);
        Self {
            rect: [(viewport[0] - w) / 2.0, (viewport[1] - h) / 2.0, w, h],
        }
    }
    pub fn local(self, mouse: [f32; 2]) -> [f32; 2] {
        [mouse[0] - self.rect[0], mouse[1] - self.rect[1]]
    }
}
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum Sort {
    #[default]
    Unit,
    Total,
    Profit,
    Roi,
    Count,
    Name,
    Recent,
}
impl Sort {
    pub fn label(self) -> &'static str {
        self.label_in(Language::Russian)
    }
    pub fn label_in(self, language: Language) -> &'static str {
        match self {
            Self::Unit => language.text("Цена/шт. ↑", "Price/unit ↑"),
            Self::Total => language.text("Цена лота ↑", "Lot price ↑"),
            Self::Profit => language.text("Прибыль ↓", "Profit ↓"),
            Self::Roi => language.text("Доходность ↓", "ROI ↓"),
            Self::Count => language.text("Количество ↓", "Quantity ↓"),
            Self::Name => language.text("Название А–Я", "Name A–Z"),
            Self::Recent => language.text("Свежие первыми", "Newest first"),
        }
    }
    pub fn listings<'a>(
        self,
        book: &'a Book,
        p: &Preferences,
        now: u64,
        own: &str,
        profit_only: bool,
    ) -> Vec<&'a Listing> {
        let mut list: Vec<_> = book
            .listings
            .values()
            .filter(|l| {
                !profit_only
                    || (!l.bid
                        && !l.seller.eq_ignore_ascii_case(own)
                        && l.expires > now
                        && l.generation == book.generation)
            })
            .collect();
        let profits: std::collections::BTreeMap<_, _> = if matches!(self, Self::Profit | Self::Roi)
        {
            list.iter()
                .filter_map(|l| book.full_lot_profit(l, p).map(|profit| (l.id, profit)))
                .collect()
        } else {
            Default::default()
        };
        let score = |l: &Listing| profits.get(&l.id).copied();
        list.sort_by(|a, b| {
            let order = match self {
                Self::Unit => (i128::from(a.cost) * i128::from(b.count))
                    .cmp(&(i128::from(b.cost) * i128::from(a.count))),
                Self::Total => a.cost.cmp(&b.cost),
                Self::Count => b.count.cmp(&a.count),
                Self::Name => a.label.to_lowercase().cmp(&b.label.to_lowercase()),
                Self::Recent => b.seen.cmp(&a.seen),
                Self::Profit => score(b).cmp(&score(a)),
                Self::Roi => match (score(a), score(b)) {
                    (Some(ap), Some(bp)) => (i128::from(bp) * i128::from(a.cost.max(1)))
                        .cmp(&(i128::from(ap) * i128::from(b.cost.max(1)))),
                    (a, b) => b.cmp(&a),
                },
            };
            order.then(a.id.cmp(&b.id))
        });
        list
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn english_sort_labels_have_no_russian_text() {
        for sort in [
            Sort::Unit,
            Sort::Total,
            Sort::Profit,
            Sort::Roi,
            Sort::Count,
            Sort::Name,
            Sort::Recent,
        ] {
            let text = sort.label_in(Language::English);
            assert!(!text.chars().any(|c| ('\u{400}'..='\u{4ff}').contains(&c)));
            assert_eq!(sort.label(), sort.label_in(Language::Russian));
        }
    }
    #[test]
    fn centred_window_and_mouse_agree_after_resize() {
        for viewport in [[1280., 720.], [854., 480.], [320., 240.]] {
            let p = Panel::new(viewport);
            assert!(p.rect[2] < viewport[0] && p.rect[3] < viewport[1]);
            assert!(p.rect[2] <= 600. && p.rect[3] <= 350.);
            assert_eq!(p.local([p.rect[0] + 24., p.rect[1] + 56.]), [24., 56.]);
        }
    }
    #[test]
    fn unit_sort_uses_total_cost_and_profit_sort_puts_missing_quotes_last() {
        let mut b = Book::default();
        let now = 100000;
        for (id, count, cost) in [(1, 2, 1000), (2, 64, 6400), (3, 1, 100)] {
            b.listings.insert(
                id,
                Listing {
                    id,
                    item: format!("minecraft:item{id}"),
                    label: format!("Item {id}"),
                    count,
                    cost,
                    seller: "Alex".into(),
                    bid: false,
                    seen: now,
                    expires: now + 10000,
                    generation: 0,
                },
            );
        }
        let p = Preferences::default();
        assert_eq!(
            Sort::Unit
                .listings(&b, &p, now, "me", false)
                .iter()
                .map(|l| l.id)
                .collect::<Vec<_>>(),
            [2, 3, 1]
        );
        b.observe("minecraft:item1", Some(1000), Some(1000), now, "test");
        assert_eq!(Sort::Profit.listings(&b, &p, now, "me", true)[0].id, 1);
    }
}

// Place the observation card outside every slot and the server's panel.
pub fn beside(container: [f32; 4], viewport: [f32; 2], height: f32) -> Option<[f32; 4]> {
    let [x, y, w, h] = container;
    let gap = 8.0;
    let card_width = 194.0;
    let fits = |r: [f32; 4]| {
        r[0] >= 4.0
            && r[1] >= 4.0
            && r[0] + r[2] <= viewport[0] - 4.0
            && r[1] + r[3] <= viewport[1] - 4.0
    };
    [
        [x - gap - card_width, y, card_width, height],
        [x + w + gap, y, card_width, height],
        [x, y - height - gap, w, height],
        [x, y + h + gap, w, height],
    ]
    .into_iter()
    .find(|r| fits(*r))
}
#[cfg(test)]
mod card_tests {
    use super::*;
    #[test]
    fn card_is_never_over_a_container_or_outside_the_window() {
        for (viewport, container) in [
            ([1280., 720.], [552., 227., 176., 266.]),
            ([854., 480.], [339., 107., 176., 266.]),
            ([320., 240.], [72., 4., 176., 232.]),
        ] {
            if let Some([x, y, w, h]) = beside(container, viewport, 76.) {
                assert!(x >= 0. && y >= 0. && x + w <= viewport[0] && y + h <= viewport[1]);
                assert!(
                    x + w <= container[0]
                        || x >= container[0] + container[2]
                        || y + h <= container[1]
                        || y >= container[1] + container[3]
                );
            } else {
                assert!(viewport[0] < 500.);
            }
        }
    }
}
