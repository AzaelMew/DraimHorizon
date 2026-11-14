// Pure parts of the bridge between DraimHorizon's farm/mining and the Baritone port: how a walk
// ends, how long it may take, the port settings the Baritone tab offers and the status text.
// No game types, so it is tested on its own.
use claim_borders_relay_client::translation::Language;

/// Where a walk handed to the port stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walk {
    /// No walk was started (or it was cancelled by us).
    Idle,
    Walking,
    Arrived,
    /// The port stopped without reaching the goal: no path, cancelled from the tab or by `#cancel`,
    /// or the time ran out.
    Failed,
}

/// The longest a walk may take: ten ticks a block with a minute of slack, never under two minutes.
pub fn timeout_ticks(distance_blocks: f64) -> u32 {
    let d = if distance_blocks.is_finite() {
        distance_blocks.clamp(0.0, 4096.0)
    } else {
        4096.0
    };
    ((d * 10.0) as u32 + 1200).max(2400)
}

/// What a walk is doing: `started` is whether we handed one over, `active` whether the port's goal
/// process still holds it, `at_goal` whether the player stands in the goal.
pub fn decide(started: bool, active: bool, at_goal: bool, elapsed: u32, timeout: u32) -> Walk {
    if !started {
        Walk::Idle
    } else if at_goal {
        Walk::Arrived
    } else if !active || elapsed > timeout {
        Walk::Failed
    } else {
        Walk::Walking
    }
}

/// Settings of the port that a walk for our farm or mining must not change, as (name, value while
/// walking). Both are put back when the walk ends; neither is saved.
pub const FARM_WALK: [(&str, &str); 2] = [("allowBreak", "false"), ("allowPlace", "false")];
/// Mining approaches may not place blocks; our own job breaks the ore.
pub const MINING_WALK: [(&str, &str); 2] = [("allowBreak", "false"), ("allowPlace", "false")];

/// A setting the Baritone tab shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Flag,
    /// An integer changed in steps within `min..=max`.
    Int {
        min: i32,
        max: i32,
        step: i32,
    },
}
#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub name: &'static str,
    pub ru: &'static str,
    pub en: &'static str,
    pub kind: Kind,
}
impl Setting {
    pub fn label(&self, language: Language) -> &'static str {
        language.text(self.ru, self.en)
    }
}
/// What the Settings section lists, in order. The rest stays reachable through `#set`.
pub const SETTINGS: [Setting; 8] = [
    Setting {
        name: "allowPlace",
        ru: "Ставить блоки",
        en: "Place blocks",
        kind: Kind::Flag,
    },
    Setting {
        name: "allowBreak",
        ru: "Ломать блоки",
        en: "Break blocks",
        kind: Kind::Flag,
    },
    Setting {
        name: "allowParkour",
        ru: "Паркур",
        en: "Parkour",
        kind: Kind::Flag,
    },
    Setting {
        name: "allowDiagonalDescend",
        ru: "Спуск по диагонали",
        en: "Diagonal descend",
        kind: Kind::Flag,
    },
    Setting {
        name: "allowWaterBucketFall",
        ru: "Ведро воды при падении",
        en: "Water bucket landing",
        kind: Kind::Flag,
    },
    Setting {
        name: "allowSprint",
        ru: "Бег",
        en: "Sprint",
        kind: Kind::Flag,
    },
    Setting {
        name: "renderPath",
        ru: "Рисовать путь",
        en: "Draw the path",
        kind: Kind::Flag,
    },
    Setting {
        name: "maxFallHeightNoWater",
        ru: "Макс. падение",
        en: "Max fall",
        kind: Kind::Int {
            min: 3,
            max: 30,
            step: 1,
        },
    },
];

/// The value a stepper moves to: one `step` up or down, kept inside the range.
pub fn step_value(current: i32, kind: Kind, up: bool) -> i32 {
    match kind {
        Kind::Flag => current,
        Kind::Int { min, max, step } => {
            let next = if up {
                current.saturating_add(step)
            } else {
                current.saturating_sub(step)
            };
            next.clamp(min, max)
        }
    }
}

