// Use Horizon's native suggestion list so placement, selection and input all agree.
use super::*;
impl ChatScreen {
    pub fn draimhorizons_nickname_list(
        &mut self,
        measure: &Measure,
        start: usize,
        names: Vec<String>,
        force: bool,
    ) {
        if self.suggestions.is_some() || start > self.input.value.len() {
            return;
        }
        self.awaiting = None;
        self.request = None;
        self.usage.clear();
        self.input.suggestion = None;
        self.pending = Some((
            start,
            names
                .into_iter()
                .map(|text| Suggestion {
                    range: (start, self.input.value.len()),
                    text,
                    tooltip: None,
                    int_value: None,
                })
                .collect(),
        ));
        self.allow_suggestions = true;
        if self.auto_suggestions || force {
            self.show_suggestions(measure, force);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_list_selects_cycles_and_clicks_pay_names() {
        let font = crate::font::Font::load(
            std::path::Path::new("/nonexistent-draimhorizons-test-font"),
            &std::collections::HashMap::new(),
            &[],
            (1, 1),
            (0, 0),
            crate::hud::Rect {
                u0: 0.0,
                v0: 0.0,
                u1: 1.0,
                v1: 1.0,
                w: 1,
                h: 1,
            },
        );
        let measure = Measure(&font);
        let registries = Registries::default();
        let commands = Commands {
            dispatcher: crate::command::vanilla::game_dispatcher(),
            registries: &registries,
        };
        let mut screen = ChatScreen::new(
            &measure,
            &commands,
            "/pay al",
            0,
            640,
            360,
            ChatAbilities::new(ChatVisibility::Full),
        );
        screen.draimhorizons_nickname_list(
            &measure,
            5,
            vec!["albert".into(), "Alex_42".into()],
            true,
        );
        assert!(screen.suggestions_visible());
        screen.key(
            &measure,
            &commands,
            &[],
            winit::keyboard::KeyCode::Tab,
            false,
            false,
        );
        assert_eq!(screen.value(), "/pay albert");
        screen.key(
            &measure,
            &commands,
            &[],
            winit::keyboard::KeyCode::Tab,
            false,
            false,
        );
        assert_eq!(screen.value(), "/pay Alex_42");
        screen.key(
            &measure,
            &commands,
            &[],
            winit::keyboard::KeyCode::Tab,
            true,
            false,
        );
        assert_eq!(screen.value(), "/pay albert");
        let rect = screen.suggestions.as_ref().unwrap().rect;
        assert!(screen.click(
            &measure,
            &commands,
            f64::from(rect[0] + 1),
            f64::from(rect[1] + 13)
        ));
        assert_eq!(screen.value(), "/pay Alex_42");
        screen.key(
            &measure,
            &commands,
            &[],
            winit::keyboard::KeyCode::Escape,
            false,
            false,
        );
        assert!(!screen.suggestions_visible());
    }
}
