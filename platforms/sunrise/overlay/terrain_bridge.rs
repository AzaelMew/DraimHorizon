// A read-only view of native scan results; the map never requests extra server chunks.
// A free function: an inherent `impl Minimap` cannot sit in the game crate.
pub fn tiles(
    map: &sunrise_client_render::minimap::Minimap,
    since: std::time::Instant,
) -> impl Iterator<Item = ((i32, i32), &[u8; 256], &[i16; 256], &[u8; 256])> {
    map.tiles
        .iter()
        .filter(move |(_, tile)| tile.scanned >= since)
        .map(|(key, tile)| (*key, &tile.color, &tile.height, &tile.depth))
}
