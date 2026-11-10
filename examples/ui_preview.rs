//! Offline layout preview, not an in-game screenshot. Uses production geometry, labels,
//! palette and Lucide paths; system font metrics and compositing differ from Horizon.
//! Run: cargo run --locked --example ui_preview -- target/ui-preview
use claim_borders::{menu, ui_style as style};
use std::{fmt::Write as _, path::PathBuf};

fn rect(svg: &mut String, r: [f32; 4], radius: f32, color: u32) {
    let [x, y, w, h] = r;
    write!(
        svg,
        "<rect x='{x}' y='{y}' width='{w}' height='{h}' rx='{radius}' fill='#{color:06x}'/>"
    )
    .unwrap();
}
fn text(svg: &mut String, at: [f32; 2], value: &str, color: u32) {
    let value = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    write!(
        svg,
        "<text x='{}' y='{}' fill='#{color:06x}'>{value}</text>",
        at[0],
        at[1] + 7.5
    )
    .unwrap();
}
fn preview(menu: &menu::Menu, size: [f32; 2]) -> String {
    let [width, height] = size;
    let model = menu::ViewModel {
        language: menu.language,
        auto_eat: true,
        navigation_status: menu
            .language
            .text(
                "Нет активной задачи · нажмите клавишу старта",
                "No task running · press the start key",
            )
            .into(),
        hotkeys: ["KEYPAD 1", "KEYPAD 2", "KEYPAD 3", "KEYPAD 4"].map(str::to_owned),
        ..menu::ViewModel::default()
    };
    let pw = (width - 16.0).clamp(220.0, 720.0);
    let ph = (height - 16.0).clamp(200.0, 400.0);
    let x = (width - pw) * 0.5;
    let y = (height - ph) * 0.5;
    let rail = menu::rail_width(pw, ph);
    let accent = style::Accent::Mint.color();
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='0 0 {width} {height}'><style>text{{font:8px 'DejaVu Sans',sans-serif}}</style>",
        width * 2.0,
        height * 2.0
    );
    rect(&mut svg, [0.0, 0.0, width, height], 0.0, 0x0b1118);
    rect(&mut svg, [x, y, pw, ph], 8.0, style::BORDER);
    rect(
        &mut svg,
        [x + 1.0, y + 1.0, pw - 2.0, ph - 2.0],
        7.0,
        style::BACKGROUND,
    );
    if rail > 0.0 {
        rect(
            &mut svg,
            [x + 4.0, y + 4.0, rail - 4.0, ph - 8.0],
            6.0,
            style::SURFACE,
        );
        text(&mut svg, [x + 10.0, y + 17.0], "DraimHorizon", style::TEXT);
        text(
            &mut svg,
            [x + 10.0, y + 30.0],
            menu.language.text("Инструменты клиента", "Client tools"),
            style::MUTED,
        );
    }
    for (index, widget) in menu.layout(&model, width, height).iter().enumerate() {
        let [x, y, w, h] = widget.rect;
        if widget.readout {
            rect(&mut svg, widget.rect, 3.0, style::SURFACE);
            svg.push_str("<g text-anchor='middle'>");
            text(
                &mut svg,
                [x + w * 0.5, y + (h - 8.0) * 0.5],
                &widget.text,
                style::TEXT,
            );
            svg.push_str("</g>");
            continue;
        }
        if widget.control.is_some() {
            rect(
                &mut svg,
                widget.rect,
                4.0,
                if widget.selected {
                    accent
                } else {
                    style::BORDER
                },
            );
            let base = style::mix(style::SURFACE, style::RAISED, 0.32);
            rect(
                &mut svg,
                [x + 0.75, y + 0.75, w - 1.5, h - 1.5],
                3.25,
                if widget.selected {
                    style::mix(base, accent, 0.10)
                } else {
                    base
                },
            );
            if widget.selected {
                rect(
                    &mut svg,
                    [x + 1.0, y + 4.0, 2.0, (h - 8.0).max(2.0)],
                    1.0,
                    accent,
                );
            }
        }
        let mut label = widget.text.as_str();
        let mut left = x + 4.0;
        let mut room = w - 8.0;
        if label == "DraimHorizon" {
            label = menu.tab.label(menu.language);
        }
        if let Some((name, on)) = style::switch_label(label).filter(|_| w >= 120.0) {
            label = name;
            room = w - if w >= 180.0 { 76.0 } else { 48.0 };
            let ty = y + (h - 12.0) * 0.5;
            rect(
                &mut svg,
                [x + w - 32.0, ty, 24.0, 12.0],
                6.0,
                if on { accent } else { 0x3a4757 },
            );
            rect(
                &mut svg,
                [x + w - if on { 18.0 } else { 30.0 }, ty + 2.0, 8.0, 8.0],
                4.0,
                if on { 0xf2f7fb } else { 0x93a1b1 },
            );
            if w >= 180.0 {
                let state = if on {
                    menu.language.text("Вкл", "On")
                } else {
                    menu.language.text("Выкл", "Off")
                };
                write!(svg, "<g text-anchor='end'>").unwrap();
                text(
                    &mut svg,
                    [x + w - 40.0, y + (h - 8.0) * 0.5],
                    state,
                    style::MUTED,
                );
                svg.push_str("</g>");
            }
        } else if widget.control.is_some()
            && let Some(icon) = style::icon_for(label)
        {
            // The preview shares the official Lucide paths, not approximate glyphs.
            if w > label.chars().count() as f32 * 5.0 + 24.0 {
                write!(svg, "<g transform='translate({}, {}) scale(0.8333)' stroke='#{accent:06x}' stroke-width='1.04' stroke-linecap='round'>", x + 6.0, y + (h - 10.0) * 0.5).unwrap();
                for [x1, y1, x2, y2] in style::strokes(icon) {
                    write!(svg, "<path d='M{x1} {y1} L{x2} {y2}'/>").unwrap();
                }
                svg.push_str("</g>");
                left += 17.0;
                room -= 17.0;
            }
        }
        write!(svg, "<clipPath id='c{index}'><rect x='{left}' y='{y}' width='{}' height='{h}'/></clipPath><g clip-path='url(#c{index})'>", room.max(0.0)).unwrap();
        text(
            &mut svg,
            [left, y + (h - 8.0) * 0.5],
            label,
            if widget.muted {
                style::MUTED
            } else {
                style::TEXT
            },
        );
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    svg
}
fn main() -> std::io::Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/ui-preview".into());
    std::fs::create_dir_all(&root)?;
    for (locale, language) in [
        ("ru", menu::Language::Russian),
        ("en", menu::Language::English),
    ] {
        for (name, tab, section, experimental_section, size) in [
            (
                "mining-wide",
                menu::Tab::Experimental,
                0,
                menu::ExperimentalSection::Baritone,
                [800.0, 500.0],
            ),
            (
                "mining-compact",
                menu::Tab::Experimental,
                0,
                menu::ExperimentalSection::Baritone,
                [240.0, 216.0],
            ),
            (
                "trading",
                menu::Tab::Trading,
                4,
                menu::ExperimentalSection::Ores,
                [800.0, 500.0],
            ),
            (
                "players",
                menu::Tab::Settings,
                5,
                menu::ExperimentalSection::Ores,
                [800.0, 500.0],
            ),
            (
                "overlay",
                menu::Tab::Settings,
                6,
                menu::ExperimentalSection::Ores,
                [800.0, 500.0],
            ),
            (
                "general",
                menu::Tab::Settings,
                0,
                menu::ExperimentalSection::Ores,
                [800.0, 500.0],
            ),
            (
                "chat",
                menu::Tab::Settings,
                3,
                menu::ExperimentalSection::Ores,
                [800.0, 500.0],
            ),
            (
                "helpers",
                menu::Tab::Experimental,
                0,
                menu::ExperimentalSection::Helpers,
                [800.0, 500.0],
            ),
            (
                "compact",
                menu::Tab::Settings,
                3,
                menu::ExperimentalSection::Ores,
                [240.0, 216.0],
            ),
        ] {
            let mut menu = menu::Menu::default();
            menu.language = language;
            menu.tab = tab;
            menu.settings_section = section;
            menu.experimental_section = experimental_section;
            if name.starts_with("mining-") {
                menu.baritone_mode = menu::BaritoneMode::Mining;
            }
            let file = root.join(format!("{name}-{locale}.svg"));
            std::fs::write(&file, preview(&menu, size))?;
            println!("{}", file.display());
        }
    }
    Ok(())
}
