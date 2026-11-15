// The block picker of the Baritone tab's Mining section (MIT): every block of the game can be
// chosen, our preset targets (ores, stone, sand, wood...) come first as one-click sets, and the
// choice becomes the arguments of the port's `mine` command. Pure, so it is tested on its own.
use super::claim_borders_mining as mining;
use claim_borders_relay_client::translation::Language;
use std::collections::BTreeSet;

/// Tiles in a row of the picker, and rows to a page.
pub const COLUMNS: usize = 24;
pub const ROWS: usize = 4;
/// Entries per page of the list.
pub const PER_PAGE: usize = COLUMNS * ROWS;
/// The most blocks one `mine` command carries.
pub const MAX_SELECTED: usize = 64;
/// How many chosen blocks are kept in the settings.
const MAX_SAVED: usize = 256;

/// One row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A preset target of the mining job (`mining::target`), with its index.
    Preset(u8),
    /// One block, by id (`minecraft:stone`).
    Block(String),
}

/// Blocks that are not worth listing: not solid, not breakable, or not placed by anyone.
fn unlisted(name: &str) -> bool {
    let bare = name.strip_prefix("minecraft:").unwrap_or(name);
    matches!(
        bare,
        "air"
            | "cave_air"
            | "void_air"
            | "water"
            | "lava"
            | "bubble_column"
            | "bedrock"
            | "barrier"
            | "light"
            | "structure_void"
            | "structure_block"
            | "jigsaw"
            | "command_block"
            | "chain_command_block"
            | "repeating_command_block"
            | "moving_piston"
            | "piston_head"
            | "end_portal"
            | "end_portal_frame"
            | "end_gateway"
            | "nether_portal"
            | "reinforced_deepslate"
            | "test_block"
            | "test_instance_block"
    )
}

/// The ids to list, from every block state name of the game: namespaced, sorted, without
/// duplicates and without the blocks nobody mines.
pub fn catalogue<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut set = BTreeSet::new();
    for name in names {
        let id = if name.contains(':') {
            name.to_owned()
        } else {
            format!("minecraft:{name}")
        };
        if !unlisted(&id) && id.len() <= 80 {
            set.insert(id);
        }
    }
    set.into_iter().collect()
}

