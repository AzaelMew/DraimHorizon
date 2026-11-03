// Shared presentation tokens, geometry, settings and timing. No game or GPU dependency.
use serde_json::{Value, json};
pub const SURFACE: u32 = 0x1d2a3a;
pub const RAISED: u32 = 0x2b3d50;
pub const TEXT: u32 = 0xe3ebf3;
pub const MUTED: u32 = 0x9aacc0;
pub const GOOD: u32 = 0xa2d0b7;
pub const WARN: u32 = 0xe1c69e;
pub const BAD: u32 = 0xdfa8b5;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    #[default]
    Mint,
    Lavender,
    Sky,
}
impl Accent {
    pub fn color(self) -> u32 {
        match self {
            Self::Mint => 0xa4d3c9,
            Self::Lavender => 0xbcb8e3,
            Self::Sky => 0xa6c9e5,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Mint => "Мята",
            Self::Lavender => "Лаванда",
            Self::Sky => "Небо",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Mint => Self::Lavender,
            Self::Lavender => Self::Sky,
            Self::Sky => Self::Mint,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub opacity: u8,
    pub motion: bool,
    pub shadows: bool,
    pub contrast: bool,
    pub accent: Accent,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            opacity: 84,
            motion: true,
            shadows: true,
            contrast: false,
            accent: Accent::Mint,
        }
    }
}
impl Preferences {
    pub fn encode(self) -> String {
        json!({"schema":1,"opacity":self.opacity,"motion":self.motion,"shadows":self.shadows,"contrast":self.contrast,
        "accent":match self.accent{Accent::Mint=>0,Accent::Lavender=>1,Accent::Sky=>2}}).to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 4096 {
            return Err("Файл оформления слишком большой".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if v["schema"].as_u64() != Some(1) {
            return Err("Неизвестная версия оформления".into());
        }
        Ok(Self {
            opacity: v["opacity"].as_u64().unwrap_or(84).clamp(50, 98) as u8,
            motion: v["motion"].as_bool().unwrap_or(true),
            shadows: v["shadows"].as_bool().unwrap_or(true),
            contrast: v["contrast"].as_bool().unwrap_or(false),
            accent: match v["accent"].as_u64() {
                Some(1) => Accent::Lavender,
                Some(2) => Accent::Sky,
                _ => Accent::Mint,
            },
        })
    }
    pub fn alpha(self) -> f32 {
        if self.contrast {
            0.96
        } else {
            f32::from(self.opacity) / 100.0
        }
    }
}
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = if t.is_finite() {
        t.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut color = 0;
    for shift in [0, 8, 16] {
        let x = ((a >> shift) & 255) as f32;
        let y = ((b >> shift) & 255) as f32;
        color |= ((x + (y - x) * t).round() as u32) << shift;
    }
    color
}
pub fn alpha(color: u32, opacity: f32) -> u32 {
    let t = if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (color & 0x00ff_ffff) | (((((color >> 24) & 255) as f32 * t).round() as u32) << 24)
}
/// Exponential response is independent of frame rate; disabling motion settles immediately.
pub fn approach(value: f32, target: f32, seconds: f32, motion: bool) -> f32 {
    if !motion || !value.is_finite() {
        return target.clamp(0.0, 1.0);
    }
    let dt = if seconds.is_finite() {
        seconds.clamp(0.0, 1.0)
    } else {
        0.0
    };
    (value + (target - value) * (1.0 - (-dt / 0.075).exp())).clamp(0.0, 1.0)
}
pub fn entrance(seconds: f32, motion: bool) -> f32 {
    if !motion {
        return 1.0;
    }
    let t = if seconds.is_finite() {
        (seconds / 0.16).clamp(0.0, 1.0)
    } else {
        1.0
    };
    1.0 - (1.0 - t).powi(3)
}
pub fn contains(r: [f32; 4], p: [f32; 2]) -> bool {
    r.iter().chain(&p).all(|n| n.is_finite())
        && r[2] > 0.0
        && r[3] > 0.0
        && p[0] >= r[0]
        && p[1] >= r[1]
        && p[0] < r[0] + r[2]
        && p[1] < r[1] + r[3]
}
/// Convex rounded perimeter, clamped to small controls so shapes never leave their hit box.
pub fn perimeter(r: [f32; 4], radius: f32) -> Vec<[f32; 2]> {
    if !r.iter().all(|v| v.is_finite()) || r[2] <= 0.0 || r[3] <= 0.0 {
        return Vec::new();
    }
    let radius = if radius.is_finite() {
        radius.max(0.0).min(r[2] * 0.5).min(r[3] * 0.5)
    } else {
        0.0
    };
    let mut points = Vec::with_capacity(20);
    for (cx, cy, base) in [
        (r[0] + radius, r[1] + radius, 180.0f32),
        (r[0] + r[2] - radius, r[1] + radius, 270.0),
        (r[0] + r[2] - radius, r[1] + r[3] - radius, 0.0),
        (r[0] + radius, r[1] + r[3] - radius, 90.0),
    ] {
        for i in 0..=4 {
            let angle = (base + i as f32 * 22.5).to_radians();
            points.push([cx + radius * angle.cos(), cy + radius * angle.sin()]);
        }
    }
    points
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Search,
    Portal,
    Chest,
    Star,
    Pin,
    People,
    Map,
    Chat,
    Market,
    Close,
    Settings,
    Plus,
    Minus,
    Back,
    Next,
    Check,
    Trash,
    Refresh,
    Sort,
    Fill,
    Restock,
    Transfer,
    Wallet,
    Clock,
    Signal,
    Eye,
    Home,
    Grid,
    Layers,
    Chart,
    Info,
    Shield,
    Save,
}
/// Original outline icons on a 12x12 grid; all strokes are atlas-independent.
pub fn strokes(icon: Icon) -> &'static [[f32; 4]] {
    use Icon::*;
    match icon {
        Search=>&[[2.,1.,6.,1.],[6.,1.,8.,3.],[8.,3.,8.,6.],[8.,6.,6.,8.],[6.,8.,3.,8.],[3.,8.,1.,6.],[1.,6.,1.,3.],[1.,3.,2.,1.],[8.,8.,11.,11.]],
        Portal=>&[[2.,11.,2.,1.],[2.,1.,10.,1.],[10.,1.,10.,11.],[4.,11.,4.,3.],[4.,3.,8.,3.],[8.,3.,8.,11.]],
        Chest=>&[[1.,3.,11.,3.],[11.,3.,11.,11.],[11.,11.,1.,11.],[1.,11.,1.,3.],[1.,6.,11.,6.],[5.,5.,7.,5.],[7.,5.,7.,8.],[7.,8.,5.,8.],[5.,8.,5.,5.]],
        Star=>&[[6.,0.,8.,4.],[8.,4.,12.,5.],[12.,5.,9.,8.],[9.,8.,10.,12.],[10.,12.,6.,10.],[6.,10.,2.,12.],[2.,12.,3.,8.],[3.,8.,0.,5.],[0.,5.,4.,4.],[4.,4.,6.,0.]],
        Pin => &[
            [6., 11., 6., 8.],
            [6., 8., 2., 4.],
            [2., 4., 3., 1.],
            [3., 1., 9., 1.],
            [9., 1., 10., 4.],
            [10., 4., 6., 8.],
            [5., 3., 7., 3.],
        ],
        People => &[
            [2., 2., 5., 2.],
            [5., 2., 5., 5.],
            [5., 5., 2., 5.],
            [2., 5., 2., 2.],
            [1., 10., 1., 7.],
            [1., 7., 6., 7.],
            [6., 7., 6., 10.],
            [8., 3., 10., 3.],
            [10., 3., 10., 6.],
            [8., 8., 11., 8.],
            [11., 8., 11., 10.],
        ],
        Map => &[
            [1., 2., 4., 1.],
            [4., 1., 8., 3.],
            [8., 3., 11., 2.],
            [11., 2., 11., 10.],
            [11., 10., 8., 11.],
            [8., 11., 4., 9.],
            [4., 9., 1., 10.],
            [1., 10., 1., 2.],
            [4., 1., 4., 9.],
            [8., 3., 8., 11.],
        ],
        Chat => &[
            [1., 1., 11., 1.],
            [11., 1., 11., 8.],
            [11., 8., 5., 8.],
            [5., 8., 2., 11.],
            [2., 11., 2., 8.],
            [2., 8., 1., 8.],
            [1., 8., 1., 1.],
            [3., 4., 9., 4.],
        ],
        Market => &[
            [1., 4., 11., 4.],
            [1., 4., 3., 1.],
            [3., 1., 9., 1.],
            [9., 1., 11., 4.],
            [2., 5., 2., 11.],
            [2., 11., 10., 11.],
            [10., 11., 10., 5.],
            [5., 11., 5., 7.],
            [5., 7., 8., 7.],
            [8., 7., 8., 11.],
        ],
        Close => &[[2., 2., 10., 10.], [10., 2., 2., 10.]],
        Settings => &[
            [4., 1., 8., 1.],
            [8., 1., 11., 4.],
            [11., 4., 11., 8.],
            [11., 8., 8., 11.],
            [8., 11., 4., 11.],
            [4., 11., 1., 8.],
            [1., 8., 1., 4.],
            [1., 4., 4., 1.],
            [4., 4., 8., 4.],
            [8., 4., 8., 8.],
            [8., 8., 4., 8.],
            [4., 8., 4., 4.],
        ],
        Plus => &[[6., 1., 6., 11.], [1., 6., 11., 6.]],
        Minus => &[[1., 6., 11., 6.]],
        Back => &[[8., 2., 4., 6.], [4., 6., 8., 10.]],
        Next => &[[4., 2., 8., 6.], [8., 6., 4., 10.]],
        Check => &[[1., 6., 4., 9.], [4., 9., 11., 2.]],
        Trash => &[
            [2., 3., 10., 3.],
            [4., 1., 8., 1.],
            [3., 4., 3., 11.],
            [3., 11., 9., 11.],
            [9., 11., 9., 4.],
            [5., 5., 5., 9.],
            [7., 5., 7., 9.],
        ],
        Refresh => &[
            [10., 4., 8., 1.],
            [8., 1., 3., 1.],
            [3., 1., 1., 4.],
            [1., 4., 1., 8.],
            [1., 8., 4., 11.],
            [4., 11., 9., 10.],
            [9., 10., 11., 7.],
            [10., 1., 10., 4.],
            [10., 4., 7., 4.],
        ],
        Sort => &[
            [2., 1., 2., 10.],
            [0., 8., 2., 10.],
            [2., 10., 4., 8.],
            [6., 2., 11., 2.],
            [6., 5., 10., 5.],
            [6., 8., 8., 8.],
        ],
        Fill => &[
            [1., 5., 1., 11.],
            [1., 11., 11., 11.],
            [11., 11., 11., 5.],
            [6., 0., 6., 8.],
            [3., 5., 6., 8.],
            [6., 8., 9., 5.],
        ],
        Restock => &[
            [1., 4., 1., 11.],
            [1., 11., 11., 11.],
            [11., 11., 11., 4.],
            [3., 5., 5., 7.],
            [5., 7., 9., 2.],
            [4., 1., 8., 1.],
        ],
        Transfer => &[
            [1., 4., 10., 4.],
            [8., 1., 11., 4.],
            [11., 4., 8., 7.],
            [11., 9., 2., 9.],
            [4., 6., 1., 9.],
            [1., 9., 4., 12.],
        ],
        Wallet => &[
            [1., 2., 11., 2.],
            [11., 2., 11., 10.],
            [11., 10., 1., 10.],
            [1., 10., 1., 2.],
            [11., 5., 7., 5.],
            [7., 5., 7., 8.],
            [7., 8., 11., 8.],
        ],
        Clock => &[
            [3., 1., 9., 1.],
            [9., 1., 11., 4.],
            [11., 4., 11., 9.],
            [11., 9., 8., 11.],
            [8., 11., 3., 11.],
            [3., 11., 1., 8.],
            [1., 8., 1., 3.],
            [1., 3., 3., 1.],
            [6., 3., 6., 6.],
            [6., 6., 9., 7.],
        ],
        Signal => &[
            [1., 9., 1., 11.],
            [4., 7., 4., 11.],
            [7., 4., 7., 11.],
            [10., 1., 10., 11.],
        ],
        Eye => &[
            [1., 6., 4., 2.],
            [4., 2., 8., 2.],
            [8., 2., 11., 6.],
            [11., 6., 8., 10.],
            [8., 10., 4., 10.],
            [4., 10., 1., 6.],
            [5., 5., 7., 5.],
            [7., 5., 7., 7.],
            [7., 7., 5., 7.],
            [5., 7., 5., 5.],
        ],
        Home => &[
            [1., 5., 6., 1.],
            [6., 1., 11., 5.],
            [2., 5., 2., 11.],
            [2., 11., 10., 11.],
            [10., 11., 10., 5.],
            [5., 11., 5., 7.],
            [5., 7., 7., 7.],
            [7., 7., 7., 11.],
        ],
        Grid => &[
            [1., 1., 11., 1.],
            [11., 1., 11., 11.],
            [11., 11., 1., 11.],
            [1., 11., 1., 1.],
            [6., 1., 6., 11.],
            [1., 6., 11., 6.],
        ],
        Layers => &[
            [1., 4., 6., 1.],
            [6., 1., 11., 4.],
            [11., 4., 6., 7.],
            [6., 7., 1., 4.],
            [1., 7., 6., 10.],
            [6., 10., 11., 7.],
        ],
        Chart => &[
            [1., 1., 1., 11.],
            [1., 11., 11., 11.],
            [3., 8., 6., 5.],
            [6., 5., 8., 7.],
            [8., 7., 11., 2.],
        ],
        Info => &[[6., 1., 6., 2.], [6., 5., 6., 11.]],
        Shield => &[
            [1., 1., 6., 3.],
            [6., 3., 11., 1.],
            [11., 1., 10., 8.],
            [10., 8., 6., 11.],
            [6., 11., 2., 8.],
            [2., 8., 1., 1.],
            [4., 6., 6., 8.],
            [6., 8., 9., 4.],
        ],
        Save => &[
            [1., 1., 10., 1.],
            [10., 1., 11., 3.],
            [11., 3., 11., 11.],
            [11., 11., 1., 11.],
            [1., 11., 1., 1.],
            [4., 1., 4., 5.],
            [4., 5., 8., 5.],
            [8., 5., 8., 1.],
            [4., 11., 4., 8.],
            [4., 8., 8., 8.],
            [8., 8., 8., 11.],
        ],
    }
}
pub fn icon_for(label: &str) -> Option<Icon> {
    use Icon::*;
    match label {
        "Метки" => Some(Pin),
        "Пати" | "Друзья" => Some(People),
        "Клеймы" | "Приваты" => Some(Shield),
        "Чат" => Some(Chat),
        "Торги" | "AH" | "Аукц." => Some(Market),
        "Цены" => Some(Wallet),
        "Обзор" | "Профит" | "Прибыль" => Some(Chart),
        "Опции" | "Настройки" | "E" => Some(Settings),
        "X" | "Закрыть" | "Отклонить" | "Отмена [Esc]" => Some(Close),
        "+" => Some(Plus),
        "−" | "-" => Some(Minus),
        "<" | "Назад" => Some(Back),
        ">" => Some(Next),
        "S" => Some(Sort),
        "F" => Some(Fill),
        "R" => Some(Restock),
        "T" => Some(Transfer),
        "Принять" | "Готово" => Some(Check),
        "Удалить" => Some(Trash),
        "Обновить" => Some(Refresh),
        "К себе" => Some(Home),
        "Сетка" => Some(Grid),
        "Рельеф" => Some(Layers),
        "Зоны" | "Мир" => Some(Map),
        _ if label.starts_with("Высокий контраст") => Some(Eye),
        _ if label.starts_with("Сохранить") => Some(Save),
        _ if label.starts_with("Добавить") => Some(Plus),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn colors_keep_alpha_and_channels_separate() {
        assert_eq!(mix(0, 0xff_ff_ff, 0.5), 0x80_80_80);
        assert_eq!(alpha(0x80_12_34_56, 0.5), 0x40_12_34_56);
        assert_eq!(alpha(0xff_12_34_56, f32::NAN), 0x00_12_34_56);
    }
    #[test]
    fn frame_rate_does_not_change_hover_timing_and_reduced_motion_is_immediate() {
        let mut a = 0.0;
        let mut b = 0.0;
        for _ in 0..30 {
            a = approach(a, 1.0, 1.0 / 30.0, true);
        }
        for _ in 0..144 {
            b = approach(b, 1.0, 1.0 / 144.0, true);
        }
        assert!((a - b).abs() < 0.0001);
        assert_eq!(approach(0.0, 1.0, 0.0, false), 1.0);
        assert_eq!(entrance(0.0, false), 1.0);
        assert_eq!(entrance(0.16, true), 1.0);
        assert!(entrance(0.08, true) > entrance(0.04, true));
    }
    #[test]
    fn rounded_geometry_stays_inside_controls_at_all_supported_sizes() {
        for w in [1., 8., 15., 68., 280.] {
            for h in [1., 12., 18., 220.] {
                let r = [-6., 9., w, h];
                let points = perimeter(r, 8.);
                assert_eq!(points.len(), 20);
                for p in points {
                    assert!(
                        p[0] >= r[0] - 0.001
                            && p[0] <= r[0] + w + 0.001
                            && p[1] >= r[1] - 0.001
                            && p[1] <= r[1] + h + 0.001
                    );
                }
            }
        }
        assert!(perimeter([0., 0., f32::NAN, 3.], 4.).is_empty());
        assert!(!contains([0., 0., 12., 12.], [f32::NAN, 4.]));
    }
    #[test]
    fn settings_roundtrip_and_corrupt_files_are_not_treated_as_defaults() {
        let p = Preferences {
            opacity: 70,
            motion: false,
            accent: Accent::Sky,
            ..Preferences::default()
        };
        assert_eq!(Preferences::decode(&p.encode()).unwrap(), p);
        assert!(Preferences::decode("{}").is_err());
        assert!(Preferences::decode(r#"{"schema":99}"#).is_err());
        assert_eq!(
            Preferences::decode(r#"{"schema":1,"opacity":999}"#)
                .unwrap()
                .opacity,
            98
        );
    }
    #[test]
    fn icons_fit_their_grid_and_exist_for_the_inventory_controls() {
        for icon in [
            Icon::Pin,
            Icon::People,
            Icon::Map,
            Icon::Chat,
            Icon::Market,
            Icon::Close,
            Icon::Settings,
            Icon::Plus,
            Icon::Minus,
            Icon::Back,
            Icon::Next,
            Icon::Check,
            Icon::Trash,
            Icon::Refresh,
            Icon::Sort,
            Icon::Fill,
            Icon::Restock,
            Icon::Transfer,
            Icon::Wallet,
            Icon::Clock,
            Icon::Signal,
            Icon::Eye,
            Icon::Home,
            Icon::Grid,
            Icon::Layers,
            Icon::Chart,
            Icon::Info,
            Icon::Shield,
            Icon::Save,
        ] {
            assert!(!strokes(icon).is_empty());
            assert!(
                strokes(icon)
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite() && *v >= 0.0 && *v <= 12.0)
            );
        }
        for label in ["S", "F", "R", "T", "E"] {
            assert!(icon_for(label).is_some());
        }
    }
}
