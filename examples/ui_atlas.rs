//! Secondary-screen layout atlas, not screenshots. No game/server data is loaded.
//! Uses production geometry and system fonts; native compositing and hit handling differ.
use claim_borders::{
    economy_ui::Panel,
    menu::Language,
    outgoing::{TranslationScreen, TranslationTargets},
    seed_map, ui_style as style, waypoints,
};
use std::{fmt::Write as _, path::PathBuf};

struct Svg {
    body: String,
    clip: usize,
}
impl Svg {
    fn rect(&mut self, r: [f32; 4], color: u32) {
        write!(
            self.body,
            "<rect x='{}' y='{}' width='{}' height='{}' rx='4' fill='#{color:06x}'/>",
            r[0], r[1], r[2], r[3]
        )
        .unwrap();
    }
    fn text(&mut self, x: f32, y: f32, text: &str, color: u32) {
        let text = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        write!(
            self.body,
            "<text x='{x}' y='{}' fill='#{color:06x}'>{text}</text>",
            y + 7.5
        )
        .unwrap();
    }
    fn button(&mut self, r: [f32; 4], label: &str, selected: bool) {
        self.rect(
            r,
            if selected {
                style::Accent::Mint.color()
            } else {
                style::BORDER
            },
        );
        self.rect(
            [r[0] + 0.75, r[1] + 0.75, r[2] - 1.5, r[3] - 1.5],
            style::mix(style::SURFACE, style::RAISED, 0.32),
        );
        let id = self.clip;
        self.clip += 1;
        write!(self.body,"<clipPath id='c{id}'><rect x='{}' y='{}' width='{}' height='{}'/></clipPath><g clip-path='url(#c{id})'>",r[0]+4.0,r[1],r[2]-8.0,r[3]).unwrap();
        let lines: Vec<_> = label.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            self.text(
                r[0] + 5.0,
                r[1] + (r[3] - lines.len() as f32 * 11.0) * 0.5 + index as f32 * 11.0,
                line,
                style::TEXT,
            );
        }
        self.body.push_str("</g>");
    }
    fn icon(&mut self, r: [f32; 4], icon: style::Icon) {
        write!(self.body,"<g transform='translate({}, {}) scale({})' fill='none' stroke='#afc0cd' stroke-width='1.1' stroke-linecap='round'>",r[0]+(r[2]-12.0)*0.5,r[1]+(r[3]-12.0)*0.5,1).unwrap();
        for [x1, y1, x2, y2] in style::strokes(icon) {
            write!(self.body, "<path d='M{x1} {y1}L{x2} {y2}'/>").unwrap();
        }
        self.body.push_str("</g>");
    }
    fn frame(&mut self, r: [f32; 4], title: &str) {
        self.rect(r, style::BACKGROUND);
        self.text(r[0] + 14.0, r[1] + 15.0, title, style::TEXT);
        self.rect([r[0] + 12.0, r[1] + 36.0, r[2] - 24.0, 1.0], style::BORDER);
    }
}
fn main() -> std::io::Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| "target/ui-preview".into());
    std::fs::create_dir_all(&root)?;
    let mut svg=Svg{body:"<svg xmlns='http://www.w3.org/2000/svg' width='3200' height='1280' viewBox='0 0 1600 640'><style>text{font:8px DejaVu Sans,sans-serif}</style>".into(),clip:0};
    let size = [400.0, 260.0];
    for (index, title) in [
        "Explored map tools — no terrain data",
        "Waypoint editor — example text",
        "Translation language editor",
        "Appearance — first settings page",
        "Inventory tools — Sorting category",
        "Trading — compact details geometry",
        "Server transaction confirmation — no transaction",
        "Seed-map structure filters — no terrain data",
    ]
    .into_iter()
    .enumerate()
    {
        write!(
            svg.body,
            "<g transform='translate({}, {})'>",
            (index % 4) * 400,
            (index / 4) * 310
        )
        .unwrap();
        svg.rect([0.0, 0.0, 400.0, 300.0], 0x0b1118);
        svg.text(8.0, 8.0, title, style::MUTED);
        svg.body.push_str("<g transform='translate(0,28)'>");
        match index {
            0 => {
                let map = seed_map::map_rect(size);
                svg.rect(map, style::SURFACE);
                svg.button([6.0, 4.0, 118.0, 22.0], "Explored", true);
                svg.button([128.0, 4.0, 80.0, 22.0], "Seed map", false);
                for (i, label) in ["Overworld", "Nether", "End"].into_iter().enumerate() {
                    svg.button(seed_map::dimension_rect(size, i), label, i == 0);
                }
                for (i, icon) in [
                    style::Icon::Plus,
                    style::Icon::Minus,
                    style::Icon::Home,
                    style::Icon::Close,
                    style::Icon::Shield,
                    style::Icon::Layers,
                    style::Icon::Grid,
                    style::Icon::People,
                    style::Icon::Settings,
                ]
                .into_iter()
                .enumerate()
                {
                    let r = seed_map::tool_rect(size, i);
                    svg.button(r, "", false);
                    svg.icon(r, icon);
                }
                svg.text(
                    104.0,
                    120.0,
                    "Terrain is rendered by the live client",
                    style::MUTED,
                );
                svg.text(
                    8.0,
                    240.0,
                    "Drag to pan · Wheel to zoom · Tooltips identify tools",
                    style::MUTED,
                );
            }
            1 => {
                let form = waypoints::EditorLayout::new(size);
                svg.frame(form.panel, "Edit waypoint");
                for (r, label, value) in [
                    (form.name, "Name", "Home"),
                    (form.kind, "Type", "Pin"),
                    (form.color, "Color (HEX)", "#A4D3C9"),
                ] {
                    svg.text(r[0], r[1] - 11.0, label, style::MUTED);
                    svg.button(r, value, false);
                }
                for (i, r) in form.coordinates.into_iter().enumerate() {
                    svg.text(r[0], r[1] - 11.0, ["X", "Y", "Z"][i], style::MUTED);
                    svg.button(r, "0", false);
                }
                for (r, color) in form.swatches.into_iter().zip([
                    0xffcc55, 0xa4d3c9, 0xa6c9e5, 0xbcb8e3, 0xdfa8b5, 0xa2d0b7, 0xffffff, 0xff8866,
                ]) {
                    svg.rect(r, color);
                }
                svg.button(form.cancel, "Cancel [Esc]", false);
                svg.button(form.save, "Save", true);
            }
            2 => {
                svg.frame([8.0, 4.0, 384.0, 252.0], "Translation languages");
                let menu =
                    TranslationScreen::new(TranslationTargets::load(None, None, Language::English));
                for widget in menu.layout(size, Language::English) {
                    svg.button(widget.rect, &widget.text, widget.selected);
                }
            }
            3 => {
                let form = style::DialogLayout::new(size, [360.0, 300.0]);
                svg.frame(form.panel, "Appearance");
                svg.text(
                    form.body[0],
                    form.body[1],
                    "Shared style across the mod",
                    style::MUTED,
                );
                for (i, label) in [
                    "Background opacity: 84%",
                    "Accent: Mint",
                    "Interface animations: on",
                    "Soft shadows: on",
                    "High contrast: off",
                    "Reset appearance",
                ]
                .into_iter()
                .take(form.capacity)
                .enumerate()
                {
                    svg.button(form.row(i), label, false);
                }
                svg.button(form.footer(0, true), "Back 1/2", false);
                svg.button(form.footer(1, true), "Next", false);
                svg.button(form.footer(0, false), "Cancel [Esc]", false);
                svg.button(form.footer(1, false), "Save", true);
            }
            4 => {
                let panel = style::PagedPanel::new(size).with_tabs();
                svg.rect(panel.rect, style::BACKGROUND);
                svg.button(
                    [4.0, 4.0, 220.0, 18.0],
                    "Inventory tools · collapse [Tab]",
                    false,
                );
                for (i, label) in ["Sorting", "Buttons", "Safety"].into_iter().enumerate() {
                    svg.button(
                        [10.0 + i as f32 * 208.0 / 3.0, 28.0, 208.0 / 3.0 - 3.0, 20.0],
                        label,
                        i == 0,
                    );
                }
                for (i, label) in [
                    "Area: inventory",
                    "Sorting: on",
                    "Sort by item ID",
                    "Order: rows",
                    "Clear exclusions (0)",
                ]
                .into_iter()
                .take(panel.capacity)
                .enumerate()
                {
                    svg.button(panel.row(i), label, false);
                }
                svg.button(panel.footer(0, true), "Back 1/1", false);
                svg.button(panel.footer(1, true), "Next", false);
                svg.button(panel.footer(0, false), "Cancel [Esc]", false);
                svg.button(panel.footer(1, false), "Save", true);
                svg.text(240.0, 120.0, "Inventory stays visible", style::MUTED);
            }
            5 => {
                let panel = Panel::new(size);
                svg.rect(panel.rect, style::BACKGROUND);
                let [x, y, w, h] = panel.rect;
                svg.button([x + 8.0, y + 5.0, 54.0, 18.0], "Back", false);
                svg.text(x + 72.0, y + 10.0, "Trading workspace", style::TEXT);
                for (i, label) in ["Overview", "Prices", "Auction", "Profit", "Plan"]
                    .into_iter()
                    .enumerate()
                {
                    svg.button(
                        [
                            x + 8.0 + i as f32 * (w - 16.0) / 5.0,
                            y + 30.0,
                            (w - 16.0) / 5.0 - 2.0,
                            18.0,
                        ],
                        label,
                        i == 1,
                    );
                }
                let [rx, ry, rw, _] = panel.details();
                svg.button([x + rx, y + 56.0, rw - 52.0, 18.0], "Back to list", false);
                svg.text(
                    x + rx,
                    y + ry + 4.0,
                    "Server-provided prices and item details",
                    style::TEXT,
                );
                svg.text(
                    x + rx,
                    y + ry + 22.0,
                    "Scrollable detail viewport — no sample prices",
                    style::MUTED,
                );
                svg.button(
                    [x + rx, y + h - 72.0, (rw - 4.0) * 0.5, 18.0],
                    "/worth price",
                    false,
                );
                svg.button(
                    [
                        x + rx + (rw + 4.0) * 0.5,
                        y + h - 72.0,
                        (rw - 4.0) * 0.5,
                        18.0,
                    ],
                    "Watch",
                    false,
                );
                for (i, label) in ["Market", "Find auction", "Scan market", "Scan auction"]
                    .into_iter()
                    .enumerate()
                {
                    svg.button(
                        [
                            x + 8.0 + i as f32 * (w - 20.0) / 4.0,
                            y + h - 48.0,
                            (w - 22.0) / 4.0,
                            18.0,
                        ],
                        label,
                        false,
                    );
                }
            }
            6 => {
                let form = style::DialogLayout::new(size, [440.0, 270.0]);
                svg.frame(form.panel, "Confirm action");
                svg.text(
                    form.body[0],
                    form.body[1],
                    "Server-provided item, quantity and total",
                    style::TEXT,
                );
                svg.text(
                    form.body[0],
                    form.body[1] + 24.0,
                    "Review the server details before confirming.",
                    style::MUTED,
                );
                svg.button(form.footer(0, false), "Cancel", false);
                svg.button(form.footer(1, false), "Buy", true);
            }
            _ => {
                let r = seed_map::filter_rect(size);
                svg.rect(r, style::BACKGROUND);
                svg.button(
                    [r[0] + 8.0, r[1] + 6.0, (r[2] - 44.0) * 0.5, 19.0],
                    "Structures",
                    true,
                );
                svg.button(
                    [
                        r[0] + 12.0 + (r[2] - 44.0) * 0.5,
                        r[1] + 6.0,
                        (r[2] - 44.0) * 0.5,
                        19.0,
                    ],
                    "Biomes",
                    false,
                );
                svg.button(
                    [r[0] + 8.0, r[1] + 30.0, r[2] - 16.0, 20.0],
                    "Search filters…",
                    false,
                );
                svg.button(
                    [r[0] + 8.0, r[1] + 56.0, r[2] - 16.0, 22.0],
                    "All structure groups",
                    true,
                );
                for (n, i) in seed_map::filter_indices(0, None, "")
                    .into_iter()
                    .take(seed_map::filter_rows(size) * 4)
                    .enumerate()
                {
                    svg.button(
                        seed_map::filter_chip_rect(size, n),
                        seed_map::STRUCTURES[i].label_en,
                        true,
                    );
                }
                svg.text(
                    r[0] + 8.0,
                    r[1] + r[3] - 20.0,
                    "Wheel: more filters",
                    style::MUTED,
                );
            }
        }
        svg.body.push_str("</g></g>");
    }
    svg.text(8.0,622.0,"OFFLINE LAYOUT ATLAS — production geometry, system fonts; not in-game screenshots or live transaction verification",style::MUTED);
    svg.body.push_str("</svg>");
    let path = root.join("secondary-atlas.svg");
    std::fs::write(&path, svg.body)?;
    println!("{}", path.display());
    Ok(())
}
