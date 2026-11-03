// Geometric checks shared with native interaction. Only one attached decoration is skipped.
pub fn supporting_frame(pos: [i32; 3], step: [i32; 3]) -> Option<[i32; 3]> {
    if ![[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].contains(&step) {
        return None;
    }
    let mut out = pos;
    for i in 0..3 {
        out[i] = pos[i].checked_sub(step[i])?;
    }
    Some(out)
}
pub fn supporting_sign(name: &str, pos: [i32; 3], facing: Option<&str>) -> Option<[i32; 3]> {
    let name = name.strip_prefix("minecraft:")?;
    let step = if name.ends_with("_wall_sign") || name.ends_with("_wall_hanging_sign") {
        match facing? {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "west" => [-1, 0, 0],
            "east" => [1, 0, 0],
            _ => return None,
        }
    } else if name.ends_with("_hanging_sign") {
        [0, -1, 0]
    } else if name.ends_with("_sign") {
        [0, 1, 0]
    } else {
        return None;
    };
    supporting_frame(pos, step)
}
/// Intersect a ray with the supporting unit block and return its near face/hit point.
/// Reject a support behind the eye, out of reach or not actually under the crosshair.
pub fn support_hit(
    eye: [f64; 3],
    dir: [f64; 3],
    pos: [i32; 3],
    reach: f64,
) -> Option<([i32; 3], [f64; 3])> {
    if !reach.is_finite() || reach <= 0.0 || !eye.iter().chain(&dir).all(|n| n.is_finite()) {
        return None;
    }
    let norm = dir.iter().map(|n| n * n).sum::<f64>();
    if (norm - 1.0).abs() > 0.001 {
        return None;
    }
    let (mut near, mut far) = (0.0f64, reach);
    let mut face = [0; 3];
    for a in 0..3 {
        let lo = f64::from(pos[a]);
        let hi = lo + 1.0;
        if dir[a].abs() < 1e-9 {
            if eye[a] < lo || eye[a] > hi {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((lo - eye[a]) / dir[a], (hi - eye[a]) / dir[a]);
        let mut sign = -1;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
            sign = 1;
        }
        if t0 > near {
            near = t0;
            face = [0; 3];
            face[a] = sign;
        }
        far = far.min(t1);
        if near > far {
            return None;
        }
    }
    if face == [0; 3] || near > reach {
        return None;
    }
    Some((face, std::array::from_fn(|a| eye[a] + dir[a] * near)))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attached_supports_and_reach_do_not_skip_walls() {
        assert_eq!(
            supporting_sign("minecraft:oak_wall_sign", [0, 1, 1], Some("south")),
            Some([0, 1, 0])
        );
        assert_eq!(
            supporting_sign("minecraft:oak_sign", [0, 1, 0], None),
            Some([0, 0, 0])
        );
        assert_eq!(
            supporting_sign("minecraft:oak_hanging_sign", [0, 1, 0], None),
            Some([0, 2, 0])
        );
        assert!(supporting_sign("minecraft:stone", [0, 0, 0], None).is_none());
        assert!(supporting_frame([0, 0, 0], [1, 1, 0]).is_none());
        let (face, point) = support_hit([0.5, 0.5, 3.0], [0.0, 0.0, -1.0], [0, 0, 0], 4.5).unwrap();
        assert_eq!(face, [0, 0, 1]);
        assert_eq!(point, [0.5, 0.5, 1.0]);
        assert!(support_hit([0.5, 0.5, 6.0], [0.0, 0.0, -1.0], [0, 0, 0], 4.5).is_none());
        assert!(support_hit([2.0, 0.5, 3.0], [0.0, 0.0, -1.0], [0, 0, 0], 4.5).is_none());
        assert!(support_hit([0.5, 0.5, 3.0], [0.0, 0.0, 1.0], [0, 0, 0], 4.5).is_none());
    }
}
