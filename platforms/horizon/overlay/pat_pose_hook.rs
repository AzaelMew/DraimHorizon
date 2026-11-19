// The model pass: blend the patting arm of `m`, if it pats (poses come from the mod crate).
fn claim_borders_pat_pose(
    _value: (),
    m: &horizon_client::mobs::Mob,
    p: &mut horizon_client::mobs::anim::Poses,
) {
    let Some(arm) = mod_draimhorizon::pat::pose_for(m.id, m.join.get()) else {
        return;
    };
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
