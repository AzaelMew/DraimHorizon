use claim_borders::{
    economy,
    economy_ui::{DetailScroll, Panel},
    menu::{
        self, Action, BaritoneMode, Control, ExperimentalSection, FarmSection, Helper,
        HelperCategory, Language, Menu, Tab, ViewModel,
    },
    outgoing::{TranslationControl, TranslationField, TranslationScreen, TranslationTargets},
    ui_style::{DialogLayout, PagedPanel},
};
use std::collections::BTreeSet;

fn separate(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] + a[2] <= b[0] || b[0] + b[2] <= a[0] || a[1] + a[3] <= b[1] || b[1] + b[3] <= a[1]
}
fn bounded(r: [f32; 4], size: [f32; 2]) {
    assert!(r.iter().all(|v| v.is_finite()));
    assert!(
        r[0] >= 0.0 && r[1] >= 0.0 && r[2] > 0.0 && r[3] >= 18.0,
        "{r:?}"
    );
    assert!(
        r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1],
        "{r:?} exceeds {size:?}"
    );
}
/// Clicks a Helpers control wherever it sits, switching category and page as a player would.
fn press(menu: &mut Menu, model: &ViewModel, size: [f32; 2], control: Control) -> Option<Action> {
    let visible = |menu: &Menu| {
        menu.layout(model, size[0], size[1])
            .iter()
            .any(|w| w.control == Some(control))
    };
    if menu.tab == Tab::Helpers && !menu.experiments_prompt && !visible(menu) {
        for category in HelperCategory::ALL {
            click(menu, model, size, Control::HelperCategory(category));
            for page in 0..8 {
                menu.helper_page = page;
                if visible(menu) {
                    return click(menu, model, size, control);
                }
            }
        }
    }
    click(menu, model, size, control)
}
fn menu_with(set: impl FnOnce(&mut Menu)) -> Menu {
    let mut menu = Menu::default();
    set(&mut menu);
    menu
}
fn click(menu: &mut Menu, model: &ViewModel, size: [f32; 2], control: Control) -> Option<Action> {
    let r = menu
        .layout(model, size[0], size[1])
        .into_iter()
        .find(|w| w.control == Some(control))
        .unwrap()
        .rect;
    menu.click(model, size, [r[0] + r[2] * 0.5, r[1] + r[3] * 0.5])
}

