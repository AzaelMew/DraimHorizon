use claim_borders::{
    economy,
    economy_ui::{DetailScroll, Panel},
    menu::{
        Action, BaritoneMode, Control, ExperimentalSection, Helper, Language, Menu, Tab, ViewModel,
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
                mining_custom: vec!["minecraft:prismarine".into()],
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
                for mode in [BaritoneMode::Mining, BaritoneMode::Schematic] {
                    for page in 0..4 {
                        let mut menu = Menu::default();
                        menu.language = language;
                        menu.tab = Tab::Experimental;
                        menu.experimental_section = ExperimentalSection::Baritone;
                        menu.baritone_mode = mode;
                        menu.mining_page = page;
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
                                    Some(
                                        Control::AutoEat
                                            | Control::MiningCustom
                                            | Control::FarmStorage(_)
                                            | Control::SchematicMaterials
                                            | Control::SchematicFolder
                                    )
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
                                        "footer {:?} {:?} overlaps {:?} {:?} at {size:?}, {mode:?}, {language:?}",
                                        widget.control,
                                        widget.text,
                                        other.control,
                                        other.text
                                    );
                                }
                            }
                        }
                        assert_eq!(
                            footer_count, 5,
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

#[test]
fn helpers_stay_readable_and_all_controls_remain_reachable_on_small_guis() {
    let model = ViewModel::default();
    for size in [[240.0, 216.0], [400.0, 280.0], [800.0, 500.0]] {
        let mut menu = Menu::default();
        menu.tab = Tab::Experimental;
        menu.experimental_section = ExperimentalSection::Helpers;
        let mut found = Vec::new();
        loop {
            let widgets = menu.layout(&model, size[0], size[1]);
            let controls: Vec<_> = widgets.iter().filter(|w| w.control.is_some()).collect();
            for (i, widget) in controls.iter().enumerate() {
                bounded(widget.rect, size);
                for other in &controls[i + 1..] {
                    assert!(separate(widget.rect, other.rect));
                }
                found.push(widget.control.unwrap());
            }
            let next = widgets
                .iter()
                .filter_map(|w| match w.control {
                    Some(Control::HelperPage(page)) if page > menu.helper_page => Some(page),
                    _ => None,
                })
                .max();
            let Some(next) = next else { break };
            click(&mut menu, &model, size, Control::HelperPage(next));
        }
        for control in [
            Control::AutoEat,
            Control::AutoClickButton,
            Control::AutoClickRate,
        ]
        .into_iter()
        .chain(Helper::ALL.into_iter().map(Control::Helper))
        {
            assert!(found.contains(&control), "lost {control:?}");
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
