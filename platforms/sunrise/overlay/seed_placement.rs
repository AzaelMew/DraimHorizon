// Candidate sampling only. GeneratorWorker::structure_starts remains the authority
// for frequency, exclusion, biome and terrain validation.
use std::collections::BTreeMap;
use sunrise_core::{
    position::ChunkPos,
    random::{LegacyRandom, RandomSource, WorldgenRandom},
};

pub(crate) struct Spread {
    spacing: i32,
    range: i32,
    salt: i32,
    triangular: bool,
}

pub(crate) fn load(raw: &BTreeMap<String, String>) -> Result<BTreeMap<String, Spread>, String> {
    let mut spreads = BTreeMap::new();
    for (id, text) in raw {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let placement = &value["placement"];
        if !matches!(placement["type"].as_str(), Some("minecraft:random_spread" | "random_spread")) {
            continue;
        }
        let number = |key: &str| {
            placement[key].as_i64().and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| format!("invalid {key} in structure set {id}"))
        };
        let (spacing, separation, salt) = (number("spacing")?, number("separation")?, number("salt")?);
        if spacing <= 0 || separation < 0 || separation >= spacing {
            return Err(format!("invalid random-spread spacing in structure set {id}"));
        }
        spreads.insert(id.clone(), Spread {
            spacing,
            range: spacing - separation,
            salt,
            triangular: matches!(placement["spread_type"].as_str(), Some("triangular" | "minecraft:triangular")),
        });
    }
    Ok(spreads)
}

pub(crate) fn candidate(
    spreads: &BTreeMap<String, Spread>,
    name: &str,
    seed: i64,
    region_x: i32,
    region_z: i32,
) -> Option<ChunkPos> {
    let spread = spreads.get(name)?;
    let mut random = WorldgenRandom::new(LegacyRandom::new(0));
    random.set_large_feature_with_salt(seed, region_x, region_z, spread.salt);
    let mut offset = || {
        let first = random.next_int_bound(spread.range).unwrap_or(0);
        if spread.triangular {
            (first + random.next_int_bound(spread.range).unwrap_or(0)) / 2
        } else {
            first
        }
    };
    let (x, z) = (offset(), offset());
    Some(ChunkPos::new(
        region_x.wrapping_mul(spread.spacing).wrapping_add(x),
        region_z.wrapping_mul(spread.spacing).wrapping_add(z),
    ))
}
