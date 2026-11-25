// Image-to-block planning. The adapter supplies the active client's map colours.
use super::claim_borders_schematic::{BlockState, Blueprint, MAX_BLOCKS};
use std::collections::BTreeMap;

pub const DYES: [&str; 16] = [
    "white",
    "orange",
    "magenta",
    "light_blue",
    "yellow",
    "lime",
    "pink",
    "gray",
    "light_gray",
    "cyan",
    "purple",
    "blue",
    "brown",
    "green",
    "red",
    "black",
];
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub image: String,
    pub custom: String,
    pub floor: String,
    pub background: String,
    /// 0 carpets, 1 wool, 2 concrete, 3 custom.
    pub palette: u8,
    pub tiles: [u8; 2],
    /// 0 contain, 1 centred crop, 2 stretch.
    pub fit: u8,
    pub stepped: bool,
    pub height: u8,
    pub dither: bool,
}
impl Default for Request {
    fn default() -> Self {
        Self {
            image: String::new(),
            custom: "white_wool black_wool".into(),
            floor: "cobblestone".into(),
            background: "ffffff".into(),
            palette: 0,
            tiles: [1, 1],
            fit: 0,
            stepped: false,
            height: 16,
            dither: true,
        }
    }
}
impl Request {
    pub fn names(&self) -> Result<Vec<String>, String> {
        let names = if self.palette < 3 {
            let suffix = ["carpet", "wool", "concrete"][self.palette as usize];
            DYES.iter()
                .map(|dye| format!("minecraft:{dye}_{suffix}"))
                .collect()
        } else if self.palette == 3 {
            self.custom
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        } else {
            return Err("Invalid palette / Неверная палитра".into());
        };
        if names.is_empty() || names.len() > 64 {
            return Err("Choose 1–64 blocks / Выберите 1–64 блока".into());
        }
        names
            .iter()
            .map(|s| {
                let state = BlockState::parse(s)?;
                if !state.properties.is_empty() {
                    return Err(
                        "Use block names without properties / Укажите имена блоков без свойств"
                            .into(),
                    );
                }
                Ok(state.name)
            })
            .collect()
    }
    pub fn background_rgb(&self) -> Result<[u8; 3], String> {
        let s = self.background.trim().trim_start_matches('#');
        if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Background: RRGGBB / Фон: RRGGBB".into());
        }
        let n = u32::from_str_radix(s, 16).map_err(|e| e.to_string())?;
        Ok([(n >> 16) as u8, (n >> 8) as u8, n as u8])
    }
}
#[derive(Clone, Debug)]
pub struct Material {
    pub state: BlockState,
    /// Dark, normal, light, from the game's actual map palette.
    pub shades: [[u8; 3]; 3],
}
#[derive(Debug)]
pub struct Plan {
    pub request: Request,
    pub document: Blueprint,
    pub width: usize,
    pub depth: usize,
    pub height: i32,
    pub pixels: Vec<[u8; 3]>,
    pub materials: BTreeMap<String, usize>,
}
fn distance(a: [f32; 3], b: [u8; 3]) -> f32 {
    // Weighted RGB error; all comparisons use map pixels, never block textures.
    [0.2126, 0.7152, 0.0722]
        .into_iter()
        .enumerate()
        .map(|(i, weight)| weight * (a[i] - f32::from(b[i])).powi(2))
        .sum()
}
pub fn generate(
    request: Request,
    source_size: [usize; 2],
    rgba: &[u8],
    palette: &[Material],
    floor: BlockState,
) -> Result<Plan, String> {
    let [sw, sh] = source_size;
    if sw == 0
        || sh == 0
        || sw > 8192
        || sh > 8192
        || sw
            .checked_mul(sh)
            .is_none_or(|n| n > 16_777_216 || n.checked_mul(4) != Some(rgba.len()))
        || request.tiles.iter().any(|n| !(1..=2).contains(n))
        || !(2..=32).contains(&request.height)
        || request.fit > 2
        || palette.is_empty()
        || palette.len() > 64
        || floor.air()
    {
        return Err("Invalid image or settings / Неверная картинка или настройки".into());
    }
    let bg = request.background_rgb()?;
    let width = usize::from(request.tiles[0]) * 128;
    let depth = usize::from(request.tiles[1]) * 128;
    let mut samples = vec![[0.0; 3]; width * depth];
    let scale = if request.fit == 1 {
        (width as f64 / sw as f64).max(depth as f64 / sh as f64)
    } else {
        (width as f64 / sw as f64).min(depth as f64 / sh as f64)
    };
    for z in 0..depth {
        for x in 0..width {
            let (sx, sy) = if request.fit == 2 {
                (
                    (x as f64 + 0.5) * sw as f64 / width as f64 - 0.5,
                    (z as f64 + 0.5) * sh as f64 / depth as f64 - 0.5,
                )
            } else {
                (
                    (x as f64 + 0.5 - width as f64 / 2.0) / scale + sw as f64 / 2.0 - 0.5,
                    (z as f64 + 0.5 - depth as f64 / 2.0) / scale + sh as f64 / 2.0 - 0.5,
                )
            };
            let mut pixel = bg.map(f32::from);
            if sx >= -0.5 && sy >= -0.5 && sx < sw as f64 - 0.5 && sy < sh as f64 - 0.5 {
                // Bilinear resampling, composited before interpolation to avoid transparent fringes.
                pixel = [0.0; 3];
                let fx = sx - sx.floor();
                let fy = sy - sy.floor();
                for (dx, dy, weight) in [
                    (0, 0, (1.0 - fx) * (1.0 - fy)),
                    (1, 0, fx * (1.0 - fy)),
                    (0, 1, (1.0 - fx) * fy),
                    (1, 1, fx * fy),
                ] {
                    let xx = (sx.floor() as i32 + dx).clamp(0, sw as i32 - 1) as usize;
                    let yy = (sy.floor() as i32 + dy).clamp(0, sh as i32 - 1) as usize;
                    let p = &rgba[(yy * sw + xx) * 4..][..4];
                    let alpha = f32::from(p[3]) / 255.0;
                    for c in 0..3 {
                        pixel[c] += weight as f32
                            * (f32::from(p[c]) * alpha + f32::from(bg[c]) * (1.0 - alpha));
                    }
                }
            }
            samples[z * width + x] = pixel;
        }
    }
    let mut previous = vec![i32::from(request.height) / 2; width];
    let north = previous[0];
    let mut top = Vec::with_capacity(width * depth);
    let mut pixels = Vec::with_capacity(width * depth);
    for z in 0..depth {
        for (x, previous_y) in previous.iter_mut().enumerate() {
            let i = z * width + x;
            let wanted = samples[i].map(|v| v.clamp(0.0, 255.0));
            let mut best = (f32::INFINITY, 0, 1, *previous_y);
            for (m, material) in palette.iter().enumerate() {
                for (shade, delta) in [(0, -1), (1, 0), (2, 1)] {
                    if !request.stepped && delta != 0 {
                        continue;
                    }
                    let y = *previous_y + delta;
                    if y < 1 || y > i32::from(request.height) {
                        continue;
                    }
                    let error = distance(wanted, material.shades[shade]);
                    if error < best.0 {
                        best = (error, m, shade, y);
                    }
                }
            }
            let (_, m, shade, y) = best;
            *previous_y = y;
            let color = palette[m].shades[shade];
            pixels.push(color);
            top.push((x as i32, y, z as i32, m));
            if request.dither {
                let error = std::array::from_fn::<_, 3, _>(|c| wanted[c] - f32::from(color[c]));
                for (dx, dz, weight) in [
                    (1, 0, 7.0 / 16.0),
                    (-1, 1, 3.0 / 16.0),
                    (0, 1, 5.0 / 16.0),
                    (1, 1, 1.0 / 16.0),
                ] {
                    let xx = x as i32 + dx;
                    let zz = z as i32 + dz;
                    if xx >= 0 && xx < width as i32 && zz < depth as i32 {
                        for (c, e) in error.iter().enumerate() {
                            samples[zz as usize * width + xx as usize][c] += e * weight;
                        }
                    }
                }
            }
        }
    }
    // A flat build only needs one floor layer. Stepped builds include solid support columns.
    let min_y = top
        .iter()
        .map(|(_, y, _, _)| *y)
        .min()
        .unwrap_or(1)
        .min(north);
    let mut blocks = Vec::new();
    for x in 0..width as i32 {
        for y in 0..=north - min_y + 1 {
            blocks.push(([x, y, -1], floor.clone()));
        }
    }
    for (x, y, z, m) in top {
        let y = y - min_y + 1;
        if blocks.len() + y as usize + 1 > MAX_BLOCKS {
            return Err("Too many support blocks: reduce size/height / Слишком много опор: уменьшите размер/высоту".into());
        }
        for sy in 0..y {
            blocks.push(([x, sy, z], floor.clone()));
        }
        blocks.push(([x, y, z], palette[m].state.clone()));
    }
    let height = blocks.iter().map(|(p, _)| p[1]).max().unwrap_or(0) + 1;
    let document = Blueprint {
        blocks,
        data_version: 0,
        ignored_data: 0,
        format: "litematic",
    };
    let materials = document.materials();
    Ok(Plan {
        request,
        document,
        width,
        depth,
        height,
        pixels,
        materials,
    })
}

