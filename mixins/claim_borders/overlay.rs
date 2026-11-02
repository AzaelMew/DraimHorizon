// Pure projection and overlay data shared by tests and the injected client.
#[derive(Clone, Debug, PartialEq)]
pub struct FriendRow {
    pub name: String,
    /// None means unavailable, not zero health.
    pub health_percent: Option<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub player: Option<u128>,
    pub label: String,
    /// World-space label anchor; dimensions are filtered by the caller.
    pub position: [f64; 3],
    /// RGB; opacity is separate so distant labels fade without changing hue.
    pub color: u32,
    pub alpha: f32,
    /// Distance from the local player's position, independent of third-person camera.
    pub distance: f64,
}

/// Perspective in GUI pixels, using Horizon's camera convention and vertical FOV.
/// Subtract in f64 before projecting so positions near the world border remain stable.
pub fn project(
    camera: [f64; 3],
    yaw: f32,
    pitch: f32,
    fov: f32,
    viewport: [f32; 2],
    position: [f64; 3],
) -> Option<[f32; 2]> {
    if !camera.iter().chain(position.iter()).all(|v| v.is_finite())
        || ![yaw, pitch, fov, viewport[0], viewport[1]]
            .iter()
            .all(|v| v.is_finite())
        || !(1.0..179.0).contains(&fov)
        || viewport[0] <= 0.0
        || viewport[1] <= 0.0
    {
        return None;
    }
    let offset: [f64; 3] = std::array::from_fn(|i| position[i] - camera[i]);
    let (sy, cy) = f64::from(yaw).to_radians().sin_cos();
    let (sp, cp) = f64::from(pitch).to_radians().sin_cos();
    let forward = [-sy * cp, -sp, cy * cp];
    let right = [-cy, 0.0, -sy];
    let up = [-sy * sp, cp, cy * sp];
    let dot = |direction: [f64; 3]| {
        direction
            .into_iter()
            .zip(offset)
            .map(|(a, b)| a * b)
            .sum::<f64>()
    };
    let depth = dot(forward);
    if depth <= 0.05 || !depth.is_finite() {
        return None;
    }
    let focal = f64::from(viewport[1]) / (2.0 * (f64::from(fov) * 0.5).to_radians().tan());
    let x = f64::from(viewport[0]) * 0.5 + dot(right) * focal / depth;
    let y = f64::from(viewport[1]) * 0.5 - dot(up) * focal / depth;
    if !x.is_finite()
        || !y.is_finite()
        || x < 0.0
        || x > f64::from(viewport[0])
        || y < 0.0
        || y > f64::from(viewport[1])
    {
        return None;
    }
    Some([x as f32, y as f32])
}

pub const MARKER_TEXT_SCALE: f32 = 0.8;
pub const MARKER_WIDTH: f32 = 72.0;
pub const MARKER_HEIGHT: f32 = 16.0;
pub const MARKER_TEXT_WIDTH: f32 = 68.0;
pub const MARKER_ICON_SIZE: f32 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkerLayout {
    pub icon: [f32; 4],
    pub bounds: [f32; 4],
}

/// All dimensions are GUI pixels, independent of distance and label contents.
pub fn marker_layout(viewport: [f32; 2], anchor: [f32; 2]) -> Option<MarkerLayout> {
    if !viewport.iter().chain(anchor.iter()).all(|v| v.is_finite())
        || viewport[0] < MARKER_WIDTH + 4.0
        || viewport[1] < MARKER_HEIGHT + 4.0
    {
        return None;
    }
    let center = anchor[0].clamp(
        MARKER_WIDTH / 2.0 + 2.0,
        viewport[0] - MARKER_WIDTH / 2.0 - 2.0,
    );
    let top = (anchor[1] + 4.0).clamp(2.0, viewport[1] - MARKER_HEIGHT - 2.0);
    let half_icon = MARKER_ICON_SIZE / 2.0;
    Some(MarkerLayout {
        icon: [
            anchor[0].clamp(half_icon, viewport[0] - half_icon) - half_icon,
            anchor[1].clamp(half_icon, viewport[1] - half_icon) - half_icon,
            MARKER_ICON_SIZE,
            MARKER_ICON_SIZE,
        ],
        bounds: [
            center - MARKER_WIDTH / 2.0,
            top,
            MARKER_WIDTH,
            MARKER_HEIGHT,
        ],
    })
}

/// Truncate on character boundaries using the active resource pack's font.
pub fn marker_label(label: &str, width: impl Fn(&str) -> f32) -> String {
    let fits = |text: &str| width(text) * MARKER_TEXT_SCALE <= MARKER_TEXT_WIDTH;
    if fits(label) {
        return label.to_owned();
    }
    let mut shortened = label.to_owned();
    while shortened.pop().is_some() {
        let candidate = format!("{shortened}...");
        if fits(&candidate) {
            return candidate;
        }
    }
    String::new()
}

pub fn distance_label(distance: f64) -> Option<String> {
    if !distance.is_finite() || distance < 0.0 {
        None
    } else if distance >= 999_950.0 {
        // Even malformed or world-border distances cannot widen the marker.
        Some("999+ km".to_owned())
    } else if distance >= 1000.0 {
        Some(format!("{:.1} km", distance / 1000.0))
    } else {
        Some(format!("{distance:.0} m"))
    }
}

