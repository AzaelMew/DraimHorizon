#[path = "../platforms/sunrise/overlay/crate/zoom_state.rs"]
mod zoom_state;

use zoom_state::Zoom;

#[test]
fn wheel_works_before_any_camera_query_and_between_frames() {
    let mut zoom = Zoom::default();
    assert!(zoom.scroll(1, true));
    zoom.frame(0.1, true, true);
    assert!(zoom.scroll(1, true));
    for _ in 0..120 {
        zoom.frame(1.0 / 60.0, true, true);
    }
    assert!((zoom.scale() - 1.0 / 6.0).abs() < 0.00001);
    assert!(zoom.scroll(0, true));
    assert!(!zoom.scroll(1, false));
}

#[test]
fn entry_wheel_changes_and_release_are_continuous() {
    let mut zoom = Zoom::default();
    zoom.frame(1.0 / 60.0, true, true);
    let first = zoom.scale();
    assert!(first < 1.0 && first > 0.25);
    zoom.scroll(1, true);
    assert_eq!(zoom.scale(), first);
    zoom.frame(1.0 / 60.0, true, true);
    assert!(zoom.scale() < first);
    let held = zoom.scale();
    zoom.frame(1.0 / 60.0, true, false);
    assert!(zoom.scale() > held && zoom.scale() < 1.0);
}

#[test]
fn smooth_zoom_has_the_same_timing_at_different_frame_rates() {
    let scales: Vec<_> = [30, 60, 144]
        .into_iter()
        .map(|fps| {
            let mut zoom = Zoom::default();
            for _ in 0..fps / 6 {
                zoom.frame(1.0 / fps as f32, true, true);
                // Repeated FOV reads must never advance the animation.
                let scale = zoom.scale();
                assert_eq!(zoom.scale(), scale);
            }
            zoom.scale()
        })
        .collect();
    assert!(scales.iter().all(|scale| (*scale - scales[0]).abs() < 0.00001));
}

#[test]
fn forbidden_context_clears_zoom_and_extreme_wheel_steps_stay_bounded() {
    let mut zoom = Zoom::default();
    zoom.scroll(i32::MAX, true);
    for _ in 0..120 {
        zoom.frame(1.0 / 60.0, true, true);
    }
    assert!((zoom.scale() - 0.1).abs() < 0.00001);
    zoom.frame(1.0 / 60.0, false, true);
    assert_eq!(zoom.scale(), 1.0);
    zoom.scroll(i32::MIN, true);
    for _ in 0..120 {
        zoom.frame(1.0 / 60.0, true, true);
    }
    assert!((zoom.scale() - 0.5).abs() < 0.00001);
    let before = zoom.scale();
    zoom.frame(f32::NAN, true, true);
    assert_eq!(zoom.scale(), before);
}
