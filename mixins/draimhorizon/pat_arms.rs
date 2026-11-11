// Pose identities shared by the native model pass and standalone regression tests.
// The local third-person model is unjoined; remote mobs have a process-unique join
// sequence. A server entity id alone cannot distinguish the two (both can be zero).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Model {
    pub entity: u32,
    pub join: u64,
}

/// The patting arm of one exact model, not every entity with the same server id.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arm {
    pub model: Model,
    /// `None`: the model's own main arm.
    pub left: Option<bool>,
    /// 0 keeps the vanilla pose, 1 replaces the arm's rotation.
    pub weight: f32,
    /// Right-arm x, y, z rotation in radians; y and z are mirrored for a left arm.
    pub rotation: [f32; 3],
}

pub fn select(arms: &[Arm], model: Model) -> Option<Arm> {
    arms.iter().find(|a| a.model == model).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arm(join: u64, rotation: f32) -> Arm {
        Arm {
            model: Model { entity: 0, join },
            left: None,
            weight: 1.0,
            rotation: [rotation, 0.0, 0.0],
        }
    }

    #[test]
    fn local_and_remote_entity_zero_never_share_a_pose() {
        let local = arm(0, -2.0);
        let remote = arm(73, -1.0);
        assert_eq!(select(&[local], remote.model), None);
        assert_eq!(select(&[remote], local.model), None);
        for arms in [[local, remote], [remote, local]] {
            assert_eq!(select(&arms, local.model), Some(local));
            assert_eq!(select(&arms, remote.model), Some(remote));
        }
    }

    #[test]
    fn reused_server_ids_do_not_inherit_a_departed_models_pose() {
        let departed = arm(73, -1.0);
        assert_eq!(
            select(
                &[departed],
                Model {
                    entity: 0,
                    join: 74
                }
            ),
            None
        );
        assert_eq!(
            select(
                &[departed],
                Model {
                    entity: 1,
                    join: 73
                }
            ),
            None
        );
    }
}
