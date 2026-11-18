//! The ghost renderer lives in a game module; the render hooks run in a lower crate and reach it
//! through the pointers the module registers (the frame's `WorldDrawn` travels type-erased).
use std::sync::OnceLock;

type Draw = fn(*mut ());
type Wanted = fn() -> bool;

static DRAW: OnceLock<Draw> = OnceLock::new();
static WANTED: OnceLock<Wanted> = OnceLock::new();

pub fn register(draw: Draw, wanted: Wanted) {
    let _ = DRAW.set(draw);
    let _ = WANTED.set(wanted);
}

pub fn draw(world: *mut ()) {
    if let Some(draw) = DRAW.get() {
        draw(world);
    }
}

pub fn wanted() -> bool {
    WANTED.get().is_some_and(|wanted| wanted())
}
