// Render-only selection; no entity metadata or active effects are modified.
pub fn enabled(mod_on: bool, experiments_accepted: bool, setting: Option<&str>) -> bool {
    mod_on && experiments_accepted && setting == Some("true")
}

pub fn reveal(shared_flag: bool, potion: bool, removed: bool, camera: bool) -> bool {
    (shared_flag || potion) && !removed && !camera
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mod_and_missing_or_false_settings_never_reveal_entities() {
        for setting in [None, Some("false"), Some("invalid"), Some("true")] {
            assert!(!enabled(false, true, setting));
            assert!(!enabled(true, false, setting));
        }
        assert!(!enabled(true, true, None));
        assert!(!enabled(true, true, Some("false")));
        assert!(enabled(true, true, Some("true")));
    }

    #[test]
    fn reveal_handles_metadata_and_potions_without_duplicating_visible_bodies() {
        assert!(!reveal(false, false, false, false));
        assert!(reveal(true, false, false, false));
        assert!(reveal(false, true, false, false));
        assert!(reveal(true, true, false, false));
        for (flag, potion) in [(true, false), (false, true), (true, true)] {
            assert!(!reveal(flag, potion, true, false));
            assert!(!reveal(flag, potion, false, true));
        }
    }
}
