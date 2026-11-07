// A read-only view of native scan results; the map never requests extra server chunks.
impl super::Minimap {
    pub fn claim_borders_tiles(
        &self,
        since: std::time::Instant,
    ) -> impl Iterator<Item = ((i32, i32), &[u8; 256], &[i16; 256], &[u8; 256])> {
        self.tiles
            .iter()
            .filter(move |(_, tile)| tile.scanned >= since)
            .map(|(key, tile)| (*key, &tile.color, &tile.height, &tile.depth))
    }
}