#[test]
fn baritone_footer_text_and_controls_have_separate_rows_in_both_languages() {
    for language in [Language::Russian, Language::English] {
        for assigned in [false, true] {
            let model = ViewModel {
                language,
                navigation_status: "No task running · press the start key".into(),
                hotkeys: if assigned {
                    ["KEYPAD 1", "KEYPAD 2", "KEYPAD 3", "KEYPAD 4"].map(str::to_owned)
                } else {
                    Default::default()
                },
                farm_storage: [Some([120, 64, -350]), None, None],
                schematic_places: (0..6)
                    .map(|index| (index, format!("Preview {index}"), true, false))
                    .collect(),
                ..ViewModel::default()
            };
            for size in [
                [240.0, 216.0],
                [320.0, 240.0],
                [400.0, 280.0],
                [536.0, 260.0],
                [800.0, 500.0],
                [1280.0, 720.0],
            ] {
                {
                    for page in 0..4 {
                        let mut menu = Menu::default();
                        menu.language = language;
                        menu.tab = Tab::Schematics;
                        let _ = page;
                        let widgets = menu.layout(&model, size[0], size[1]);
                        let mut footer_count = 0;
                        for (i, widget) in widgets.iter().enumerate() {
                            let hint = widget.text.starts_with("Preview KEYPAD")
                                || widget.text.starts_with("Просмотр KEYPAD")
                                || widget.text.starts_with("No keys assigned:")
                                || widget.text.starts_with("Клавиши не назначены:");
                            let footer = hint
                                || widget.text == model.navigation_status
                                || matches!(
                                    widget.control,
                                    Some(Control::AutoEat | Control::FarmStorage(_))
                                );
                            if !footer {
                                continue;
                            }
                            footer_count += 1;
                            bounded(widget.rect, size);
                            for (j, other) in widgets.iter().enumerate() {
                                if i != j && !other.text.is_empty() {
                                    assert!(
                                        separate(widget.rect, other.rect),
                                        "footer {:?} {:?} overlaps {:?} {:?} at {size:?}, {language:?}",
                                        widget.control,
                                        widget.text,
                                        other.control,
                                        other.text
                                    );
                                }
                            }
                        }
                        // The Schematics tab has its folder and materials buttons in its own panel.
                        let expected = 3;
                        assert_eq!(
                            footer_count, expected,
                            "actions, status, auto-eat and key hint must all remain visible"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn trading_is_a_dedicated_destination_and_all_groups_remain_reachable() {
    for language in [Language::Russian, Language::English] {
        for size in [[240.0, 216.0], [400.0, 280.0], [800.0, 500.0]] {
            let model = ViewModel::default();
            let mut menu = Menu::default();
            menu.language = language;
            menu.tab = Tab::Settings;
            assert!(
                !menu
                    .layout(&model, size[0], size[1])
                    .iter()
                    .any(|w| w.control == Some(Control::SettingsTab(4)))
            );
            if menu
                .layout(&model, size[0], size[1])
                .iter()
                .any(|w| w.control == Some(Control::NavigationMenu))
            {
                click(&mut menu, &model, size, Control::NavigationMenu);
            }
            click(&mut menu, &model, size, Control::Tab(Tab::Trading));
            assert_eq!(menu.tab, Tab::Trading);
            let mut keys = BTreeSet::new();
            for group in 0..5 {
                click(&mut menu, &model, size, Control::TradingGroup(group));
                for _ in 0..64 {
                    let widgets = menu.layout(&model, size[0], size[1]);
                    let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
                    for (i, a) in controls.iter().enumerate() {
                        bounded(a.rect, size);
                        for b in &controls[i + 1..] {
                            assert!(
                                separate(a.rect, b.rect),
                                "{:?} overlaps {:?} at {size:?}",
                                a.control,
                                b.control
                            );
                        }
                        if let Some(Control::EconomySetting(key, _) | Control::EconomyChoice(key)) =
                            a.control
                        {
                            keys.insert(key);
                        }
                    }
                    assert!(
                        widgets
                            .iter()
                            .any(|w| w.control == Some(Control::TradingGroup(group)) && w.selected)
                    );
                    assert!(widgets.iter().any(|w| w.control == Some(Control::Economy)));
                    let page = menu.settings_page;
                    // A single page has no paging controls.
                    if !widgets
                        .iter()
                        .any(|w| w.control == Some(Control::SettingsPage(1)))
                    {
                        break;
                    }
                    click(&mut menu, &model, size, Control::SettingsPage(1));
                    if menu.settings_page == page {
                        break;
                    }
                    assert!(menu.settings_page < 64);
                }
            }
            assert_eq!(
                keys,
                economy::SETTINGS
                    .iter()
                    .filter(|s| s.group <= 4)
                    .map(|s| s.key)
                    .collect()
            );
            click(&mut menu, &model, size, Control::Tab(Tab::Settings));
            assert!(
                menu.layout(&model, size[0], size[1])
                    .iter()
                    .any(|w| matches!(w.control, Some(Control::SettingsTab(_))) && w.selected)
            );
        }
    }
}

#[test]
fn compact_navigation_reaches_every_destination_without_starting_a_task() {
    let model = ViewModel {
        experiments_accepted: true,
        ..ViewModel::default()
    };
    for tab in [
        Tab::Waypoints,
        Tab::Claims,
        Tab::Party,
        Tab::Chat,
        Tab::Trading,
        Tab::Helpers,
        Tab::Experimental,
    ] {
        let mut menu = Menu::default();
        let size = [240.0, 216.0];
        click(&mut menu, &model, size, Control::NavigationMenu);
        assert!(menu.navigation_menu);
        assert!(click(&mut menu, &model, size, Control::Tab(tab)).is_none());
        assert_eq!(menu.tab, tab);
        assert!(!menu.navigation_menu);
    }
}

/// Every helper control the QOL tab hosts (they were Experiments → Helpers at the old base).
fn qol_controls() -> Vec<Control> {
    vec![
        Control::AutoEat,
        Control::AutoEatFoods,
        Control::AutoEatToHeal,
        Control::AutoToolsOnlyHolding,
        Control::AutoToolsFromInventory,
        Control::Helper(Helper::Totem),
        Control::Helper(Helper::Fill),
        Control::Helper(Helper::Tools),
        Control::Helper(Helper::Resume),
        Control::Helper(Helper::HumanLook),
        Control::Helper(Helper::AutoClick),
        Control::AutoClickButton,
        Control::AutoClickRate,
    ]
}
/// What Experiments → Automation (Farm and Drain), the Baritone tab and the Schematics tab own.
/// None of it belongs in QOL.
fn upstream_automation_controls() -> Vec<Control> {
    let mut all = vec![
        Control::BaritoneMode(BaritoneMode::Farm),
        Control::BaritoneMode(BaritoneMode::Drain),
        Control::DrainRadius,
        Control::DrainPattern,
        Control::BaritoneEnable,
    ];
    all.extend(FarmSection::ALL.map(Control::FarmSection));
    all
}
/// Walks every category and page of the Helpers tab through the real Back/Next actions.
fn walk_helpers(
    menu: &mut Menu,
    model: &ViewModel,
    size: [f32; 2],
) -> Vec<(HelperCategory, usize, Vec<claim_borders::menu::Widget>)> {
    let mut seen = Vec::new();
    for category in HelperCategory::ALL {
        click(menu, model, size, Control::HelperCategory(category));
        assert_eq!(menu.helper_category, category);
        assert_eq!(menu.helper_page, 0, "a category opens on its first page");
        loop {
            let widgets = menu.layout(model, size[0], size[1]);
            let next = widgets
                .iter()
                .filter_map(|w| match w.control {
                    Some(Control::HelperPage(page)) if page > menu.helper_page => Some(page),
                    _ => None,
                })
                .max();
            seen.push((category, menu.helper_page, widgets));
            let Some(next) = next else { break };
            click(menu, model, size, Control::HelperPage(next));
        }
    }
    seen
}

#[test]
fn helpers_are_a_top_level_tab_and_no_longer_an_experiments_section() {
    let model = ViewModel {
        experiments_accepted: true,
        ..ViewModel::default()
    };
    for language in [Language::Russian, Language::English] {
        for size in [[800.0_f32, 500.0], [640.0, 300.0], [240.0, 216.0]] {
            let mut menu = menu_with(|m| {
                m.language = language;
            });
            // Compact menus list the destinations behind the section button.
            if menu::rail_width(
                (size[0] - 16.0).clamp(220.0, 720.0),
                (size[1] - 16.0).clamp(200.0, 400.0),
            ) == 0.0
            {
                click(&mut menu, &model, size, Control::NavigationMenu);
            }
            let tabs: Vec<_> = menu
                .layout(&model, size[0], size[1])
                .into_iter()
                .filter(|w| matches!(w.control, Some(Control::Tab(_))))
                .collect();
            let label = Tab::Helpers.label(language);
            assert!(
                tabs.iter()
                    .any(|w| w.control == Some(Control::Tab(Tab::Helpers)) && w.text == label)
            );
            assert_eq!(
                label,
                match language {
                    Language::Russian => "Помощники",
                    Language::English => "QOL",
                }
            );
            click(&mut menu, &model, size, Control::Tab(Tab::Helpers));
            assert_eq!(menu.tab, Tab::Helpers);
            // Experiments keeps its own sections: Ores and Automation (Farm, Drain).
            menu.tab = Tab::Experimental;
            let sections: Vec<_> = menu
                .layout(&model, size[0], size[1])
                .into_iter()
                .filter(|w| matches!(w.control, Some(Control::ExperimentalTab(_))))
                .map(|w| w.control)
                .collect();
            assert_eq!(
                sections,
                [ExperimentalSection::Ores, ExperimentalSection::Baritone]
                    .map(|s| Some(Control::ExperimentalTab(s)))
            );
            // No QOL helper is listed under Experiments (the Baritone footer's own "Eat"
            // switch was always there).
            for section in [ExperimentalSection::Ores, ExperimentalSection::Baritone] {
                menu.experimental_section = section;
                for w in menu.layout(&model, size[0], size[1]) {
                    let control = w.control.unwrap_or(Control::Close);
                    let footer_eat =
                        control == Control::AutoEat && section == ExperimentalSection::Baritone;
                    assert!(
                        !qol_controls().contains(&control) || footer_eat,
                        "{:?} in {section:?}",
                        w.control
                    );
                }
            }
        }
    }
}

#[test]
fn every_original_helper_control_is_reachable_by_category_and_page_at_all_sizes() {
    let model = ViewModel {
        auto_eat: true,
        ..ViewModel::default()
    };
    for language in [Language::Russian, Language::English] {
        for size in [
            [240.0, 216.0],
            [320.0, 240.0],
            [400.0, 280.0],
            [640.0, 300.0],
            [800.0, 500.0],
        ] {
            let mut menu = menu_with(|m| {
                m.language = language;
                m.tab = Tab::Helpers;
            });
            let mut found = Vec::new();
            for (_, _, widgets) in walk_helpers(&mut menu, &model, size) {
                let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
                for (i, widget) in controls.iter().enumerate() {
                    bounded(widget.rect, size);
                    for other in &controls[i + 1..] {
                        assert!(
                            separate(widget.rect, other.rect),
                            "{:?} overlaps {:?} at {size:?}",
                            widget.control,
                            other.control
                        );
                    }
                    found.push(widget.control.unwrap());
                }
                // Hint lines never run under a control.
                for hint in widgets.iter().filter(|w| w.control.is_none() && w.muted) {
                    for control in &controls {
                        assert!(separate(hint.rect, control.rect));
                    }
                }
            }
            for control in qol_controls().into_iter().chain([Control::ToggleReconnect]) {
                assert!(
                    found.contains(&control),
                    "lost {control:?} at {size:?} in {language:?}"
                );
            }
            for control in upstream_automation_controls() {
                assert!(
                    !found.contains(&control),
                    "{control:?} belongs to Automation, found in QOL at {size:?}"
                );
            }
            // A category row is always among the controls, never paged away.
            for category in HelperCategory::ALL {
                assert!(found.contains(&Control::HelperCategory(category)));
            }
        }
    }
}

#[test]
fn helper_categories_keep_each_switch_with_the_choices_that_depend_on_it() {
    let model = ViewModel::default();
    let size = [800.0, 500.0];
    let mut menu = menu_with(|m| {
        m.language = Language::English;
        m.tab = Tab::Helpers;
    });
    let pages = walk_helpers(&mut menu, &model, size);
    // Roomy menus show a whole category on one page.
    assert_eq!(pages.len(), HelperCategory::ALL.len());
    let groups = [
        (
            HelperCategory::Survival,
            vec![
                Control::AutoEat,
                Control::AutoEatFoods,
                Control::AutoEatToHeal,
            ],
        ),
        (
            HelperCategory::Survival,
            vec![Control::Helper(Helper::Totem)],
        ),
        (HelperCategory::Gear, vec![Control::Helper(Helper::Fill)]),
        (
            HelperCategory::Gear,
            vec![
                Control::Helper(Helper::Tools),
                Control::AutoToolsOnlyHolding,
                Control::AutoToolsFromInventory,
            ],
        ),
        (
            HelperCategory::Controls,
            vec![
                Control::Helper(Helper::AutoClick),
                Control::AutoClickButton,
                Control::AutoClickRate,
            ],
        ),
        (
            HelperCategory::Controls,
            vec![Control::Helper(Helper::Resume)],
        ),
        (
            HelperCategory::Controls,
            vec![Control::Helper(Helper::HumanLook)],
        ),
        (HelperCategory::Controls, vec![Control::ToggleReconnect]),
    ];
    for (category, controls) in groups {
        let (_, _, widgets) = pages.iter().find(|(c, _, _)| *c == category).unwrap();
        let rects: Vec<_> = controls
            .iter()
            .map(|c| {
                widgets
                    .iter()
                    .find(|w| w.control == Some(*c))
                    .unwrap_or_else(|| panic!("{c:?} not in {category:?}"))
            })
            .collect();
        // The parent comes first and its choices follow in reading order, indented.
        for pair in rects.windows(2) {
            assert!(pair[0].rect[1] <= pair[1].rect[1]);
        }
        for child in &rects[1..] {
            assert!(
                child.rect[0] > rects[0].rect[0],
                "choices are indented under the switch"
            );
        }
    }
    // While a parent is off its choices stay savable but say what they wait for.
    let (_, _, gear) = pages
        .iter()
        .find(|(c, _, _)| *c == HelperCategory::Gear)
        .unwrap();
    let held = gear
        .iter()
        .find(|w| w.control == Some(Control::AutoToolsOnlyHolding))
        .unwrap();
    assert!(
        held.muted && held.text.contains("needs Auto Tools"),
        "{}",
        held.text
    );
    let on = ViewModel {
        helpers: [true; 6],
        ..ViewModel::default()
    };
    menu.helper_category = HelperCategory::Gear;
    let held = menu
        .layout(&on, size[0], size[1])
        .into_iter()
        .find(|w| w.control == Some(Control::AutoToolsOnlyHolding))
        .unwrap();
    assert!(!held.muted && !held.text.contains("needs"));
    let accepted = ViewModel {
        experiments_accepted: true,
        ..ViewModel::default()
    };
    assert_eq!(
        click(&mut menu, &accepted, size, Control::AutoToolsOnlyHolding),
        Some(Action::SetAutoToolsOnlyHolding(true)),
        "idle choices are not dead click areas"
    );
}

#[test]
fn helper_pages_start_a_block_on_a_fresh_page_when_it_does_not_fit() {
    assert_eq!(Menu::helper_pages(&[3, 1], 4), vec![0..4]);
    assert_eq!(Menu::helper_pages(&[3, 1], 3), vec![0..3, 3..4]);
    assert_eq!(Menu::helper_pages(&[1, 3], 3), vec![0..1, 1..4]);
    assert_eq!(Menu::helper_pages(&[3, 1], 2), vec![0..2, 2..4]);
    assert_eq!(Menu::helper_pages(&[1, 3], 2), vec![0..1, 1..3, 3..4]);
    assert_eq!(Menu::helper_pages(&[], 4), vec![0..0]);
    for per in 1..8 {
        let pages = Menu::helper_pages(&[3, 1, 3, 2], per);
        assert_eq!(pages.first().unwrap().start, 0);
        assert_eq!(pages.last().unwrap().end, 9);
        assert!(pages.windows(2).all(|p| p[0].end == p[1].start));
        assert!(pages.iter().all(|p| p.len() <= per));
    }
}

#[test]
fn food_picker_round_trips_and_keeps_selections_across_categories_and_tabs() {
    let mut model = ViewModel {
        auto_eat_options: (0..40)
            .map(|i| (format!("minecraft:f{i}"), format!("Food {i}"), 4))
            .collect(),
        auto_eat_foods: vec!["minecraft:f3".into(), "minecraft:f9".into()],
        ..ViewModel::default()
    };
    for size in [[240.0, 216.0], [640.0, 300.0], [800.0, 500.0]] {
        let mut menu = menu_with(|m| {
            m.tab = Tab::Helpers;
            m.language = Language::English;
        });
        click(&mut menu, &model, size, Control::AutoEatFoods);
        assert!(menu.auto_eat_food_picker, "{size:?}");
        let widgets = menu.layout(&model, size[0], size[1]);
        assert!(
            widgets
                .iter()
                .any(|w| matches!(w.control, Some(Control::AutoEatFood(_))))
        );
        // The picker replaces the category list, and its Back row returns to the same category.
        assert!(
            !widgets
                .iter()
                .any(|w| matches!(w.control, Some(Control::HelperCategory(_))))
        );
        assert_eq!(
            click(&mut menu, &model, size, Control::AutoEatFood(5)),
            Some(Action::SetAutoEatFood("minecraft:f5".into(), true))
        );
        click(&mut menu, &model, size, Control::AutoEatFoods);
        assert!(!menu.auto_eat_food_picker);
        assert_eq!(menu.helper_category, HelperCategory::Survival);
        assert!(
            menu.layout(&model, size[0], size[1])
                .iter()
                .any(|w| w.control == Some(Control::AutoEatFoods) && w.text.contains("2"))
        );
        // Leaving the tab with the picker open returns to the list, selections are model state.
        click(&mut menu, &model, size, Control::AutoEatFoods);
        for tab in [Tab::Waypoints, Tab::Helpers] {
            let has_rail = menu::rail_width(
                (size[0] - 16.0).clamp(220.0, 720.0),
                (size[1] - 16.0).clamp(200.0, 400.0),
            ) > 0.0;
            if !has_rail {
                click(&mut menu, &model, size, Control::NavigationMenu);
            }
            click(&mut menu, &model, size, Control::Tab(tab));
        }
        assert!(!menu.auto_eat_food_picker);
        assert_eq!(model.auto_eat_foods, ["minecraft:f3", "minecraft:f9"]);
        // Another category and back does not touch the selection either.
        click(
            &mut menu,
            &model,
            size,
            Control::HelperCategory(HelperCategory::Gear),
        );
        click(
            &mut menu,
            &model,
            size,
            Control::HelperCategory(HelperCategory::Survival),
        );
        assert!(
            menu.layout(&model, size[0], size[1])
                .iter()
                .any(|w| w.control == Some(Control::AutoEatFoods) && w.text.contains("2"))
        );
    }
    model.auto_eat_foods.clear();
}

#[test]
fn helpers_keyboard_walks_categories_pages_and_leaves_the_picker() {
    let model = ViewModel {
        auto_eat: true,
        auto_eat_options: (0..60)
            .map(|i| (format!("minecraft:f{i}"), format!("Food {i}"), 4))
            .collect(),
        ..ViewModel::default()
    };
    let size = [240.0, 216.0];
    let mut menu = menu_with(|m| {
        m.tab = Tab::Helpers;
        m.language = Language::English;
    });
    // Tab never focuses a hidden text field; it cycles categories and Shift+Tab goes back.
    assert!(menu.key("key.keyboard.tab", 0));
    assert_eq!(menu.helper_category, HelperCategory::Gear);
    assert!(menu.key("key.keyboard.tab", 1));
    assert!(menu.key("key.keyboard.tab", 1));
    assert_eq!(menu.helper_category, HelperCategory::Controls);
    assert!(menu.key("key.keyboard.tab", 0));
    assert_eq!(menu.helper_category, HelperCategory::Survival);
    assert!(menu.focus.is_none());
    // Page Down / Page Up take the Back and Next steps and stop at the ends.
    let pages = menu.helper_page_count(&model, 200.0);
    assert!(pages > 1);
    for expected in 1..pages {
        assert!(menu.helper_page_key(&model, size, true));
        assert_eq!(menu.helper_page, expected);
    }
    assert!(menu.helper_page_key(&model, size, true));
    assert_eq!(menu.helper_page, pages - 1);
    for expected in (0..pages - 1).rev() {
        assert!(menu.helper_page_key(&model, size, false));
        assert_eq!(menu.helper_page, expected);
    }
    // Inside the picker the same keys page the food tiles and Escape goes back, not out.
    click(&mut menu, &model, size, Control::AutoEatFoods);
    assert!(menu.helper_page_key(&model, size, true));
    assert_eq!(menu.auto_eat_food_page, 1);
    assert!(menu.helper_page_key(&model, size, false));
    assert_eq!(menu.auto_eat_food_page, 0);
    assert!(menu.key("key.keyboard.escape", 0));
    assert!(!menu.auto_eat_food_picker);
    assert!(
        !menu.key("key.keyboard.escape", 0),
        "Escape closes the menu from the list"
    );
    // Other tabs keep their own keys.
    menu.tab = Tab::Waypoints;
    assert!(!menu.helper_page_key(&model, size, true));
}

#[test]
fn switching_a_helper_on_still_asks_for_the_experiments_warning_once() {
    let size = [800.0, 500.0];
    for control in [
        Control::AutoEat,
        Control::AutoEatToHeal,
        Control::Helper(Helper::Totem),
        Control::AutoToolsFromInventory,
    ] {
        let mut model = ViewModel::default();
        let mut menu = menu_with(|m| {
            m.tab = Tab::Helpers;
        });
        press(&mut menu, &model, size, control);
        assert!(menu.experiments_prompt, "{control:?} needs the warning");
        let confirm = press(&mut menu, &model, size, Control::ConfirmExperiments);
        assert_eq!(confirm, Some(Action::AcceptExperiments));
        assert_eq!(
            menu.tab,
            Tab::Helpers,
            "the warning returns to the Helpers tab"
        );
        model.experiments_accepted = true;
        assert!(
            click(
                &mut menu,
                &model,
                size,
                Control::HelperCategory(HelperCategory::Survival)
            )
            .is_none()
        );
        assert!(press(&mut menu, &model, size, control).is_some());
        assert!(!menu.experiments_prompt);
    }
    // Declining stays where it was, and configuring or switching off never asks.
    let model = ViewModel {
        auto_eat: true,
        ..ViewModel::default()
    };
    let mut menu = menu_with(|m| {
        m.tab = Tab::Helpers;
    });
    assert!(press(&mut menu, &model, size, Control::AutoEat).is_some());
    assert!(!menu.experiments_prompt);
    let mut menu = menu_with(|m| {
        m.tab = Tab::Helpers;
    });
    press(&mut menu, &ViewModel::default(), size, Control::AutoEat);
    press(
        &mut menu,
        &ViewModel::default(),
        size,
        Control::DeclineExperiments,
    );
    assert_eq!(menu.tab, Tab::Helpers);
}

#[test]
fn helper_rail_and_compact_navigation_keep_usable_targets_for_every_destination() {
    let model = ViewModel::default();
    for (size, rail) in [
        ([800.0, 500.0], true),
        ([800.0, 330.0], true),
        ([800.0, 300.0], false),
        ([800.0, 280.0], false),
        ([240.0, 216.0], false),
    ] {
        let mut menu = Menu::default();
        if !rail {
            click(&mut menu, &model, size, Control::NavigationMenu);
        }
        let tabs: Vec<_> = menu
            .layout(&model, size[0], size[1])
            .into_iter()
            .filter(|w| {
                matches!(
                    w.control,
                    Some(Control::Tab(_) | Control::Language | Control::Close)
                )
            })
            .collect();
        assert_eq!(
            tabs.iter()
                .filter(|w| matches!(w.control, Some(Control::Tab(_))))
                .count(),
            10,
            "{size:?}"
        );
        for (i, a) in tabs.iter().enumerate() {
            bounded(a.rect, size);
            if rail {
                assert!(
                    a.rect[3] >= 18.0 || a.control == Some(Control::Close),
                    "{:?} {:?}",
                    a.control,
                    a.rect
                );
            }
            for b in &tabs[i + 1..] {
                assert!(
                    separate(a.rect, b.rect),
                    "{:?} overlaps {:?} at {size:?}",
                    a.control,
                    b.control
                );
            }
        }
    }
}

#[test]
fn modal_and_tabbed_inventory_geometry_reserves_readable_rows_and_actions() {
    for size in [
        [240.0, 216.0],
        [320.0, 240.0],
        [640.0, 360.0],
        [800.0, 500.0],
    ] {
        let form = DialogLayout::new(size, [360.0, 300.0]);
        let mut regions: Vec<_> = (0..form.capacity).map(|i| form.row(i)).collect();
        regions.extend(
            (0..2).flat_map(|column| [form.footer(column, true), form.footer(column, false)]),
        );
        for (i, r) in regions.iter().enumerate() {
            bounded(*r, size);
            for other in &regions[i + 1..] {
                assert!(separate(*r, *other));
            }
        }
        for preferred in [[360.0, 180.0], [440.0, 270.0]] {
            let modal = DialogLayout::new(size, preferred);
            bounded(modal.footer(0, false), size);
            bounded(modal.footer(1, false), size);
            assert!(separate(modal.body, modal.footer(0, false)));
        }
        let panel = PagedPanel::new(size).with_tabs();
        let mut rows: Vec<_> = (0..panel.capacity).map(|i| panel.row(i)).collect();
        rows.extend(
            (0..2).flat_map(|column| [panel.footer(column, true), panel.footer(column, false)]),
        );
        for (i, r) in rows.iter().enumerate() {
            bounded(*r, size);
            assert!(r[1] >= panel.rect[1] + 48.0);
            for other in &rows[i + 1..] {
                assert!(separate(*r, *other));
            }
        }
    }
}

#[test]
fn detail_scroll_reaches_hidden_content_and_clamps_after_resize() {
    let mut scroll = DetailScroll::default();
    scroll.set_limit(240.0);
    for _ in 0..20 {
        scroll.move_by(24.0);
    }
    assert_eq!(scroll.offset(), 240.0);
    scroll.set_limit(48.0);
    assert_eq!(scroll.offset(), 48.0);
    scroll.move_by(-24.0);
    assert_eq!(scroll.offset(), 24.0);
    scroll.move_by(f32::NAN);
    assert_eq!(scroll.offset(), 24.0);
    scroll.move_by(-100.0);
    assert_eq!(scroll.offset(), 0.0);
    scroll.set_limit(f32::INFINITY);
    scroll.move_by(24.0);
    assert_eq!(scroll.offset(), 0.0);
}

#[test]
fn small_trading_workspace_uses_full_width_detail_and_visible_sort_choices() {
    for size in [
        [240.0, 216.0],
        [320.0, 240.0],
        [640.0, 216.0],
        [800.0, 500.0],
    ] {
        let panel = Panel::new(size);
        let inner = [panel.rect[2], panel.rect[3]];
        let detail = panel.details();
        assert!(detail[2] >= 180.0 && detail[3] > 0.0);
        if panel.compact() {
            assert_eq!(detail[0], 8.0);
            assert_eq!(detail[2], panel.list_width());
        } else {
            assert!(panel.list_width() + 8.0 < detail[0]);
        }
        let options: Vec<_> = (0..7).map(|i| panel.sort_option(i)).collect();
        for (i, option) in options.iter().enumerate() {
            bounded(*option, inner);
            for other in &options[i + 1..] {
                assert!(separate(*option, *other));
            }
        }
    }
}

#[test]
fn translation_editor_keeps_every_language_choice_and_separates_navigation_from_save() {
    for language in [Language::Russian, Language::English] {
        for size in [[240.0, 216.0], [400.0, 240.0], [800.0, 500.0]] {
            for field in [
                TranslationField::Input,
                TranslationField::Output,
                TranslationField::Received,
                TranslationField::ChatSource,
            ] {
                let mut screen =
                    TranslationScreen::new(TranslationTargets::load(None, None, language));
                screen.apply(TranslationControl::Field(field), size);
                let mut choices = BTreeSet::new();
                for _ in 0..32 {
                    let widgets = screen.layout(size, language);
                    for (i, widget) in widgets.iter().enumerate() {
                        bounded(widget.rect, size);
                        for other in &widgets[i + 1..] {
                            assert!(separate(widget.rect, other.rect));
                        }
                        if let TranslationControl::Choose(target) = widget.control {
                            choices.insert(target.map(|l| l.tag()));
                        }
                    }
                    let page = screen.page;
                    screen.apply(TranslationControl::Next, size);
                    if screen.page == page {
                        break;
                    }
                }
                assert_eq!(choices.len(), 20 + usize::from(field.allows_auto()));
                assert_eq!(choices.contains(&None), field.allows_auto());
                assert!(!field.hint(language).is_empty());
            }
        }
    }
}

#[test]
fn map_and_chat_preferences_live_in_their_own_tabs() {
    use claim_borders::menu::{ChatSection, ClaimsSection, MapOption};
    let model = ViewModel::default();
    for language in [Language::Russian, Language::English] {
        for size in [[240.0, 216.0], [640.0, 360.0], [1280.0, 720.0]] {
            let mut menu = Menu::default();
            menu.language = language;
            menu.tab = Tab::Settings;
            // Settings keeps the mod-wide categories only.
            let settings = menu.layout(&model, size[0], size[1]);
            for section in [2, 3] {
                assert!(
                    !settings
                        .iter()
                        .any(|w| w.control == Some(Control::SettingsTab(section)))
                );
            }
            // The Chat tab holds every chat section and the translation switches.
            menu.tab = Tab::Chat;
            menu.chat_section = ChatSection::Appearance;
            let chat = menu.layout(&model, size[0], size[1]);
            assert!(
                chat.iter()
                    .any(|w| w.control == Some(Control::ChatTranslate))
            );
            for section in [
                ChatSection::Appearance,
                ChatSection::Notifications,
                ChatSection::Spam,
                ChatSection::DeepL,
            ] {
                assert!(
                    chat.iter()
                        .any(|w| w.control == Some(Control::ChatSection(section)))
                );
            }
            // Map and claims opens on the claims list, with the map preferences beside it.
            let mut claims = Menu::default();
            claims.language = language;
            claims.tab = Tab::Claims;
            let list = claims.layout(&model, size[0], size[1]);
            assert!(
                list.iter().any(|w| {
                    w.control == Some(Control::ClaimsSection(ClaimsSection::List)) && w.selected
                }),
                "{size:?} {:?}",
                list.iter()
                    .map(|w| (&w.text, w.control))
                    .collect::<Vec<_>>()
            );
            assert!(list.iter().any(|w| w.control == Some(Control::ClaimFilter)));
            click(
                &mut claims,
                &model,
                size,
                Control::ClaimsSection(ClaimsSection::Map),
            );
            assert_eq!(claims.tab, Tab::Claims);
            let map = claims.layout(&model, size[0], size[1]);
            assert!(map.iter().any(|w| w.control == Some(Control::MapToggle)));
            // A stepper is one labelled row with − and + buttons and a readout of the value.
            let minus = map
                .iter()
                .find(|w| w.control == Some(Control::MapAdjust(MapOption::Size, -1)))
                .unwrap();
            let plus = map
                .iter()
                .find(|w| w.control == Some(Control::MapAdjust(MapOption::Size, 1)))
                .unwrap();
            assert_eq!((minus.text.as_str(), plus.text.as_str()), ("−", "+"));
            assert!(minus.rect[0] + minus.rect[2] <= plus.rect[0]);
            let value = model.map_preferences.size.to_string();
            let readout = map.iter().find(|w| w.readout && w.text == value).unwrap();
            assert!(readout.rect[0] + readout.rect[2] <= minus.rect[0]);
            assert!(map.iter().any(|w| w.row
                && w.rect[0] <= readout.rect[0]
                && plus.rect[0] + plus.rect[2] <= w.rect[0] + w.rect[2]));
            assert!(
                !map.iter()
                    .any(|w| w.text.contains(" − ") || w.text.contains(" + "))
            );
        }
    }
}

#[test]
fn claims_next_reaches_every_claim_at_each_size() {
    use claim_borders::menu::ClaimRow;
    for count in [11, 12, 13, 25] {
        let model = ViewModel {
            claim_rows: (0..count)
                .map(|i| ClaimRow {
                    dimension: "minecraft:overworld".into(),
                    x: i,
                    z: 0,
                    label: format!("Claim {i}"),
                    owner: "Alex".into(),
                    foreign: false,
                })
                .collect(),
            ..ViewModel::default()
        };
        for size in [[800.0, 500.0], [640.0, 360.0], [1280.0, 720.0]] {
            let mut menu = Menu::default();
            menu.tab = Tab::Claims;
            let mut seen = BTreeSet::new();
            for _ in 0..32 {
                let widgets = menu.layout(&model, size[0], size[1]);
                for w in &widgets {
                    if let Some(Control::ClaimRow(i)) = w.control {
                        bounded(w.rect, size);
                        seen.insert(i);
                    }
                }
                let page = menu.claims_page;
                click(&mut menu, &model, size, Control::ClaimsNext);
                if menu.claims_page == page {
                    break;
                }
            }
            assert_eq!(
                seen.len(),
                count as usize,
                "{count} claims at {size:?}: saw {seen:?}"
            );
        }
    }
}

#[test]
fn farm_plots_are_listed_without_overlap_and_their_buttons_act() {
    for language in [Language::Russian, Language::English] {
        let model = ViewModel {
            language,
            experiments_accepted: true,
            navigation_status: "No task running · press the start key".into(),
            farm_storage: [Some([120, 64, -350]), None, None],
            farm_plots: (0..8)
                .map(|i| {
                    (
                        i * 2,
                        format!("Field {i} · 24×16 · 100 64 -300"),
                        i % 2 == 0,
                        i % 3 != 0,
                    )
                })
                .collect(),
            ..ViewModel::default()
        };
        for size in [
            [240.0, 216.0],
            [320.0, 240.0],
            [400.0, 280.0],
            [536.0, 260.0],
            [640.0, 360.0],
            [800.0, 500.0],
            [1280.0, 720.0],
        ] {
            let mut menu = Menu::default();
            menu.language = language;
            menu.tab = Tab::Experimental;
            menu.experimental_section = ExperimentalSection::Baritone;
            menu.baritone_mode = BaritoneMode::Farm;
            menu.farm_section = FarmSection::Plots;
            let widgets = menu.layout(&model, size[0], size[1]);
            let plot = |c: Option<Control>| {
                matches!(
                    c,
                    Some(
                        Control::FarmPlotFarm(_)
                            | Control::FarmPlotShow(_)
                            | Control::FarmPlotRemove(_)
                            | Control::FarmPlotPage(_)
                    )
                )
            };
            for (i, w) in widgets.iter().enumerate() {
                if !plot(w.control) {
                    continue;
                }
                let r = w.rect;
                assert!(
                    r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1]
                );
                for (j, other) in widgets.iter().enumerate() {
                    if i != j && !other.text.is_empty() {
                        assert!(
                            separate(r, other.rect),
                            "{:?} {:?} overlaps {:?} {:?} at {size:?} {language:?}",
                            w.control,
                            w.text,
                            other.control,
                            other.text
                        );
                    }
                }
            }
            // Every plot can be reached, paging when they do not fit, and its buttons act on
            // it.
            let mut seen = BTreeSet::new();
            for _ in 0..8 {
                let widgets = menu.layout(&model, size[0], size[1]);
                seen.extend(widgets.iter().filter_map(|w| match w.control {
                    Some(Control::FarmPlotFarm(i)) => Some(i),
                    _ => None,
                }));
                if widgets
                    .iter()
                    .any(|w| w.control == Some(Control::FarmPlotPage(1)))
                {
                    click(&mut menu, &model, size, Control::FarmPlotPage(1));
                } else {
                    break;
                }
            }
            // From a 640×360 screen (1280×720 at GUI scale 2) up, every plot is reachable.
            if size[1] >= 360.0 {
                assert_eq!(seen.len(), 8, "{size:?} {language:?}: {seen:?}");
            }
            if let Some(&index) = seen.iter().next() {
                let mut menu = Menu::default();
                menu.language = language;
                menu.tab = Tab::Experimental;
                menu.experimental_section = ExperimentalSection::Baritone;
                menu.baritone_mode = BaritoneMode::Farm;
                menu.farm_section = FarmSection::Plots;
                for (control, action) in [
                    (Control::FarmPlotFarm(index), Action::FarmPlotFarm(index)),
                    (Control::FarmPlotShow(index), Action::FarmPlotShow(index)),
                    (
                        Control::FarmPlotRemove(index),
                        Action::FarmPlotRemove(index),
                    ),
                ] {
                    assert_eq!(click(&mut menu, &model, size, control), Some(action));
                }
            }
        }
        // Without plots: how to mark one, and no limit control any more.
        let empty = ViewModel {
            language,
            experiments_accepted: true,
            ..ViewModel::default()
        };
        let mut menu = Menu::default();
        menu.language = language;
        menu.tab = Tab::Experimental;
        menu.experimental_section = ExperimentalSection::Baritone;
        menu.baritone_mode = BaritoneMode::Farm;
        menu.farm_section = FarmSection::Plots;
        let widgets = menu.layout(&empty, 800.0, 500.0);
        assert!(widgets.iter().any(|w| w.text.contains("/dfarm pos1")));
        assert!(!widgets.iter().any(|w| w.text.contains("0 = ∞")));
    }
}

#[test]
fn friends_previews_are_listed_after_own_ones_with_only_show_and_hide() {
    for language in [Language::Russian, Language::English] {
        let model = ViewModel {
            language,
            experiments_accepted: true,
            schematic_files: vec!["house.litematic".into()],
            schematic_places: (0..2)
                .map(|i| (i, format!("house.litematic · {i} 64 0 · 0°"), true, i == 0))
                .collect(),
            schematic_friend_places: (0..3)
                .map(|i| (format!("tower.litematic · Alex · {i} 70 5"), i != 1))
                .collect(),
            ..ViewModel::default()
        };
        for size in [
            [240.0, 216.0],
            [320.0, 240.0],
            [536.0, 260.0],
            [640.0, 360.0],
            [800.0, 500.0],
            [1280.0, 720.0],
        ] {
            let mut menu = Menu::default();
            menu.language = language;
            menu.tab = Tab::Schematics;
            let widgets = menu.layout(&model, size[0], size[1]);
            for (i, w) in widgets.iter().enumerate() {
                if !matches!(
                    w.control,
                    Some(Control::SchematicFriendShow(_) | Control::SchematicPlacePage(_))
                ) {
                    continue;
                }
                let r = w.rect;
                assert!(
                    r[0] >= 0.0 && r[1] >= 0.0 && r[0] + r[2] <= size[0] && r[1] + r[3] <= size[1]
                );
                for (j, other) in widgets.iter().enumerate() {
                    if i != j && !other.text.is_empty() {
                        assert!(
                            separate(r, other.rect),
                            "{:?} overlaps {:?} {:?} at {size:?}",
                            w.control,
                            other.control,
                            other.text
                        );
                    }
                }
            }
            // Every preview can be reached, paging when they do not fit.
            let mut friends = BTreeSet::new();
            let mut own = BTreeSet::new();
            for _ in 0..8 {
                let widgets = menu.layout(&model, size[0], size[1]);
                for w in &widgets {
                    match w.control {
                        Some(Control::SchematicFriendShow(i)) => {
                            friends.insert(i);
                        }
                        Some(Control::SchematicPlace(i)) => {
                            own.insert(i);
                        }
                        _ => {}
                    }
                }
                if widgets
                    .iter()
                    .any(|w| w.control == Some(Control::SchematicPlacePage(1)))
                {
                    click(&mut menu, &model, size, Control::SchematicPlacePage(1));
                } else {
                    break;
                }
            }
            if size[1] >= 360.0 {
                assert_eq!(friends.len(), 3, "{size:?}");
                assert_eq!(own.len(), 2, "{size:?}");
                let mut menu = Menu::default();
                menu.language = language;
                menu.tab = Tab::Schematics;
                // No select or delete for a friend's preview: only show or hide.
                let mut label = false;
                for _ in 0..8 {
                    let widgets = menu.layout(&model, size[0], size[1]);
                    label |= widgets
                        .iter()
                        .any(|w| w.text.contains("tower.litematic · Alex") && w.control.is_none());
                    if label
                        || !widgets
                            .iter()
                            .any(|w| w.control == Some(Control::SchematicPlacePage(1)))
                    {
                        break;
                    }
                    click(&mut menu, &model, size, Control::SchematicPlacePage(1));
                }
                assert!(label, "{size:?}");
                let mut menu = Menu::default();
                menu.language = language;
                menu.tab = Tab::Schematics;
                // Paged to the friend's row, its button acts on it.
                while !menu
                    .layout(&model, size[0], size[1])
                    .iter()
                    .any(|w| w.control == Some(Control::SchematicFriendShow(1)))
                {
                    click(&mut menu, &model, size, Control::SchematicPlacePage(1));
                }
                assert_eq!(
                    click(&mut menu, &model, size, Control::SchematicFriendShow(1)),
                    Some(Action::SchematicFriendShow(1))
                );
            }
        }
    }
}

#[test]
fn schematics_tab_lists_files_and_keeps_the_preview_controls_beside_them() {
    for language in [Language::Russian, Language::English] {
        let model = ViewModel {
            language,
            schematic_files: (0..40).map(|i| format!("build {i}.litematic")).collect(),
            schematic_places: vec![(0, "build 0.litematic · 1 2 3".into(), true, true)],
            ..ViewModel::default()
        };
        for size in [[536.0, 300.0], [800.0, 500.0], [1280.0, 720.0]] {
            let mut menu = Menu::default();
            menu.language = language;
            menu.tab = Tab::Schematics;
            let widgets = menu.layout(&model, size[0], size[1]);
            let find = |c: Control| widgets.iter().find(|w| w.control == Some(c));
            for c in [
                Control::SchematicRefresh,
                Control::SchematicFolder,
                Control::SchematicMaterials,
                Control::SchematicPreviewHere,
                Control::SchematicChoice(0),
                Control::SchematicLayerMode,
            ] {
                assert!(find(c).is_some(), "{c:?} missing at {size:?}");
            }
            // The list is to the left of the preview controls, and nothing overlaps.
            let file = find(Control::SchematicChoice(0)).unwrap().rect;
            let place = find(Control::SchematicPreviewHere).unwrap().rect;
            assert!(file[0] + file[2] <= place[0], "{file:?} {place:?}");
            let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
            for (i, a) in controls.iter().enumerate() {
                for b in &controls[i + 1..] {
                    assert!(
                        separate(a.rect, b.rect),
                        "{:?} overlaps {:?} at {size:?}",
                        a.control,
                        b.control
                    );
                }
            }
            // The wheel scrolls the list a row at a time; it never runs past the end.
            let first = |menu: &Menu| {
                menu.layout(&model, size[0], size[1])
                    .into_iter()
                    .find_map(|w| match w.control {
                        Some(Control::SchematicChoice(i)) => Some(i),
                        _ => None,
                    })
                    .unwrap()
            };
            menu.scroll(&model, size[1], -1.0);
            assert_eq!(first(&menu), 1);
            for _ in 0..100 {
                menu.scroll(&model, size[1], -1.0);
            }
            let last_first = first(&menu);
            assert!(last_first > 1 && last_first < 40);
            menu.scroll(&model, size[1], 1.0);
            assert_eq!(first(&menu), last_first - 1);
        }
    }
}

const MOVED: [(Control, HelperCategory); 4] = [
    (Control::ProtectGlowBerries, HelperCategory::Survival),
    (Control::ShowInvisibleEntities, HelperCategory::Survival),
    (Control::ToggleShulkerPeek, HelperCategory::Gear),
    (Control::ToggleReconnect, HelperCategory::Controls),
];

#[test]
fn qol_is_named_in_english_only_and_the_four_moved_settings_live_there() {
    assert_eq!(Tab::Helpers.label(Language::English), "QOL");
    assert_eq!(Tab::Helpers.label(Language::Russian), "Помощники");
    let model = ViewModel::default();
    for language in [Language::Russian, Language::English] {
        for size in [[800.0_f32, 500.0], [400.0, 280.0], [240.0, 216.0]] {
            // Settings → General no longer lists them.
            for page in 0..6 {
                let mut settings = menu_with(|m| {
                    m.tab = Tab::Settings;
                    m.language = language;
                    m.settings_section = 0;
                    m.settings_page = page;
                });
                for w in settings.layout(&model, size[0], size[1]) {
                    assert!(
                        !MOVED.iter().any(|(c, _)| w.control == Some(*c)),
                        "{:?} is still in Settings",
                        w.control
                    );
                }
                let _ = &mut settings;
            }
            // Experiments never listed them and still does not.
            let mut exp = menu_with(|m| {
                m.tab = Tab::Experimental;
                m.language = language;
            });
            for section in [ExperimentalSection::Ores, ExperimentalSection::Baritone] {
                exp.experimental_section = section;
                for w in exp.layout(&model, size[0], size[1]) {
                    assert!(!MOVED.iter().any(|(c, _)| w.control == Some(*c)));
                }
            }
            // QOL: each lives in its category, reachable by the real page actions.
            let mut menu = menu_with(|m| {
                m.tab = Tab::Helpers;
                m.language = language;
            });
            let walked = walk_helpers(&mut menu, &model, size);
            for (control, category) in MOVED {
                let hit = walked.iter().find(|(c, _, ws)| {
                    ws.iter().any(|w| w.control == Some(control)) && *c == category
                });
                assert!(hit.is_some(), "{control:?} not in {category:?} at {size:?}");
            }
        }
    }
}

#[test]
fn every_boolean_in_qol_is_a_switch_with_a_localized_state() {
    let toggles = [
        Control::AutoEat,
        Control::AutoEatToHeal,
        Control::AutoToolsOnlyHolding,
        Control::AutoToolsFromInventory,
        Control::ProtectGlowBerries,
        Control::ShowInvisibleEntities,
        Control::ToggleShulkerPeek,
        Control::ToggleReconnect,
        Control::Helper(Helper::Totem),
        Control::Helper(Helper::Fill),
        Control::Helper(Helper::Tools),
    ];
    for on in [false, true] {
        let model = ViewModel {
            auto_eat: on,
            auto_eat_to_heal: on,
            auto_tools_only_holding: on,
            auto_tools_from_inventory: on,
            protect_glow_berries: on,
            show_invisible_entities: on,
            shulker_peek: on,
            auto_reconnect: on,
            helpers: [on; 6],
            ..ViewModel::default()
        };
        for language in [Language::Russian, Language::English] {
            let mut menu = menu_with(|m| {
                m.tab = Tab::Helpers;
                m.language = language;
            });
            let mut seen = Vec::new();
            for (_, _, widgets) in walk_helpers(&mut menu, &model, [800.0, 500.0]) {
                for w in widgets {
                    let Some(control) = w.control.filter(|c| toggles.contains(c)) else {
                        continue;
                    };
                    let (_, shown) = claim_borders::ui_style::switch_label(&w.text)
                        .unwrap_or_else(|| panic!("{control:?} is not a switch: {}", w.text));
                    assert_eq!(shown, on, "{}", w.text);
                    assert_eq!(w.selected, on);
                    let words = match language {
                        Language::English => ["on", "off"],
                        Language::Russian => ["вкл", "выкл"],
                    };
                    assert!(w.text.ends_with(words[usize::from(!on)]), "{}", w.text);
                    seen.push(control);
                }
            }
            for control in &toggles {
                assert!(seen.contains(control), "{control:?} missing");
            }
        }
    }
}

#[test]
fn moved_settings_stay_actionable_in_both_directions() {
    let size = [800.0, 500.0];
    type Case = (Control, fn(bool) -> Action, fn(&mut ViewModel, bool));
    let cases: [Case; 3] = [
        (
            Control::ProtectGlowBerries,
            Action::SetProtectGlowBerries,
            |m, on| m.protect_glow_berries = on,
        ),
        (
            Control::ToggleShulkerPeek,
            Action::SetShulkerPeek,
            |m, on| m.shulker_peek = on,
        ),
        (
            Control::ShowInvisibleEntities,
            Action::SetShowInvisibleEntities,
            |m, on| m.show_invisible_entities = on,
        ),
    ];
    for (control, action, set) in cases {
        // Consent already given: both directions act, nothing else is asked.
        let mut model = ViewModel {
            experiments_accepted: true,
            ..ViewModel::default()
        };
        let mut menu = menu_with(|m| m.tab = Tab::Helpers);
        for on in [true, false] {
            assert_eq!(press(&mut menu, &model, size, control), Some(action(on)));
            assert!(!menu.experiments_prompt);
            set(&mut model, on);
        }
    }
    // Reconnect has a single toggle action.
    let model = ViewModel::default();
    let mut menu = menu_with(|m| m.tab = Tab::Helpers);
    assert_eq!(
        press(&mut menu, &model, size, Control::ToggleReconnect),
        Some(Action::ToggleReconnect)
    );
    // Benign moved settings need no experimental consent, as before.
    let mut menu = menu_with(|m| m.tab = Tab::Helpers);
    for control in [Control::ProtectGlowBerries, Control::ToggleShulkerPeek] {
        assert!(press(&mut menu, &model, size, control).is_some());
        assert!(!menu.experiments_prompt);
    }
}

#[test]
fn show_invisible_entities_keeps_consent_and_the_ban_warning_in_qol() {
    let size = [800.0, 500.0];
    for language in [Language::English, Language::Russian] {
        let mut model = ViewModel::default();
        let mut menu = menu_with(|m| {
            m.tab = Tab::Helpers;
            m.language = language;
        });
        // Switching on without consent shows the warning instead and returns to QOL.
        assert_eq!(
            press(&mut menu, &model, size, Control::ShowInvisibleEntities),
            None
        );
        assert!(menu.experiments_prompt);
        assert_eq!(
            press(&mut menu, &model, size, Control::ConfirmExperiments),
            Some(Action::AcceptExperiments)
        );
        assert_eq!(menu.tab, Tab::Helpers);
        // Declining leaves it off and in place.
        let mut menu = menu_with(|m| {
            m.tab = Tab::Helpers;
            m.language = language;
        });
        press(&mut menu, &model, size, Control::ShowInvisibleEntities);
        press(&mut menu, &model, size, Control::DeclineExperiments);
        assert_eq!((menu.tab, menu.experiments_prompt), (Tab::Helpers, false));
        // A saved "on" without consent stays visibly inactive and can still be switched off.
        model.show_invisible_entities = true;
        let rows = menu.layout(&model, size[0], size[1]);
        let hint_texts: Vec<_> = rows.iter().filter(|w| w.control.is_none()).collect();
        assert!(
            hint_texts.iter().any(|w| match language {
                Language::English => w.text.starts_with("Inactive without experimental consent"),
                Language::Russian => w.text.starts_with("Не работает без согласия"),
            }),
            "{:?}",
            hint_texts.iter().map(|w| &w.text).collect::<Vec<_>>()
        );
        assert_eq!(
            press(&mut menu, &model, size, Control::ShowInvisibleEntities),
            Some(Action::SetShowInvisibleEntities(false))
        );
        // The ban risk is on the switch itself, so compact menus without hints keep it.
        for compact in [[240.0, 216.0], [400.0, 280.0]] {
            let mut menu = menu_with(|m| {
                m.tab = Tab::Helpers;
                m.language = language;
            });
            let found = walk_helpers(&mut menu, &model, compact)
                .into_iter()
                .flat_map(|(_, _, ws)| ws)
                .find(|w| w.control == Some(Control::ShowInvisibleEntities))
                .unwrap();
            assert!(
                found.text.contains(match language {
                    Language::English => "ban risk",
                    Language::Russian => "риск бана",
                }),
                "{}",
                found.text
            );
        }
    }
    // Runtime: a persisted "true" never reveals without the consent or with the mod off.
    for setting in [None, Some("false"), Some("true")] {
        assert!(!claim_borders::visibility::enabled(true, false, setting));
        assert!(!claim_borders::visibility::enabled(false, true, setting));
    }
    assert!(claim_borders::visibility::enabled(true, true, Some("true")));
}

/// The key names Horizon's `keybinds::key_name` emits (crates/horizon-client-input, pin
/// 9f42064c): dotted, never underscored.
const PAGE_UP: &str = "key.keyboard.page.up";
const PAGE_DOWN: &str = "key.keyboard.page.down";

#[test]
fn physical_page_keys_reach_qol_and_food_picker_pagination_through_production_dispatch() {
    let model = ViewModel {
        auto_eat: true,
        auto_eat_options: (0..60)
            .map(|i| (format!("minecraft:f{i}"), format!("Food {i}"), 4))
            .collect(),
        ..ViewModel::default()
    };
    let size = [240.0, 216.0];
    let mut menu = menu_with(|m| {
        m.tab = Tab::Helpers;
        m.language = Language::English;
    });
    assert!(menu.helper_page_count(&model, 200.0) > 1);
    assert!(menu.dispatch_key(&model, size, PAGE_DOWN, 0));
    assert_eq!(menu.helper_page, 1, "Page Down must page the QOL rows");
    assert!(menu.dispatch_key(&model, size, PAGE_UP, 0));
    assert_eq!(menu.helper_page, 0, "Page Up must page back");
    click(&mut menu, &model, size, Control::AutoEatFoods);
    assert!(menu.dispatch_key(&model, size, PAGE_DOWN, 0));
    assert_eq!(
        menu.auto_eat_food_page, 1,
        "Page Down must page the food tiles"
    );
    assert!(menu.dispatch_key(&model, size, PAGE_UP, 0));
    assert_eq!(menu.auto_eat_food_page, 0);
    // Other keys are not mistaken for paging: Tab still walks categories from the list.
    click(&mut menu, &model, size, Control::AutoEatFoods);
    assert!(menu.dispatch_key(&model, size, "key.keyboard.tab", 0));
    assert_eq!(menu.helper_category, HelperCategory::Gear);
    // The old underscore spelling is not a key Horizon ever emits and must not page.
    menu.helper_page = 0;
    menu.dispatch_key(&model, size, "key.keyboard.page_down", 0);
    assert_eq!(menu.helper_page, 0);
}

#[test]
fn default_on_auto_eat_is_inactive_without_consent_but_foods_can_be_chosen_ahead() {
    use claim_borders::auto_eat as eat;
    // The real fresh-profile snapshot: neither "auto_eat" nor "experiments_accepted" is saved.
    let model = ViewModel {
        auto_eat: eat::enabled(None),
        experiments_accepted: eat::consented(None),
        auto_eat_options: vec![("minecraft:bread".into(), "Bread".into(), 5)],
        ..ViewModel::default()
    };
    assert!(model.auto_eat && !model.experiments_accepted);
    assert!(!eat::active(None, None), "the runtime gate stays shut");
    for language in [Language::English, Language::Russian] {
        let size = [800.0, 500.0];
        let mut menu = menu_with(|m| {
            m.tab = Tab::Helpers;
            m.language = language;
        });
        let walked = walk_helpers(&mut menu, &model, size);
        let survival = &walked[0].2;
        let hint = survival.iter().any(|w| {
            w.control.is_none()
                && w.text.starts_with(match language {
                    Language::English => "Won't eat until you accept",
                    Language::Russian => "Не ест без согласия",
                })
        });
        assert!(hint, "the switch must say it is inactive");
        // Choosing foods ahead of time works, and no prompt is forced for it.
        press(&mut menu, &model, size, Control::AutoEatFoods);
        assert_eq!(
            click(&mut menu, &model, size, Control::AutoEatFood(0)),
            Some(Action::SetAutoEatFood("minecraft:bread".into(), true))
        );
        assert!(!menu.experiments_prompt);
        click(&mut menu, &model, size, Control::AutoEatFoods);
        // It can be switched off, and switching it back on asks for the warning.
        assert_eq!(
            press(&mut menu, &model, size, Control::AutoEat),
            Some(Action::SetAutoEat(false))
        );
        let off = ViewModel {
            auto_eat: false,
            ..model.clone()
        };
        assert_eq!(press(&mut menu, &off, size, Control::AutoEat), None);
        assert!(menu.experiments_prompt);
    }
}

#[test]
fn upstream_automation_baritone_and_schematics_stay_where_they_were_and_apart_from_qol() {
    let model = ViewModel {
        experiments_accepted: true,
        baritone_enabled: true,
        ..ViewModel::default()
    };
    for language in [Language::Russian, Language::English] {
        for size in [[800.0_f32, 500.0], [240.0, 216.0]] {
            let has_rail = menu::rail_width(
                (size[0] - 16.0).clamp(220.0, 720.0),
                (size[1] - 16.0).clamp(200.0, 400.0),
            ) > 0.0;
            let mut menu = menu_with(|m| m.language = language);
            let open = |menu: &mut Menu, tab: Tab| {
                if !has_rail {
                    click(menu, &model, size, Control::NavigationMenu);
                }
                click(menu, &model, size, Control::Tab(tab))
            };
            // Experiments → Automation is still Farm and Drain, under its own name.
            open(&mut menu, Tab::Experimental);
            let widgets = menu.layout(&model, size[0], size[1]);
            let label = match language {
                Language::Russian => "Автоматизация",
                Language::English => "Automation",
            };
            assert!(widgets.iter().any(|w| {
                w.control == Some(Control::ExperimentalTab(ExperimentalSection::Baritone))
                    && w.text == label
            }));
            click(
                &mut menu,
                &model,
                size,
                Control::ExperimentalTab(ExperimentalSection::Baritone),
            );
            for mode in [BaritoneMode::Farm, BaritoneMode::Drain] {
                click(&mut menu, &model, size, Control::BaritoneMode(mode));
                assert_eq!(menu.baritone_mode, mode);
            }
            click(
                &mut menu,
                &model,
                size,
                Control::BaritoneMode(BaritoneMode::Farm),
            );
            for section in FarmSection::ALL {
                click(&mut menu, &model, size, Control::FarmSection(section));
                assert_eq!(menu.farm_section, section);
            }
            // Baritone and Schematics keep their own top-level tabs.
            open(&mut menu, Tab::Baritone);
            assert_eq!(menu.tab, Tab::Baritone);
            assert!(
                menu.layout(&model, size[0], size[1])
                    .iter()
                    .any(|w| w.control == Some(Control::BaritoneEnable))
            );
            let refresh = open(&mut menu, Tab::Schematics);
            assert_eq!(menu.tab, Tab::Schematics);
            assert_eq!(refresh, Some(Action::RefreshSchematics));
            // QOL hosts helpers only: no Farm, Drain or Baritone control is in it.
            open(&mut menu, Tab::Helpers);
            for (_, _, widgets) in walk_helpers(&mut menu, &model, size) {
                for w in widgets {
                    let control = w.control.unwrap_or(Control::Close);
                    assert!(
                        !upstream_automation_controls().contains(&control),
                        "{control:?} in QOL"
                    );
                    assert!(
                        !matches!(
                            control,
                            Control::BaritoneCmd(_) | Control::BaritoneSection(_)
                        ),
                        "{control:?} in QOL"
                    );
                }
            }
        }
    }
}

#[test]
fn saved_views_restore_the_new_tab_and_the_old_helpers_path() {
    // The current format keeps the QOL tab and its category.
    let mut kept = menu_with(|m| {
        m.tab = Tab::Helpers;
        m.helper_category = HelperCategory::Controls;
    });
    let text = kept.view_state();
    assert!(
        text.contains("tab=helpers") && text.contains("hc=controls"),
        "{text}"
    );
    let mut fresh = Menu::default();
    fresh.restore_view(&text);
    assert_eq!(
        (fresh.tab, fresh.helper_category),
        (Tab::Helpers, HelperCategory::Controls)
    );
    assert_eq!(fresh.view_state(), text);
    // What the published version saved for Experiments → Helpers opens the QOL tab.
    let mut old = Menu::default();
    old.restore_view("tab=experimental;exp=helpers;bs=tasks;bm=farm;fs=crops;set=0;chat=0;claims=0;party=1;trade=0");
    assert_eq!(old.tab, Tab::Helpers);
    // The other published views are untouched, including Automation (Farm, Drain).
    let mut automation = Menu::default();
    automation.restore_view("tab=experimental;exp=automation;bm=drain;fs=plots");
    assert_eq!(automation.tab, Tab::Experimental);
    assert_eq!(
        automation.experimental_section,
        ExperimentalSection::Baritone
    );
    assert_eq!(automation.baritone_mode, BaritoneMode::Drain);
    assert_eq!(automation.farm_section, FarmSection::Plots);
    for (saved, tab) in [
        ("tab=baritone", Tab::Baritone),
        ("tab=schematics", Tab::Schematics),
        ("tab=trading", Tab::Trading),
    ] {
        let mut menu = Menu::default();
        menu.restore_view(saved);
        assert_eq!(menu.tab, tab);
    }
    // A view saved without the new key, and unknown ones, still restore.
    let mut menu = Menu::default();
    menu.restore_view("tab=helpers;future=1;hc=nonsense");
    assert_eq!(
        (menu.tab, menu.helper_category),
        (Tab::Helpers, HelperCategory::Survival)
    );
    kept.helper_page = 0;
}
