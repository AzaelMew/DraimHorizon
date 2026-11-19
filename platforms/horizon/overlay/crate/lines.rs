//! Baritone's lines, handed over by the game side once a tick and added to the lines drawn over
//! everything. The gizmo renderer calls `camera_relative` three times a frame (depth-tested lines,
//! lines over everything, crosshair axes): `next` answers the first two.
use std::sync::Mutex;
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Line {
    pub start: [f64; 3],
    pub end: [f64; 3],
    pub color: u32,
    pub width: f32,
}

static LINES: Mutex<(Vec<Line>, Vec<Line>)> = Mutex::new((Vec::new(), Vec::new()));
static CALLS: Mutex<(Option<Instant>, u32)> = Mutex::new((None, 0));

/// The depth-tested lines, and the lines drawn over everything (`ignoreDepth` of the renderer's settings).
pub fn set(depth: Vec<Line>, on_top: Vec<Line>) {
    if let Ok(mut held) = LINES.lock() {
        *held = (depth, on_top);
    }
}

/// Our lines for this call of `camera_relative`; empty when it is not one of ours to extend.
pub fn next() -> Vec<Line> {
    let which = match CALLS.lock() {
        Ok(mut c) => {
            let now = Instant::now();
            if c.0.is_none_or(|t| now.duration_since(t).as_millis() > 100) {
                c.1 = 0;
            }
            c.0 = Some(now);
            c.1 += 1;
            (c.1 - 1) % 3
        }
        Err(_) => 2,
    };
    if which > 1 {
        return Vec::new();
    }
    LINES
        .lock()
        .map(|l| if which == 0 { l.0.clone() } else { l.1.clone() })
        .unwrap_or_default()
}
