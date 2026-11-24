//! The shulker box preview on the typed API: a shulker box that holds something shows its 27
//! slots in its item tooltip, wherever the game shows one, in place of the game's list of the
//! contents, and the tooltip takes the colour of the box's dye.
use crate::shulker_peek::{SIZE, SLOT, colors, is_shulker, slot_origin};
use mod_api::{
    ClientHost, ClientTooltipComponent, Compound, DrawContext, ItemStack, JavaString, Tag, Text,
    TooltipComponent, TooltipComponentData, TooltipDisplay, TooltipElement, TooltipFont,
};

/// The mod's `sunrise-mod.toml` id.
const ID: &str = "draimhorizon";
/// The menu's switch, one file per key where the game-side menu keeps it (`native_set_setting`
/// in `overlay/sunrise_platform.rs`, under the id the settings had before `draimhorizon`).
const SETTING: &str = "sunrise-mods/.sunrise-mod-data/claim_borders/shulker_peek";
/// `ItemContainerContents.addToTooltip` lists this many stacks, then how many more there are.
const LISTED: usize = 5;

/// What a previewed box holds, by slot.
pub struct Contents(pub Vec<(u8, ItemStack)>);
impl TooltipComponentData for Contents {}

/// The `{slot, item}` entries of a shulker box that holds something, when the mod is on, the
/// preview is not switched off in its menu, the box does not hide its contents
/// (`minecraft:tooltip_display`, as `ItemStack.addToTooltip` honours it) and
/// every slot fits the grid; `None` otherwise, which leaves the game's tooltip as it is.
fn entries<'a>(host: &dyn ClientHost, stack: &'a ItemStack) -> Option<Vec<(u8, &'a Compound)>> {
    if !stack.item().id(host).is_some_and(is_shulker) || !host.on(ID) {
        return None;
    }
    let setting = std::fs::read_to_string(host.instance_dir().join(SETTING));
    if setting.is_ok_and(|value| value.trim() == "false") {
        return None;
    }
    if !TooltipDisplay::for_stack(stack).shows("minecraft:container") {
        return None;
    }
    let components = stack.components()?.as_nbt();
    let container = JavaString::from("minecraft:container");
    // `minecraft:container`: a list of `{slot, item}`.
    let Tag::List(slots) = components.get(&container)? else {
        return None;
    };
    let entries = slots
        .iter()
        .map(|entry| {
            let Tag::Compound(entry) = entry else {
                return None;
            };
            let (Some(Tag::Int(slot)), Some(Tag::Compound(item))) = (
                entry.get(&JavaString::from("slot")),
                entry.get(&JavaString::from("item")),
            ) else {
                return None;
            };
            let slot = u8::try_from(*slot)
                .ok()
                .filter(|slot| slot_origin(*slot).is_some())?;
            Some((slot, item))
        })
        .collect::<Option<Vec<_>>>()?;
    (!entries.is_empty()).then_some(entries)
}

