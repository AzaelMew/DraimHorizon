fn claim_borders_world_map_slot(images: &mut Vec<(String, u32, u32, Vec<u8>)>) {
    images.push((
        "claim_borders/seed_map".into(),
        256,
        256,
        vec![0; 256 * 256 * 4],
    ));
    images.push((
        "claim_borders/world_map".into(),
        768,
        768,
        vec![0; 768 * 768 * 4],
    ));
}
