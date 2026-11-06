// Attribute quiet replies only to our recent read-only requests. Manual commands reset it.
use super::claim_borders_economy as model;
use std::collections::VecDeque;
#[derive(Default)]
pub struct Replies {
    pending: VecDeque<(String, u64, bool)>,
}
impl Replies {
    pub fn begin(&mut self, command: &str, now: u64, quiet: bool) {
        self.pending
            .retain(|(_, at, _)| now.saturating_sub(*at) < 10000);
        if self.pending.len() >= 8 {
            self.pending.pop_front();
        }
        self.pending.push_back((command.into(), now, quiet));
    }
    pub fn manual(&mut self, command: &str) {
        if matches!(
            command.split_whitespace().next(),
            Some("worth" | "bal" | "balance")
        ) {
            self.pending.clear();
        }
    }
    pub fn quiet(&mut self, text: &str, now: u64) -> bool {
        self.pending
            .retain(|(_, at, _)| now.saturating_sub(*at) < 10000);
        let item = model::quote_item(text).or_else(|| model::unsupported_item(text));
        self.pending.iter().rev().any(|(command, _, quiet)| {
            if !quiet {
                return false;
            }
            if command == "worth" {
                return text.starts_with("Selling today: ");
            }
            if let Some(expected) = command.strip_prefix("worth ") {
                if let Some(item) = &item {
                    return model::id(expected).as_ref() == Some(item);
                }
                return text.starts_with("Selling today: ")
                    || text.starts_with("You may sell ")
                        && text
                            .strip_suffix(" today.")
                            .and_then(|s| s.split_once(" more "))
                            .and_then(|(_, s)| model::id(s))
                            == model::id(expected)
                    || text.starts_with("Moves: ")
                        && text
                            .rsplit_once("Chart: /market ")
                            .and_then(|(_, s)| model::id(s))
                            == model::id(expected);
            }
            command == "bal"
                && (text.starts_with("Balance: ") || text.starts_with("Your balance: "))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_quotes_and_rejections_never_hide_player_actions_or_manual_queries() {
        let mut r = Replies::default();
        r.begin("worth", 99, true);
        assert!(r.quiet("Selling today: 50 coins left of your 100 coins", 100));
        assert!(!r.quiet("Balance: 50 coins", 100));
        r.begin("worth leather", 100, true);
        assert!(r.quiet("leather: 3.01 coins each, to buy or sell.", 101));
        assert!(!r.quiet("Sold 1 leather for 3 coins.", 101));
        assert!(!r.quiet("diamond: 100 coins each, to buy or sell.", 101));
        r.begin("worth brown_wool", 102, true);
        assert!(r.quiet("The market doesn't trade brown wool.", 103));
        r.manual("worth leather");
        assert!(!r.quiet("leather: 3.01 coins each, to buy or sell.", 104));
        r.begin("worth gravel", 105, true);
        assert!(r.quiet(
            "gravel: 2 coins each, to buy (the market doesn't buy it).",
            106
        ));
        assert!(!r.quiet(
            "gravel: 2 coins each, to buy (the market doesn't buy it).",
            10106
        ));
    }
}

pub fn next_watch<'a>(
    watch: &'a std::collections::BTreeSet<String>,
    unsupported: &std::collections::BTreeSet<String>,
    polled: &std::collections::BTreeMap<String, u64>,
    now: u64,
    interval: u64,
) -> Option<&'a str> {
    watch
        .iter()
        .filter(|id| {
            !unsupported.contains(*id)
                && polled
                    .get(*id)
                    .is_none_or(|at| now.saturating_sub(*at) >= interval)
        })
        .min_by_key(|id| polled.get(*id).copied().unwrap_or(0))
        .map(String::as_str)
}
#[cfg(test)]
mod scheduling_tests {
    use super::*;
    #[test]
    fn all_sixty_four_goods_are_queried_before_any_is_polled_twice() {
        let watch = (0..64).map(|i| format!("minecraft:item{i:02}")).collect();
        let unsupported = Default::default();
        let mut polled = std::collections::BTreeMap::new();
        for step in 0..64 {
            let now = 100000 + step * 2000;
            let item = next_watch(&watch, &unsupported, &polled, now, 30000)
                .unwrap()
                .to_owned();
            assert!(!polled.contains_key(&item));
            polled.insert(item, now);
        }
        assert_eq!(polled.len(), 64);
    }
}
