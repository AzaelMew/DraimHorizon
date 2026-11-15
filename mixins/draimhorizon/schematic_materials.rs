// The material list of a schematic preview (MIT glue over `litematica/materials.rs`, LGPL): what
// the file needs, what is still missing of it in the world, what is standing there as another
// block, and what the player holds, sorted like Litematica's list. Copied to the clipboard.
use crate::game::{
    Game, claim_borders_litematica_materials as mat, claim_borders_schematic_ghosts as ghosts,
    claim_borders_schematic_places as places, claim_borders_schematic_view as view,
};
use std::collections::HashMap;

fn text(ru: &'static str, en: &'static str) -> &'static str {
    crate::game::claim_borders_chat_view::current_language().text(ru, en)
}
/// The items of the main inventory and the hotbar, as the list counts them.
fn held(game: &Game<'_>) -> HashMap<String, usize> {
    let stacks: Vec<mat::Stack> = (0..36)
        .filter_map(|g| {
            let stack = if g < 9 {
                game.stack_in(g)
            } else {
                game.inventory.get(g).and_then(Option::as_ref)
            };
            stack
                .filter(|s| s.count > 0)
                .map(|s| mat::Stack::new(&s.item, s.count as usize))
        })
        .collect();
    mat::inventory_counts(&stacks)
}
/// The list for the selected preview, as text.
pub(super) fn report(game: &Game<'_>) -> Result<String, String> {
    let pl = view::active(game).ok_or_else(|| {
        text(
            "Сначала поставьте или выберите предпоказ",
            "Place or select a preview first",
        )
        .to_owned()
    })?;
    let shape = view::shape(game, &pl.file)
        .ok_or_else(|| text("Файл ещё читается", "The file is still being read").to_owned())?;
    // How many cells of each state of the file, and of those how many are not as wanted.
    let mut total: HashMap<usize, usize> = HashMap::new();
    let mut missing: HashMap<usize, usize> = HashMap::new();
    let mut wrong: HashMap<usize, usize> = HashMap::new();
    let facts = game.res.facts();
    let ids: Vec<_> = shape
        .states
        .iter()
        .map(|s| ghosts::resolve(game, &s.transformed(pl.rotation)))
        .collect();
    for (rel, index) in &shape.cells {
        let i = *index as usize;
        *total.entry(i).or_default() += 1;
        let at = places::place(*rel, pl.origin, pl.rotation);
        if !game.world.chunks.contains_key(&(at[0] >> 4, at[2] >> 4)) {
            *missing.entry(i).or_default() += 1;
            continue;
        }
        let live = game.world.block(at[0], at[1], at[2]);
        let right = ids.get(i).copied().flatten().is_some_and(|id| live == id);
        if right {
            continue;
        }
        *missing.entry(i).or_default() += 1;
        if !matches!(
            &*facts.get(live).name,
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        ) {
            *wrong.entry(i).or_default() += 1;
        }
    }
    let items = |counts: &HashMap<usize, usize>| {
        mat::states_to_items(
            counts,
            |i| shape.states[*i].waterlogged(),
            |i| shape.states[*i].items(),
        )
    };
    let mut list = mat::entries(
        &items(&total),
        &items(&missing),
        &items(&wrong),
        &held(game),
    );
    mat::sort(&mut list, mat::Sort::CountMissing, false, str::to_owned);
    let short = |s: &str| s.trim_start_matches("minecraft:").to_owned();
    let mut out = format!(
        "{} · {}\n{}\n",
        pl.file,
        text(
            "нужно / не хватает в мире / не тот блок / есть у вас",
            "need / missing in the world / wrong block / you have"
        ),
        "-".repeat(40)
    );
    for e in &list {
        out.push_str(&format!(
            "{}: {} / {} / {} / {}{}\n",
            short(&e.item),
            e.total,
            e.missing,
            e.mismatched,
            e.available,
            if e.shortfall() > 0 {
                format!("  ({} {})", text("докупить", "get"), e.shortfall())
            } else {
                String::new()
            }
        ));
    }
    Ok(out)
}
