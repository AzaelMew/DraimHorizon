// Candidate sampling only. GeneratorWorker::structure_starts remains the authority
// for frequency, exclusion, biome and terrain validation.
use std::collections::BTreeMap;
use sunrise_core::{
    position::ChunkPos,
    random::{LegacyRandom, RandomSource, WorldgenRandom},
};
pub(crate) use mod_draimhorizon::seed_placement::{Spread, load};

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
    let [x, z] = spread.candidate([region_x, region_z], |bound| {
        random.next_int_bound(bound).unwrap_or(0)
    });
    Some(ChunkPos::new(x, z))
}
