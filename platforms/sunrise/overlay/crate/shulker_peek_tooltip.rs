//! The shulker box preview on the typed API: a shulker box that holds something shows its 27
//! slots in its item tooltip, wherever the game shows one, in place of the game's list of the
//! contents, and the tooltip takes the colour of the box's dye.
use crate::shulker_peek::{SIZE, SLOT, colors, is_shulker, slot_origin};
use mod_api::{
    ClientHost, ClientTooltipComponent, DrawContext, ItemStack, JavaString, Tag, Text,
    TooltipComponent, TooltipComponentData, TooltipElement, TooltipFont,
};

/// The mod's `sunrise-mod.toml` id.
const ID: &str = "draimhorizon";
/// The menu's switch, one file per key where the game-side menu keeps it (`native_set_setting`
/// in `overlay/sunrise_platform.rs`, under the id the settings had before `draimhorizon`).
const SETTING: &str = "sunrise-mods/.sunrise-mod-data/claim_borders/shulker_peek";
/// The gap between the text and the grid, in GUI pixels.
const GAP: f32 = 2.0;
/// `ItemContainerContents.addToTooltip` lists this many stacks, then how many more there are.
const LISTED: usize = 5;

/// What a previewed box holds, by slot.
pub struct Contents(pub Vec<(u8, ItemStack)>);
impl TooltipComponentData for Contents {}

/// The stacks of a shulker box that holds something, with the mod on and the preview not
/// switched off in its menu; `None` for any other stack.
fn previewed(host: &dyn ClientHost, stack: &ItemStack) -> Option<Vec<(u8, ItemStack)>> {
    if !stack.item().id(host).is_some_and(is_shulker) || !host.on(ID) {
        return None;
    }
    let setting = std::fs::read_to_string(host.instance_dir().join(SETTING));
    if setting.is_ok_and(|value| value.trim() == "false") {
        return None;
    }
    // `minecraft:container`: a list of `{slot, item}`.
    let Tag::List(slots) = stack
        .components()?
        .as_nbt()
        .get(&JavaString::from("minecraft:container"))?
    else {
        return None;
    };
    let held: Vec<_> = slots
        .iter()
        .filter_map(|entry| {
            let Tag::Compound(entry) = entry else {
                return None;
            };
            let (Some(Tag::Int(slot)), Some(Tag::Compound(item))) = (
                entry.get(&JavaString::from("slot")),
                entry.get(&JavaString::from("item")),
            ) else {
                return None;
            };
            Some((u8::try_from(*slot).ok()?, ItemStack::from_nbt(host, item)?))
        })
        .collect();
    (!held.is_empty()).then_some(held)
}

/// Takes the game's list of what the box holds ("6 x Iron Ingot ... and 20 more...") out of the
/// tooltip, as the grid shows it, and adds the grid under the rest.
pub fn gather(host: &dyn ClientHost, stack: &ItemStack, elements: &mut Vec<TooltipElement>) {
    let Some(mut held) = previewed(host, stack) else {
        return;
    };
    held.sort_by_key(|(slot, _)| *slot);
    let mut listed: Vec<String> = held
        .iter()
        .take(LISTED)
        .map(|(_, s)| {
            let name = host.hover_name(s).unwrap_or_else(|_| Text::literal(""));
            let count = Text::literal(s.count().to_string());
            host.text_string(&Text::translate(
                "item.container.item_count",
                vec![count, name],
            ))
        })
        .collect();
    if held.len() > LISTED {
        let more = Text::literal((held.len() - LISTED).to_string());
        listed.push(host.text_string(&Text::translate("item.container.more_items", vec![more])));
    }
    elements.retain(|element| {
        let TooltipElement::Text(text, _) = element else {
            return true;
        };
        let plain = host.text_string(text);
        match listed.iter().position(|line| *line == plain) {
            Some(at) => {
                listed.remove(at);
                false
            }
            None => true,
        }
    });
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
    let Some(stack) = stack.filter(|stack| previewed(host, stack).is_some()) else {
        return;
    };
    if let Some(item) = stack.item().id(host) {
        (*background, *border_top, *border_bottom) = colors(item);
    }
}

/// The 9x3 grid of a previewed box, drawn under the tooltip's text.
pub struct Grid(pub Vec<(u8, ItemStack)>);

impl ClientTooltipComponent for Grid {
    fn height(&self, _: &dyn TooltipFont) -> i32 {
        (SIZE[1] + GAP) as i32
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
        let origin = [x as f32, y as f32 + GAP];
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