/// Without `§` and the formatting code after it: the game reads its list through
/// `Text::legacy`, which turns them into style.
fn without_codes(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\u{a7}' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

/// Takes the game's list of what the box holds ("6 x Iron Ingot ... and 20 more...") out of the
/// tooltip, as the grid shows it, and adds the grid under the rest.
pub fn gather(host: &dyn ClientHost, stack: &ItemStack, elements: &mut Vec<TooltipElement>) {
    let Some(entries) = entries(host, stack) else {
        return;
    };
    let Some(mut held) = entries
        .into_iter()
        .map(|(slot, item)| Some((slot, ItemStack::from_nbt(host, item)?)))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    held.sort_by_key(|(slot, _)| *slot);
    // `item.container.item_count` and `item.container.more_items`, with the templates the game
    // falls back to when the language lacks them (`item_tooltip::container_lines`). `Text`
    // builds a translation only without a fallback, so the fallback goes in through its NBT form.
    let line = |key: &str, fallback: &str, arguments| {
        let mut text = Text::translate(key, arguments);
        if let Ok(Tag::Compound(mut nbt)) = host.text_to_nbt(&text) {
            nbt.insert("fallback".into(), Tag::String(fallback.into()));
            if let Ok(Some(with)) = host.text_from_nbt(&Tag::Compound(nbt)) {
                text = with;
            }
        }
        without_codes(&host.text_string(&text))
    };
    let mut listed: Vec<String> = held
        .iter()
        .take(LISTED)
        .map(|(_, s)| {
            let name = host.hover_name(s).unwrap_or_else(|_| Text::literal(""));
            let count = Text::literal(s.count().to_string());
            line("item.container.item_count", "%s x %s", vec![count, name])
        })
        .collect();
    if held.len() > LISTED {
        let more = Text::literal((held.len() - LISTED).to_string());
        listed.push(line(
            "item.container.more_items",
            "and %s more...",
            vec![more],
        ));
    }
    // The list is one run of text lines after the name (line 0); the first run that matches it
    // goes, so a name or lore line that reads the same stays.
    let matches = |at: usize| {
        listed.iter().enumerate().all(|(i, line)| {
            matches!(elements.get(at + i), Some(TooltipElement::Text(text, _)) if host.text_string(text) == *line)
        })
    };
    if let Some(at) = (1..elements.len()).find(|at| matches(*at)) {
        elements.drain(at..at + listed.len());
    }
    elements.push(TooltipElement::Component(TooltipComponent::custom(
        Contents(held),
    )));
}

/// Tints the tooltip box with the colour of a previewed box's dye.
pub fn tint(
    host: &dyn ClientHost,
    stack: Option<&ItemStack>,
    background: &mut u32,
    border_top: &mut u32,
    border_bottom: &mut u32,
) {
    let Some(stack) = stack.filter(|stack| entries(host, stack).is_some()) else {
        return;
    };
    if let Some(item) = stack.item().id(host) {
        (*background, *border_top, *border_bottom) = colors(item);
    }
}

/// The 9x3 grid of a previewed box, drawn under the tooltip's text.
pub struct Grid(pub Vec<(u8, ItemStack)>);

impl ClientTooltipComponent for Grid {
    // The grid and 2 px under it, as a text line's 10 px hold 8 px of glyphs
    // (`ClientTextTooltip.getHeight`), so the box's last row keeps its margin.
    fn height(&self, _: &dyn TooltipFont) -> i32 {
        SIZE[1] as i32 + 2
    }

    fn width(&self, _: &dyn TooltipFont) -> i32 {
        SIZE[0] as i32
    }

    fn draw_image(
        &self,
        _: &dyn TooltipFont,
        x: i32,
        y: i32,
        _: i32,
        _: i32,
        draw: &mut dyn DrawContext,
    ) {
        let origin = [x as f32, y as f32];
        for slot in 0..27u8 {
            let Some([x, y]) = slot_origin(slot) else {
                continue;
            };
            let (x, y) = (origin[0] + x, origin[1] + y);
            // A darker well with a faint rim, like an inventory slot.
            draw.fill([x, y, SLOT - 1.0, SLOT - 1.0], 0x40ffffff);
            draw.fill([x + 1.0, y + 1.0, SLOT - 3.0, SLOT - 3.0], 0xc0101018);
        }
        for (slot, held) in &self.0 {
            let Some([x, y]) = slot_origin(*slot) else {
                continue;
            };
            let at = [origin[0] + x + 1.0, origin[1] + y + 1.0];
            draw.item_stack(held, at, 16.0);
            // `GuiGraphics.renderItemDecorations`: the count, right-aligned, with a shadow.
            if held.count() != 1 {
                let count = held.count().to_string();
                let left = at[0] + 19.0 - 2.0 - draw.text_width(&count);
                draw.text(&count, [left, at[1] + 6.0 + 3.0], 0xffffffff, true);
            }
        }
    }
}