/// "minecraft:deepslate_diamond_ore" as "Deepslate Diamond Ore".
pub fn humanize(id: &str) -> String {
    let bare = id.split_once(':').map_or(id, |(_, rest)| rest);
    bare.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            chars
                .next()
                .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn presets() -> impl Iterator<Item = u8> {
    // The last target is "own block", which has no blocks of its own.
    0..mining::CUSTOM
}

/// The rows to show for the typed filter: with none, the presets and then every block; with a
/// filter, the presets whose name and the blocks whose id or name contain it, case-insensitive.
pub fn entries(blocks: &[String], language: Language, query: &str) -> Vec<Entry> {
    let q = query.trim().to_lowercase().replace(' ', "_");
    let mut out = Vec::new();
    for i in presets() {
        let Some((ru, en, list)) = mining::target(i) else {
            continue;
        };
        if list.is_empty() {
            continue;
        }
        let label = language.text(ru, en).to_lowercase().replace(' ', "_");
        if q.is_empty() || label.contains(&q) {
            out.push(Entry::Preset(i));
        }
    }
    for id in blocks {
        if q.is_empty() || id.to_lowercase().contains(&q) {
            out.push(Entry::Block(id.clone()));
        }
    }
    out
}

/// The text of a row.
pub fn label(entry: &Entry, language: Language) -> String {
    match entry {
        Entry::Preset(i) => mining::target(*i).map_or_else(String::new, |(ru, en, list)| {
            format!(
                "{}: {} ({})",
                language.text("Набор", "Set"),
                language.text(ru, en),
                list.len()
            )
        }),
        Entry::Block(id) => humanize(id),
    }
}

/// Whether a row is chosen: a block is in the list, a preset when all its blocks are.
pub fn chosen(entry: &Entry, selected: &[String]) -> bool {
    match entry {
        Entry::Block(id) => selected.contains(id),
        Entry::Preset(i) => mining::target(*i).is_some_and(|(_, _, list)| {
            !list.is_empty() && list.iter().all(|b| selected.iter().any(|s| s == b))
        }),
    }
}

/// The list after a click on a row: a block is toggled; a preset is taken out when all its
/// blocks are chosen and put in (what is missing of it) otherwise. Never longer than
/// `MAX_SELECTED`.
pub fn toggled(entry: &Entry, selected: &[String]) -> Vec<String> {
    let mut list: Vec<String> = selected.to_vec();
    match entry {
        Entry::Block(id) => {
            if let Some(at) = list.iter().position(|s| s == id) {
                list.remove(at);
            } else {
                list.push(id.clone());
            }
        }
        Entry::Preset(i) => {
            let blocks: Vec<&str> =
                mining::target(*i).map_or_else(Vec::new, |(_, _, b)| b.to_vec());
            if chosen(entry, selected) {
                list.retain(|s| !blocks.contains(&s.as_str()));
            } else {
                for b in blocks {
                    if !list.iter().any(|s| s == b) {
                        list.push(b.to_owned());
                    }
                }
            }
        }
    }
    list.truncate(MAX_SELECTED);
    list
}

/// The pages of `count` rows, and the page kept inside them.
pub fn pages(count: usize) -> usize {
    count.div_ceil(PER_PAGE).max(1)
}
pub fn clamp_page(page: usize, count: usize) -> usize {
    page.min(pages(count) - 1)
}

/// The port's command for the chosen blocks (without the prefix): `mine a b c`.
pub fn mine_line(selected: &[String]) -> Option<String> {
    if selected.is_empty() {
        return None;
    }
    let names: Vec<&str> = selected
        .iter()
        .take(MAX_SELECTED)
        .map(|s| s.strip_prefix("minecraft:").unwrap_or(s))
        .collect();
    Some(format!("mine {}", names.join(" ")))
}

/// The chosen blocks as kept in the settings: one id a line.
pub fn encode(selected: &[String]) -> String {
    selected
        .iter()
        .take(MAX_SAVED)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n")
}
/// What `encode` kept; a line that is not a block id is left out.
pub fn decode(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let id = line.trim();
        let valid = !id.is_empty()
            && id.len() <= 80
            && !id.contains("..")
            && !id.starts_with(['.', '/'])
            && id.chars().all(|c| {
                c.is_ascii_lowercase()
                    || c.is_ascii_digit()
                    || matches!(c, '_' | ':' | '/' | '.' | '-')
            });
        if valid && !out.iter().any(|o| o == id) {
            out.push(id.to_owned());
        }
        if out.len() >= MAX_SELECTED {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocks() -> Vec<String> {
        catalogue([
            "minecraft:stone",
            "minecraft:stone",
            "minecraft:deepslate_diamond_ore",
            "minecraft:diamond_ore",
            "minecraft:air",
            "minecraft:water",
            "minecraft:bedrock",
            "oak_log",
            "modded:ruby_ore",
        ])
    }

    #[test]
    fn the_catalogue_is_sorted_unique_namespaced_and_without_unminable_blocks() {
        let list = blocks();
        assert_eq!(
            list,
            [
                "minecraft:deepslate_diamond_ore",
                "minecraft:diamond_ore",
                "minecraft:oak_log",
                "minecraft:stone",
                "modded:ruby_ore"
            ]
        );
        assert_eq!(
            humanize("minecraft:deepslate_diamond_ore"),
            "Deepslate Diamond Ore"
        );
        assert_eq!(humanize("modded:ruby_ore"), "Ruby Ore");
    }

    #[test]
    fn rows_are_presets_then_blocks_and_a_filter_narrows_both() {
        let all = entries(&blocks(), Language::English, "");
        assert!(matches!(all[0], Entry::Preset(0)));
        assert_eq!(
            all.iter().filter(|e| matches!(e, Entry::Block(_))).count(),
            5
        );
        // The own-block target has no blocks of its own and is not a row.
        assert!(!all.contains(&Entry::Preset(mining::CUSTOM)));
        let found = entries(&blocks(), Language::English, "Diamond Ore");
        assert!(found.contains(&Entry::Block("minecraft:diamond_ore".into())));
        assert!(found.contains(&Entry::Block("minecraft:deepslate_diamond_ore".into())));
        assert!(!found.contains(&Entry::Block("minecraft:stone".into())));
        // A filter in Russian finds the Russian preset name.
        assert!(
            entries(&blocks(), Language::Russian, "камень")
                .iter()
                .any(|e| matches!(e, Entry::Preset(_)))
        );
        assert!(entries(&blocks(), Language::English, "no such block").is_empty());
    }

    #[test]
    fn a_block_toggles_and_a_preset_takes_all_its_blocks_in_or_out() {
        let diamond = Entry::Block("minecraft:diamond_ore".into());
        let one = toggled(&diamond, &[]);
        assert_eq!(one, ["minecraft:diamond_ore"]);
        assert!(chosen(&diamond, &one));
        assert!(toggled(&diamond, &one).is_empty());
        // A preset: every block of it, once, and again takes them out.
        let stone = entries(&[], Language::English, "stone")
            .into_iter()
            .find(|e| matches!(e, Entry::Preset(i) if mining::target(*i).is_some_and(|(_, en, _)| en == "Stone")))
            .expect("the Stone preset");
        let with = toggled(&stone, &["minecraft:diamond_ore".to_owned()]);
        assert!(chosen(&stone, &with));
        assert!(with.contains(&"minecraft:diamond_ore".to_owned()));
        assert!(with.contains(&"minecraft:deepslate".to_owned()));
        let again = toggled(&stone, &with);
        assert_eq!(again, ["minecraft:diamond_ore"]);
        // A half chosen preset is completed, not cleared.
        let half = toggled(&stone, &["minecraft:stone".to_owned()]);
        assert!(chosen(&stone, &half));
    }

    #[test]
    fn the_selection_is_bounded_paged_and_becomes_a_mine_command() {
        let many: Vec<String> = (0..100).map(|i| format!("minecraft:b{i}")).collect();
        let extra = Entry::Block("minecraft:extra".into());
        assert_eq!(toggled(&extra, &many).len(), MAX_SELECTED);
        assert_eq!(mine_line(&[]), None);
        assert_eq!(
            mine_line(&["minecraft:stone".into(), "modded:ruby_ore".into()]).as_deref(),
            Some("mine stone modded:ruby_ore")
        );
        assert_eq!(
            mine_line(&many).unwrap().split(' ').count(),
            MAX_SELECTED + 1
        );
        assert_eq!((pages(0), pages(PER_PAGE), pages(PER_PAGE + 1)), (1, 1, 2));
        assert_eq!((clamp_page(9, PER_PAGE + 1), clamp_page(1, 0)), (1, 0));
    }

    #[test]
    fn the_saved_choice_round_trips_and_drops_junk() {
        let list = vec!["minecraft:stone".to_owned(), "modded:ruby_ore".to_owned()];
        assert_eq!(decode(&encode(&list)), list);
        let text = "minecraft:stone\n\nBAD ID\nminecraft:stone\n../x\n  minecraft:dirt  \n";
        assert_eq!(decode(text), ["minecraft:stone", "minecraft:dirt"]);
        let long: String = (0..400).map(|i| format!("minecraft:b{i}\n")).collect();
        assert_eq!(decode(&long).len(), MAX_SELECTED);
    }
}