/// Ticks as "12 s", "3 min 5 s" or "1 h 2 min"; `None` and non-finite values are "—".
pub fn format_ticks(ticks: Option<f64>) -> String {
    let Some(t) = ticks.filter(|t| t.is_finite() && *t >= 0.0) else {
        return "—".to_owned();
    };
    let seconds = (t / 20.0).round() as u64;
    match seconds {
        0..=59 => format!("{seconds} s"),
        60..=3599 => format!("{} min {} s", seconds / 60, seconds % 60),
        _ => format!("{} h {} min", seconds / 3600, seconds % 3600 / 60),
    }
}

/// The status line of the tab: what the port is doing and when it expects to finish.
pub fn status_text(
    language: Language,
    process: Option<&str>,
    segment: Option<f64>,
    goal: Option<f64>,
) -> String {
    let Some(process) = process else {
        return language.text("Задач нет", "Idle").to_owned();
    };
    let eta = language.text("осталось", "ETA");
    format!("{process} · {eta} {}", format_ticks(goal.or(segment)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_walk_ends_by_arriving_failing_or_timing_out() {
        assert_eq!(decide(false, true, true, 0, 100), Walk::Idle);
        assert_eq!(decide(true, true, false, 5, 100), Walk::Walking);
        assert_eq!(decide(true, false, true, 5, 100), Walk::Arrived);
        // Standing in the goal wins even when the port has just let go of it.
        assert_eq!(decide(true, true, true, 5, 100), Walk::Arrived);
        // Let go away from the goal: no path, or cancelled.
        assert_eq!(decide(true, false, false, 5, 100), Walk::Failed);
        assert_eq!(decide(true, true, false, 101, 100), Walk::Failed);
        assert_eq!(decide(true, true, false, 100, 100), Walk::Walking);
    }

    #[test]
    fn the_time_allowed_grows_with_distance_and_stays_bounded() {
        assert_eq!(timeout_ticks(0.0), 2400);
        assert_eq!(timeout_ticks(f64::NAN), 4096 * 10 + 1200);
        assert!(timeout_ticks(300.0) > timeout_ticks(100.0));
        assert_eq!(timeout_ticks(1e12), 4096 * 10 + 1200);
        assert_eq!(timeout_ticks(-5.0), 2400);
    }

    #[test]
    fn the_settings_list_is_unique_and_steppers_stay_in_range() {
        let mut names: Vec<_> = SETTINGS.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), SETTINGS.len());
        for s in SETTINGS {
            assert!(!s.ru.is_empty() && !s.en.is_empty());
            assert!(s.en.is_ascii());
        }
        let kind = Kind::Int {
            min: 3,
            max: 30,
            step: 1,
        };
        assert_eq!(step_value(3, kind, false), 3);
        assert_eq!(step_value(30, kind, true), 30);
        assert_eq!(step_value(10, kind, true), 11);
        assert_eq!(step_value(i32::MAX, kind, true), 30);
        assert_eq!(step_value(7, Kind::Flag, true), 7);
    }

    #[test]
    fn times_and_status_read_in_both_languages() {
        assert_eq!(format_ticks(None), "—");
        assert_eq!(format_ticks(Some(f64::INFINITY)), "—");
        assert_eq!(format_ticks(Some(-1.0)), "—");
        assert_eq!(format_ticks(Some(200.0)), "10 s");
        assert_eq!(format_ticks(Some(20.0 * 185.0)), "3 min 5 s");
        assert_eq!(format_ticks(Some(20.0 * 3720.0)), "1 h 2 min");
        assert_eq!(status_text(Language::English, None, None, None), "Idle");
        assert_eq!(
            status_text(Language::Russian, None, None, None),
            "Задач нет"
        );
        assert_eq!(
            status_text(Language::English, Some("Mining"), Some(40.0), None),
            "Mining · ETA 2 s"
        );
        assert_eq!(
            status_text(Language::Russian, Some("Mining"), Some(40.0), Some(400.0)),
            "Mining · осталось 20 s"
        );
    }

    #[test]
    fn farm_and_mining_walks_never_break_or_place() {
        for walk in [FARM_WALK, MINING_WALK] {
            assert!(walk.contains(&("allowBreak", "false")));
            assert!(walk.contains(&("allowPlace", "false")));
        }
    }
}
