// The path, what it breaks and places, and the goal, drawn over the world.
fn baritone_lines(
    lines: &[crate::gizmos::Line],
    camera: [f64; 3],
    view: glam::Mat4,
) -> core::ops::ControlFlow<Vec<[f32; 8]>> {
    let mine = mod_draimhorizon::lines::next();
    if mine.is_empty() {
        return core::ops::ControlFlow::Continue(());
    }
    let all: Vec<crate::gizmos::Line> = lines
        .iter()
        .copied()
        .chain(mine.iter().map(|l| crate::gizmos::Line {
            start: l.start,
            end: l.end,
            color: l.color,
            width: l.width,
        }))
        .collect();
    let mut out = Vec::with_capacity(all.len());
    crate::gizmos::camera_relative_to(&all, camera, view, |line| out.push(line));
    core::ops::ControlFlow::Break(out)
}
