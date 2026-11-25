#[path = "../platforms/sunrise/overlay/crate/seed_placement.rs"]
mod placement;

use std::collections::BTreeMap;

fn resource(placement: serde_json::Value) -> BTreeMap<String, String> {
    BTreeMap::from([(
        "minecraft:test".into(),
        serde_json::json!({"placement": placement}).to_string(),
    )])
}

#[test]
fn uniform_candidates_respect_negative_regions_and_separation() {
    let raw = resource(serde_json::json!({
        "type": "minecraft:random_spread", "spacing": 32, "separation": 22, "salt": 123
    }));
    let spreads = placement::load(&raw).unwrap();
    let spread = &spreads["minecraft:test"];
    assert_eq!(spread.salt, 123);
    let mut draws = [0, 9].into_iter();
    let point = spread.candidate([-1, -2], |bound| {
        assert_eq!(bound, 10);
        draws.next().unwrap()
    });
    assert_eq!(point, [-32, -55]);
    assert!(draws.next().is_none());
}

#[test]
fn triangular_candidates_draw_twice_per_axis_in_order() {
    let raw = resource(serde_json::json!({
        "type": "minecraft:random_spread", "spacing": 32, "separation": 22,
        "spread_type": "triangular"
    }));
    let spreads = placement::load(&raw).unwrap();
    let spread = &spreads["minecraft:test"];
    assert_eq!(spread.salt, 0);
    let mut draws = [2, 8, 1, 5].into_iter();
    assert_eq!(
        spread.candidate([-1, 2], |_| draws.next().unwrap()),
        [-27, 67]
    );
    assert!(draws.next().is_none());
}

#[test]
fn invalid_spacing_is_refused_and_non_spread_sets_are_left_to_the_generator() {
    for (spacing, separation) in [(0, 0), (8, 8), (8, -1), (4097, 0)] {
        let raw = resource(serde_json::json!({
            "type": "minecraft:random_spread", "spacing": spacing, "separation": separation
        }));
        assert!(placement::load(&raw).is_err());
    }
    let raw = resource(serde_json::json!({"type": "minecraft:concentric_rings"}));
    assert!(placement::load(&raw).unwrap().is_empty());
}