/// Split the northern guide row from the image, keeping every region within import bounds.
pub fn regions(plan: &Plan) -> Result<Vec<super::claim_borders_litematica_writer::Region>, String> {
    use super::claim_borders_litematica_writer::{Region, State};
    let mut regions = Vec::new();
    for tz in 0..plan.request.tiles[1] {
        for tx in 0..plan.request.tiles[0] {
            let z0 = i32::from(tz) * 128;
            let x0 = i32::from(tx) * 128;
            let start = if tz == 0 { -1 } else { z0 };
            let mut region = Region::new(
                &format!("Map art {}-{}", tx + 1, tz + 1),
                [x0, 0, start],
                [128, plan.height, z0 + 128 - start],
            )
            .ok_or("Invalid map-art region")?;
            for (p, state) in &plan.document.blocks {
                if p[0] >= x0 && p[0] < x0 + 128 && p[2] >= start && p[2] < z0 + 128 {
                    region.set(
                        p[0] - x0,
                        p[1],
                        p[2] - start,
                        &State {
                            name: state.name.clone(),
                            properties: state.properties.clone(),
                        },
                    );
                }
            }
            regions.push(region);
        }
    }
    Ok(regions)
}

#[derive(Clone, Debug, Default)]
pub struct View {
    pub status: String,
    pub busy: bool,
    pub plan: Option<std::sync::Arc<Plan>>,
}
