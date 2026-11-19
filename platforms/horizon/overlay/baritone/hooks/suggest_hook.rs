fn baritone_suggest(screen: &mut ChatScreen, m: &Measure) -> std::ops::ControlFlow<()> {
    // Player names for the commands that take one, filtered by the text the screen holds right now. The
    // list is built and shown in this update; any other input, or no match, takes the vanilla update.
    let input = &screen.input;
    if input.cursor == input.value.chars().count()
        && input.highlight == input.cursor
        && !(screen.keep_suggestions && screen.suggestions.is_some())
    {
        if let Some((start, names)) = mod_draimhorizon::suggest::nicknames(&input.value) {
            let end = input.value.len();
            screen.parse = None;
            screen.suggestions = None;
            screen.input.suggestion = None;
            screen.usage.clear();
            screen.awaiting = None;
            screen.request = None;
            let list = names
                .into_iter()
                .map(|text| Suggestion {
                    range: (start, end),
                    text,
                    tooltip: None,
                    int_value: None,
                })
                .collect();
            screen.pending = Some((start, list));
            screen.usage_width = 0;
            screen.usage_position = 0;
            if screen.allow_suggestions && screen.auto_suggestions {
                screen.show_suggestions(m, false);
            }
            return std::ops::ControlFlow::Break(());
        }
    }
    let text = screen.input.value.clone();
    if text.starts_with('/') || text.chars().all(char::is_whitespace) {
        return std::ops::ControlFlow::Continue(());
    }
    let Some(provide) = mod_draimhorizon::suggest::get() else {
        return std::ops::ControlFlow::Continue(());
    };
    let cursor_b = byte_at(&text, screen.input.cursor);
    let partial = &text[..cursor_b];
    let Some((start, names)) = provide(partial) else {
        return std::ops::ControlFlow::Continue(());
    };
    if screen.keep_suggestions && screen.suggestions.is_some() {
        return std::ops::ControlFlow::Break(());
    }
    screen.parse = None;
    screen.input.suggestion = None;
    screen.suggestions = None;
    screen.usage.clear();
    // MixinCommandSuggestionHelper: the completions in the order they came, already filtered.
    let list = names
        .into_iter()
        .map(|text| Suggestion {
            range: (start, partial.len()),
            text,
            tooltip: None,
            int_value: None,
        })
        .collect();
    screen.awaiting = None;
    screen.pending = Some((start, list));
    screen.usage_width = 0;
    screen.usage_position = 0;
    if screen.allow_suggestions && screen.auto_suggestions {
        screen.show_suggestions(m, false);
    }
    std::ops::ControlFlow::Break(())
}