#[cfg(test)]
pub fn friend_label(row: &FriendRow) -> String {
    match row.health_percent {
        Some(percent) => format!("{}  {}%", row.name, percent.min(100)),
        None => format!("{}  ?", row.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_bounds_do_not_grow_with_distance_or_label_length() {
        let viewport = [400.0, 300.0];
        let expected = marker_layout(viewport, [200.0, 150.0]).unwrap();
        let measure = |label: &str| label.chars().count() as f32 * 6.0;
        for distance in [10.0, 100.0, 1_000.0, 30_000_000.0, f64::MAX] {
            let anchor = project([0.0; 3], 0.0, 0.0, 90.0, viewport, [0.0, 0.0, distance]).unwrap();
            assert_eq!(marker_layout(viewport, anchor), Some(expected));
            let distance = marker_label(&distance_label(distance).unwrap(), measure);
            assert!(measure(&distance) * MARKER_TEXT_SCALE <= MARKER_TEXT_WIDTH);
        }
        assert_eq!(expected.icon[2..], [2.0, 2.0]);
        assert_eq!(expected.bounds[2..], [72.0, 16.0]);
        for name in [
            "Alex",
            "Очень длинное название точки назначения",
            "界".repeat(80).as_str(),
        ] {
            let label = marker_label(name, measure);
            assert!(measure(&label) * MARKER_TEXT_SCALE <= MARKER_TEXT_WIDTH);
            if measure(name) * MARKER_TEXT_SCALE > MARKER_TEXT_WIDTH {
                assert!(label.ends_with("..."));
            }
        }
    }

    #[test]
    fn fixed_marker_bounds_stay_inside_viewport_edges() {
        let viewport = [400.0, 300.0];
        for anchor in [[0.0, 0.0], [400.0, 300.0], [200.0, 150.0]] {
            let layout = marker_layout(viewport, anchor).unwrap();
            assert_eq!(layout.bounds[2..], [MARKER_WIDTH, MARKER_HEIGHT]);
            for [x, y, w, h] in [layout.icon, layout.bounds] {
                assert!(x >= 0.0 && y >= 0.0);
                assert!(x + w <= viewport[0] && y + h <= viewport[1]);
            }
        }
        assert_eq!(marker_layout([70.0, 300.0], [0.0; 2]), None);
        assert_eq!(marker_layout(viewport, [f32::NAN, 0.0]), None);
    }

    #[test]
    fn front_projects_to_center_and_behind_or_near_plane_is_hidden() {
        let view = [400.0, 300.0];
        assert_eq!(
            project([0.0; 3], 0.0, 0.0, 90.0, view, [0.0, 0.0, 10.0]),
            Some([200.0, 150.0])
        );
        assert_eq!(
            project([0.0; 3], 0.0, 0.0, 90.0, view, [0.0, 0.0, -10.0]),
            None
        );
        assert_eq!(
            project([0.0; 3], 0.0, 0.0, 90.0, view, [0.0, 0.0, 0.01]),
            None
        );
    }

    #[test]
    fn minecraft_yaw_and_vertical_fov_preserve_screen_directions() {
        let p = project([0.0; 3], 0.0, 0.0, 90.0, [400.0, 300.0], [-5.0, 5.0, 10.0]).unwrap();
        assert!((p[0] - 275.0).abs() < 0.001);
        assert!((p[1] - 75.0).abs() < 0.001);
        let p = project([0.0; 3], 90.0, 0.0, 90.0, [400.0, 300.0], [-10.0, 0.0, 0.0]).unwrap();
        assert!((p[0] - 200.0).abs() < 0.001);
    }

    #[test]
    fn world_border_precision_offscreen_and_invalid_inputs() {
        let camera = [29_999_000.0, 64.0, 29_999_000.0];
        assert_eq!(
            project(
                camera,
                0.0,
                0.0,
                90.0,
                [400.0, 300.0],
                [camera[0], 64.0, camera[2] + 10.0]
            ),
            Some([200.0, 150.0])
        );
        assert_eq!(
            project(
                [0.0; 3],
                0.0,
                0.0,
                90.0,
                [400.0, 300.0],
                [1000.0, 0.0, 10.0]
            ),
            None
        );
        assert_eq!(
            project([0.0; 3], 0.0, 0.0, 90.0, [0.0, 300.0], [0.0, 0.0, 10.0]),
            None
        );
        assert_eq!(
            project(
                [0.0; 3],
                f32::NAN,
                0.0,
                90.0,
                [400.0, 300.0],
                [0.0, 0.0, 10.0]
            ),
            None
        );
        assert_eq!(
            project([0.0; 3], 0.0, 0.0, 0.0, [400.0, 300.0], [0.0, 0.0, 10.0]),
            None
        );
    }

    #[test]
    fn distances_and_unknown_health_are_not_invented() {
        assert_eq!(distance_label(12.4).as_deref(), Some("12 m"));
        assert_eq!(distance_label(1525.0).as_deref(), Some("1.5 km"));
        assert_eq!(distance_label(f64::INFINITY), None);
        assert_eq!(distance_label(-1.0), None);
        assert_eq!(
            friend_label(&FriendRow {
                name: "Alex".into(),
                health_percent: None
            }),
            "Alex  ?"
        );
        assert_eq!(
            friend_label(&FriendRow {
                name: "Alex".into(),
                health_percent: Some(75)
            }),
            "Alex  75%"
        );
    }
}
