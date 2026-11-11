// Third-person pat arms, inserted into horizon-client-lib's `mobs::anim` as
// `claim_borders_pat_pose`. The game thread publishes one pose per animated player entity each
// frame; after the model's own animation (`anim::setup`) the patting arm is blended towards it.
// Nothing else of the model changes, and an entity without a published pose is left alone.
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The patting arm of one player model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    pub entity: u32,
    /// `None`: the model's own main arm.
    pub left: Option<bool>,
    /// 0 keeps the vanilla pose, 1 replaces the arm's rotation.
    pub weight: f32,
    /// Right-arm x, y, z rotation in radians; y and z are mirrored for a left arm.
    pub rotation: [f32; 3],
}

const MAX: usize = 16;
static ANY: AtomicBool = AtomicBool::new(false);
static ARMS: Mutex<Vec<Arm>> = Mutex::new(Vec::new());

/// Replaces the published poses; an empty list stops every pat arm at once.
pub fn publish(arms: &[Arm]) {
    let mut published = ARMS.lock().unwrap_or_else(|e| e.into_inner());
    published.clear();
    published.extend(
        arms.iter()
            .filter(|a| a.weight.is_finite() && a.rotation.iter().all(|r| r.is_finite()))
            .take(MAX)
            .copied(),
    );
    ANY.store(!published.is_empty(), Ordering::Release);
}

/// The model pass: blend the patting arm of `m`, if it pats.
pub fn apply(m: &crate::mobs::Mob, p: &mut crate::mobs::anim::Poses) {
    if !ANY.load(Ordering::Acquire) {
        return;
    }
    let Ok(arms) = ARMS.try_lock() else {
        return;
    };
    let Some(arm) = arms.iter().find(|a| a.entity == m.id).copied() else {
        return;
    };
    drop(arms);
    let left = arm.left.unwrap_or_else(|| m.main_arm_left());
    let name = if left { "left_arm" } else { "right_arm" };
    if !p.has(name) {
        return;
    }
    let mirror = if left { -1.0 } else { 1.0 };
    let target = [
        arm.rotation[0],
        arm.rotation[1] * mirror,
        arm.rotation[2] * mirror,
    ];
    let weight = arm.weight.clamp(0.0, 1.0);
    let part = p.get(name);
    for (i, goal) in target.into_iter().enumerate() {
        part[3 + i] += (goal - part[3 + i]) * weight;
    }
}
