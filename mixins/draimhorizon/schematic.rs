// Bounded NBT schematic import. Format references are documented in docs/automation.md.
use super::claim_borders_navigation::Pos;
use std::collections::BTreeMap;
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
/// The largest file read: 256 blocks to a side, at most 4 194 304 cells (256 × 64 × 256) and
/// 262 144 blocks that are not air. Real Litematica builds were refused under the earlier limits
/// (128 to a side, 64³ cells, 32 768 blocks) and their preview never showed.
pub const MAX_SIDE: u32 = 256;
pub const MAX_VOLUME: usize = 4_194_304;
pub const MAX_BLOCKS: usize = 262_144;
pub type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockState {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}
impl BlockState {
    pub fn parse(text: &str) -> Result<Self> {
        if text.len() > 1024 {
            return Err("block state too long".into());
        }
        let (name, props) = text.split_once('[').map_or((text, ""), |(n, p)| (n, p));
        let name = if name.contains(':') {
            name.to_owned()
        } else {
            format!("minecraft:{name}")
        };
        if !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_:./-".contains(&b))
            || name.split(':').count() != 2
            || name.split(':').any(str::is_empty)
        {
            return Err("invalid block name".into());
        }
        let mut properties = BTreeMap::new();
        if !props.is_empty() {
            let props = props.strip_suffix(']').ok_or("invalid block properties")?;
            for pair in props.split(',') {
                let (key, value) = pair.split_once('=').ok_or("invalid block property")?;
                if key.is_empty()
                    || value.is_empty()
                    || !key
                        .bytes()
                        .chain(value.bytes())
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                    || properties.insert(key.into(), value.into()).is_some()
                {
                    return Err("invalid or duplicate block property".into());
                }
            }
        }
        if properties.len() > 32 {
            return Err("too many block properties".into());
        }
        Ok(Self { name, properties })
    }
    pub fn air(&self) -> bool {
        matches!(
            self.name.as_str(),
            "minecraft:air"
                | "minecraft:cave_air"
                | "minecraft:void_air"
                | "minecraft:structure_void"
        )
    }
    pub fn rotated(&self, turns: u8) -> Self {
        let mut state = self.clone();
        let turns = turns % 4;
        let dirs = ["north", "east", "south", "west"];
        if let Some(facing) = self.properties.get("facing")
            && let Some(i) = dirs.iter().position(|d| *d == facing)
        {
            state
                .properties
                .insert("facing".into(), dirs[(i + usize::from(turns)) % 4].into());
        }
        if turns % 2 == 1
            && let Some(axis) = self.properties.get("axis")
        {
            if axis == "x" {
                state.properties.insert("axis".into(), "z".into());
            } else if axis == "z" {
                state.properties.insert("axis".into(), "x".into());
            }
        }
        if let Some(rotation) = self
            .properties
            .get("rotation")
            .and_then(|v| v.parse::<u8>().ok())
        {
            state
                .properties
                .insert("rotation".into(), ((rotation + turns * 4) % 16).to_string());
        }
        for key in dirs {
            state.properties.remove(key);
        }
        if self.name.ends_with("rail")
            && let Some(shape) = self.properties.get("shape")
        {
            let mut shape = shape.clone();
            for _ in 0..turns {
                shape = match shape.as_str() {
                    "north_south" => "east_west",
                    "east_west" => "north_south",
                    "ascending_north" => "ascending_east",
                    "ascending_east" => "ascending_south",
                    "ascending_south" => "ascending_west",
                    "ascending_west" => "ascending_north",
                    "north_east" => "south_east",
                    "south_east" => "south_west",
                    "south_west" => "north_west",
                    "north_west" => "north_east",
                    _ => shape.as_str(),
                }
                .into();
            }
            state.properties.insert("shape".into(), shape);
        }
        for (i, key) in dirs.iter().enumerate() {
            if let Some(value) = self.properties.get(*key) {
                state
                    .properties
                    .insert(dirs[(i + usize::from(turns)) % 4].into(), value.clone());
            }
        }
        state
    }
    /// The state mirrored the way the game mirrors a block: `1` across the Z axis (north and
    /// south change places, `Mirror.LEFT_RIGHT`), `2` across the X axis (east and west,
    /// `Mirror.FRONT_BACK`); `0` leaves it as it is.
    pub fn mirrored(&self, mirror: u8) -> Self {
        let mut state = self.clone();
        if mirror == 0 || mirror > 2 {
            return state;
        }
        let across_z = mirror == 1;
        let flip = |d: &str| -> String {
            match (across_z, d) {
                (true, "north") => "south",
                (true, "south") => "north",
                (false, "east") => "west",
                (false, "west") => "east",
                _ => d,
            }
            .to_owned()
        };
        let swap = |a: &str, b: &str, state: &mut Self| {
            let (x, y) = (state.properties.remove(a), state.properties.remove(b));
            if let Some(v) = y {
                state.properties.insert(a.into(), v);
            }
            if let Some(v) = x {
                state.properties.insert(b.into(), v);
            }
        };
        let facing = self.properties.get("facing").cloned();
        let facing_on_axis = facing.as_deref().is_some_and(|f| flip(f) != f);
        if let Some(f) = &facing {
            state.properties.insert("facing".into(), flip(f));
        }
        if let Some(r) = self
            .properties
            .get("rotation")
            .and_then(|v| v.parse::<i32>().ok())
        {
            let r = if across_z { 8 - r } else { 16 - r };
            state
                .properties
                .insert("rotation".into(), r.rem_euclid(16).to_string());
        }
        if across_z {
            swap("north", "south", &mut state);
        } else {
            swap("east", "west", &mut state);
        }
        let opposite = |v: &str, a: &str, b: &str| -> Option<String> {
            if v == a {
                Some(b.into())
            } else if v == b {
                Some(a.into())
            } else {
                None
            }
        };
        if self.name.ends_with("_stairs")
            && let Some(shape) = self.properties.get("shape")
        {
            // Across the mirror a corner swaps its side (its facing was flipped above, as the
            // game turns a straight stair round); a stair along the mirror is left alone.
            if facing_on_axis
                && let Some(shape) = opposite(shape, "inner_left", "inner_right")
                    .or_else(|| opposite(shape, "outer_left", "outer_right"))
            {
                state.properties.insert("shape".into(), shape);
            }
        }
        if self.name.ends_with("_door")
            && let Some(hinge) = self.properties.get("hinge")
            && let Some(h) = opposite(hinge, "left", "right")
        {
            state.properties.insert("hinge".into(), h);
        }
        if self.name.ends_with("chest")
            && let Some(kind) = self.properties.get("type")
            && let Some(k) = opposite(kind, "left", "right")
        {
            state.properties.insert("type".into(), k);
        }
        if self.name.ends_with("rail")
            && let Some(shape) = self.properties.get("shape")
        {
            let pairs: [(&str, &str); 3] = if across_z {
                [
                    ("ascending_north", "ascending_south"),
                    ("north_east", "south_east"),
                    ("north_west", "south_west"),
                ]
            } else {
                [
                    ("ascending_east", "ascending_west"),
                    ("north_east", "north_west"),
                    ("south_east", "south_west"),
                ]
            };
            if let Some(v) = pairs.iter().find_map(|(a, b)| opposite(shape, a, b)) {
                state.properties.insert("shape".into(), v);
            }
        }
        state
    }
    /// The items one block of this state takes to place, `(item id, count)`; none for a block no
    /// item places (the other half of a door, a bed's head, fluids that are not placed, fire).
    pub fn items(&self) -> Vec<(String, usize)> {
        let bare = self.name.strip_prefix("minecraft:").unwrap_or(&self.name);
        let prop = |k: &str| self.properties.get(k).map(String::as_str);
        if self.air()
            || prop("half") == Some("upper")
            || prop("part") == Some("head")
            || matches!(
                bare,
                "water"
                    | "lava"
                    | "fire"
                    | "soul_fire"
                    | "bubble_column"
                    | "nether_portal"
                    | "end_portal"
                    | "end_gateway"
                    | "moving_piston"
                    | "piston_head"
                    | "frosted_ice"
                    | "light"
                    | "void_air"
            )
        {
            // Water and lava stand for a bucket of each where they are placed as sources.
            return match (bare, prop("level")) {
                ("water", Some("0")) => vec![("minecraft:water_bucket".into(), 1)],
                ("lava", Some("0")) => vec![("minecraft:lava_bucket".into(), 1)],
                _ => Vec::new(),
            };
        }
        let mut item = bare.to_owned();
        let mut out: Vec<(String, usize)> = Vec::new();
        if let Some(rest) = bare.strip_prefix("potted_") {
            out.push(("minecraft:flower_pot".into(), 1));
            item = rest.to_owned();
        }
        // A block on a wall is placed by the item of the free-standing one.
        for (from, to) in [
            ("_wall_torch", "_torch"),
            ("wall_torch", "torch"),
            ("_wall_sign", "_sign"),
            ("_wall_hanging_sign", "_hanging_sign"),
            ("_wall_banner", "_banner"),
            ("_wall_head", "_head"),
            ("_wall_skull", "_skull"),
            ("_wall_fan", "_fan"),
        ] {
            if let Some(stem) = item.strip_suffix(from) {
                item = format!("{stem}{to}");
                break;
            }
        }
        item = match item.as_str() {
            "redstone_wire" => "redstone",
            "tripwire" => "string",
            "carrots" => "carrot",
            "potatoes" => "potato",
            "beetroots" => "beetroot_seeds",
            "wheat" => "wheat_seeds",
            "melon_stem" | "attached_melon_stem" => "melon_seeds",
            "pumpkin_stem" | "attached_pumpkin_stem" => "pumpkin_seeds",
            "sweet_berry_bush" => "sweet_berries",
            "cocoa" => "cocoa_beans",
            "kelp_plant" => "kelp",
            "tall_seagrass" => "seagrass",
            "bamboo_sapling" => "bamboo",
            "water_cauldron" | "lava_cauldron" | "powder_snow_cauldron" => "cauldron",
            "big_dripleaf_stem" => "big_dripleaf",
            "cave_vines" | "cave_vines_plant" => "glow_berries",
            "weeping_vines_plant" => "weeping_vines",
            "twisting_vines_plant" => "twisting_vines",
            other => other,
        }
        .to_owned();
        // Blocks that stand for several of an item: a double slab, pickles, candles, eggs.
        let count = if prop("type") == Some("double") {
            2
        } else if let Some(n) = ["pickles", "candles", "eggs"]
            .iter()
            .find_map(|k| prop(k))
            .and_then(|v| v.parse::<usize>().ok())
        {
            n.max(1)
        } else {
            1
        };
        out.push((format!("minecraft:{item}"), count));
        out
    }
    /// Whether the state holds water (a waterlogged block).
    pub fn waterlogged(&self) -> bool {
        self.properties
            .get("waterlogged")
            .is_some_and(|v| v == "true")
    }
    /// The state as a preview turned by `code` shows it: mirrored first (`code / 4`), then turned
    /// `code % 4` quarters clockwise.
    pub fn transformed(&self, code: u8) -> Self {
        self.mirrored(code / 4 % 3).rotated(code % 4)
    }
    pub fn matches(&self, name: &str, mut property: impl FnMut(&str) -> Option<String>) -> bool {
        self.name == name
            && self.properties.iter().all(|(key, value)| {
                matches!(
                    key.as_str(),
                    "north"
                        | "east"
                        | "south"
                        | "west"
                        | "distance"
                        | "powered"
                        | "lit"
                        | "snowy"
                        | "moisture"
                ) || (key == "shape" && self.name.ends_with("_stairs"))
                    || property(key).as_ref() == Some(value)
            })
    }
}
#[derive(Clone, Debug)]
pub struct Blueprint {
    pub blocks: Vec<(Pos, BlockState)>,
    pub data_version: i32,
    pub ignored_data: usize,
    pub format: &'static str,
}
impl Blueprint {
    pub fn transformed(mut self, origin: Pos, rotation: u8) -> Result<Self> {
        for (at, state) in &mut self.blocks {
            match rotation / 4 % 3 {
                1 => at[2] = -at[2],
                2 => at[0] = -at[0],
                _ => {}
            }
            for _ in 0..rotation % 4 {
                *at = [-at[2], at[1], at[0]];
            }
            for i in 0..3 {
                at[i] = at[i].checked_add(origin[i]).ok_or("coordinate overflow")?;
            }
            if at[0].unsigned_abs() > 29_999_984
                || at[2].unsigned_abs() > 29_999_984
                || at[1].unsigned_abs() > 2048
            {
                return Err("schematic exceeds world bounds".into());
            }
            *state = state.transformed(rotation);
        }
        self.blocks.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
        Ok(self)
    }
    pub fn materials(&self) -> BTreeMap<String, usize> {
        let mut out = BTreeMap::new();
        for (_, s) in &self.blocks {
            if s.properties.get("half").is_some_and(|v| v == "upper")
                || s.properties.get("part").is_some_and(|v| v == "head")
            {
                continue;
            }
            *out.entry(s.name.clone()).or_default() +=
                if s.properties.get("type").is_some_and(|v| v == "double") {
                    2
                } else {
                    1
                };
        }
        out
    }
}
#[derive(Clone, Debug)]
enum Tag {
    Number(i64),
    String(String),
    Bytes(Vec<u8>),
    List(Vec<Tag>),
    Compound(BTreeMap<String, Tag>),
    Ints(Vec<i32>),
    Longs(Vec<u64>),
    Ignored,
}
impl Tag {
    fn get(&self, key: &str) -> Result<&Self> {
        self.map()?.get(key).ok_or_else(|| format!("missing {key}"))
    }
    fn optional(&self, key: &str) -> Option<&Self> {
        self.map().ok()?.get(key)
    }
    fn map(&self) -> Result<&BTreeMap<String, Tag>> {
        if let Self::Compound(v) = self {
            Ok(v)
        } else {
            Err("expected NBT compound".into())
        }
    }
    fn number(&self) -> Result<i32> {
        if let Self::Number(v) = self {
            i32::try_from(*v).map_err(|_| "integer overflow".into())
        } else {
            Err("expected NBT integer".into())
        }
    }
    fn string(&self) -> Result<&str> {
        if let Self::String(v) = self {
            Ok(v)
        } else {
            Err("expected NBT string".into())
        }
    }
    fn list(&self) -> Result<&[Tag]> {
        if let Self::List(v) = self {
            Ok(v)
        } else {
            Err("expected NBT list".into())
        }
    }
    fn size(&self) -> usize {
        match self {
            Self::List(v) => v.len(),
            Self::Compound(v) => v.len(),
            _ => 0,
        }
    }
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    nodes: usize,
}
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self
            .at
            .checked_add(n)
            .filter(|n| *n <= self.bytes.len())
            .ok_or("truncated NBT")?;
        let data = &self.bytes[self.at..end];
        self.at = end;
        Ok(data)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn length(&mut self) -> Result<usize> {
        let n = i32::from_be_bytes(self.take(4)?.try_into().unwrap());
        usize::try_from(n)
            .ok()
            .filter(|v| *v <= MAX_BYTES)
            .ok_or("invalid NBT array length".into())
    }
    fn string(&mut self) -> Result<String> {
        let n = u16::from_be_bytes(self.take(2)?.try_into().unwrap()) as usize;
        let bytes = self.take(n)?;
        let mut units = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let first = bytes[i];
            i += 1;
            let value = if first < 128 {
                u16::from(first)
            } else if first & 0xe0 == 0xc0 {
                let b = *bytes.get(i).ok_or("invalid NBT string")?;
                i += 1;
                if b & 0xc0 != 0x80 {
                    return Err("invalid NBT string".into());
                }
                ((u16::from(first) & 31) << 6) | (u16::from(b) & 63)
            } else if first & 0xf0 == 0xe0 {
                let b = *bytes.get(i).ok_or("invalid NBT string")?;
                let c = *bytes.get(i + 1).ok_or("invalid NBT string")?;
                i += 2;
                if b & 0xc0 != 0x80 || c & 0xc0 != 0x80 {
                    return Err("invalid NBT string".into());
                }
                ((u16::from(first) & 15) << 12) | ((u16::from(b) & 63) << 6) | (u16::from(c) & 63)
            } else {
                return Err("invalid NBT modified UTF-8".into());
            };
            units.push(value);
        }
        String::from_utf16(&units).map_err(|_| "invalid NBT UTF-16".into())
    }
    fn tag(&mut self, id: u8, depth: usize) -> Result<Tag> {
        self.nodes += 1;
        if depth > 32 || self.nodes > 500_000 {
            return Err("NBT nesting/node limit exceeded".into());
        }
        Ok(match id {
            1 => Tag::Number(i64::from(self.byte()? as i8)),
            2 => Tag::Number(i64::from(i16::from_be_bytes(
                self.take(2)?.try_into().unwrap(),
            ))),
            3 => Tag::Number(i64::from(i32::from_be_bytes(
                self.take(4)?.try_into().unwrap(),
            ))),
            4 => Tag::Number(i64::from_be_bytes(self.take(8)?.try_into().unwrap())),
            5 => {
                self.take(4)?;
                Tag::Ignored
            }
            6 => {
                self.take(8)?;
                Tag::Ignored
            }
            7 => {
                let n = self.length()?;
                Tag::Bytes(self.take(n)?.to_vec())
            }
            8 => Tag::String(self.string()?),
            9 => {
                let ty = self.byte()?;
                let n = self.length()?;
                if n > 500_000 {
                    return Err("NBT list too large".into());
                }
                let mut list = Vec::new();
                for _ in 0..n {
                    list.push(self.tag(ty, depth + 1)?);
                }
                Tag::List(list)
            }
            10 => {
                let mut map = BTreeMap::new();
                loop {
                    let ty = self.byte()?;
                    if ty == 0 {
                        break;
                    }
                    let name = self.string()?;
                    let value = self.tag(ty, depth + 1)?;
                    if map.insert(name, value).is_some() {
                        return Err("duplicate NBT key".into());
                    }
                }
                Tag::Compound(map)
            }
            11 => {
                let n = self.length()?;
                let bytes = self.take(n.checked_mul(4).ok_or("array overflow")?)?;
                Tag::Ints(
                    bytes
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| i32::from_be_bytes(*b))
                        .collect(),
                )
            }
            12 => {
                let n = self.length()?;
                let bytes = self.take(n.checked_mul(8).ok_or("array overflow")?)?;
                Tag::Longs(
                    bytes
                        .as_chunks::<8>()
                        .0
                        .iter()
                        .map(|b| u64::from_be_bytes(*b))
                        .collect(),
                )
            }
            _ => return Err("unsupported NBT tag".into()),
        })
    }
}
fn volume(size: Pos) -> Result<usize> {
    if size.iter().any(|s| *s == 0 || s.unsigned_abs() > MAX_SIDE) {
        return Err(format!("schematic dimensions must be 1–{MAX_SIDE}"));
    }
    let n = size
        .iter()
        .map(|s| s.unsigned_abs() as usize)
        .product::<usize>();
    if n > MAX_VOLUME {
        Err("schematic volume limit exceeded".into())
    } else {
        Ok(n)
    }
}
fn vector(tag: &Tag) -> Result<Pos> {
    if let Tag::Ints(v) = tag {
        v.as_slice()
            .try_into()
            .map_err(|_| "expected three coordinates".into())
    } else {
        Ok([
            tag.get("x")?.number()?,
            tag.get("y")?.number()?,
            tag.get("z")?.number()?,
        ])
    }
}
fn nbt_state(tag: &Tag) -> Result<BlockState> {
    let mut state = BlockState::parse(tag.get("Name")?.string()?)?;
    if let Some(props) = tag.optional("Properties") {
        if props.map()?.len() > 32 {
            return Err("too many block properties".into());
        }
        for (k, v) in props.map()? {
            state.properties.insert(k.clone(), v.string()?.into());
        }
    }
    Ok(state)
}
fn add(blocks: &mut BTreeMap<Pos, BlockState>, at: Pos, state: &BlockState) -> Result<()> {
    if state.air() {
        return Ok(());
    }
    if at.iter().any(|v| v.unsigned_abs() > 4096) {
        return Err("region position limit exceeded".into());
    }
    if let Some(old) = blocks.insert(at, state.clone())
        && old != *state
    {
        return Err("overlapping regions disagree".into());
    }
    if blocks.len() > MAX_BLOCKS {
        return Err("non-air block limit exceeded".into());
    }
    Ok(())
}
pub fn decode(bytes: &[u8]) -> Result<Blueprint> {
    if bytes.len() > MAX_BYTES {
        return Err("schematic file too large".into());
    }
    let mut reader = Reader {
        bytes,
        at: 0,
        nodes: 0,
    };
    if reader.byte()? != 10 {
        return Err("NBT root must be a compound".into());
    }
    reader.string()?;
    let root = reader.tag(10, 0)?;
    if reader.at != bytes.len() {
        return Err("trailing NBT data".into());
    }
    let root = root.optional("Schematic").unwrap_or(&root);
    let version = root.get("Version")?.number()?;
    let mut blocks = BTreeMap::new();
    let mut ignored = 0;
    let format;
    if let Some(regions) = root.optional("Regions") {
        if !(4..=7).contains(&version) {
            return Err("unsupported Litematica version".into());
        }
        format = "litematic";
        if regions.map()?.is_empty() || regions.size() > 16 {
            return Err("region count limit exceeded".into());
        }
        let mut total = 0;
        for region in regions.map()?.values() {
            let size = vector(region.get("Size")?)?;
            let n = volume(size)?;
            total += n;
            if total > MAX_VOLUME {
                return Err("total region volume limit exceeded".into());
            }
            let position = vector(region.get("Position")?)?;
            let min = std::array::from_fn::<_, 3, _>(|i| {
                position[i]
                    .checked_add(if size[i] < 0 { size[i] + 1 } else { 0 })
                    .unwrap_or(i32::MAX)
            });
            if min.iter().any(|v| v.unsigned_abs() > 4096) {
                return Err("region position limit exceeded".into());
            }
            let palette = region
                .get("BlockStatePalette")?
                .list()?
                .iter()
                .map(nbt_state)
                .collect::<Result<Vec<_>>>()?;
            if palette.is_empty() || palette.len() > 65_536 {
                return Err("invalid region palette".into());
            }
            let bits = (usize::BITS - (palette.len() - 1).leading_zeros()).max(2) as usize;
            let Tag::Longs(words) = region.get("BlockStates")? else {
                return Err("expected packed block states".into());
            };
            if words.len()
                != n.checked_mul(bits)
                    .ok_or("packed array overflow")?
                    .div_ceil(64)
            {
                return Err("packed block-state length mismatch".into());
            }
            let [sx, _, sz] = size.map(i32::unsigned_abs);
            for i in 0..n {
                let bit = i * bits;
                let shift = bit % 64;
                let mut value = words[bit / 64] >> shift;
                if shift + bits > 64 {
                    value |= words[bit / 64 + 1] << (64 - shift);
                }
                let index = (value & ((1u64 << bits) - 1)) as usize;
                let state = palette.get(index).ok_or("palette index out of bounds")?;
                add(
                    &mut blocks,
                    [
                        min[0] + (i % sx as usize) as i32,
                        min[1] + (i / (sx * sz) as usize) as i32,
                        min[2] + ((i / sx as usize) % sz as usize) as i32,
                    ],
                    state,
                )?;
            }
            ignored += region.optional("TileEntities").map_or(0, Tag::size)
                + region.optional("Entities").map_or(0, Tag::size);
        }
    } else {
        if !matches!(version, 2 | 3) {
            return Err("unsupported Sponge schematic version".into());
        }
        format = "schem";
        let size = [
            root.get("Width")?.number()?,
            root.get("Height")?.number()?,
            root.get("Length")?.number()?,
        ];
        if size.iter().any(|v| *v <= 0) {
            return Err("Sponge dimensions must be positive".into());
        }
        let n = volume(size)?;
        let offset = root
            .optional("Offset")
            .map(vector)
            .transpose()?
            .unwrap_or([0; 3]);
        if offset.iter().any(|v| v.unsigned_abs() > 4096) {
            return Err("schematic offset limit exceeded".into());
        }
        let container = if version == 3 {
            root.get("Blocks")?
        } else {
            root
        };
        let map = container.get("Palette")?.map()?;
        if map.is_empty() || map.len() > 65_536 {
            return Err("invalid block palette".into());
        }
        let mut palette = BTreeMap::new();
        for (name, id) in map {
            let id = id.number()?;
            if id < 0
                || palette
                    .insert(id as u32, BlockState::parse(name)?)
                    .is_some()
            {
                return Err("duplicate palette id".into());
            }
        }
        let Tag::Bytes(data) = container.get(if version == 3 { "Data" } else { "BlockData" })?
        else {
            return Err("expected schematic block data".into());
        };
        let mut cursor = 0;
        for i in 0..n {
            let mut index = 0u32;
            let mut finished = false;
            for shift in (0..35).step_by(7) {
                let b = *data.get(cursor).ok_or("truncated block varint")?;
                cursor += 1;
                if shift == 28 && b > 15 {
                    return Err("block varint overflow".into());
                }
                index |= u32::from(b & 127) << shift;
                if b & 128 == 0 {
                    finished = true;
                    break;
                }
            }
            if !finished {
                return Err("invalid block varint".into());
            }
            let state = palette.get(&index).ok_or("palette index out of bounds")?;
            add(
                &mut blocks,
                [
                    offset[0] + (i % size[0] as usize) as i32,
                    offset[1] + (i / (size[0] * size[2]) as usize) as i32,
                    offset[2] + ((i / size[0] as usize) % size[2] as usize) as i32,
                ],
                state,
            )?;
        }
        if cursor != data.len() {
            return Err("extra block data".into());
        }
        ignored = container.optional("BlockEntities").map_or(0, Tag::size)
            + root.optional("Entities").map_or(0, Tag::size);
    }
    if blocks.is_empty() {
        return Err("schematic contains no blocks".into());
    }
    let data_version = root
        .optional("MinecraftDataVersion")
        .or_else(|| root.optional("DataVersion"))
        .map(Tag::number)
        .transpose()?
        .unwrap_or(0);
    Ok(Blueprint {
        blocks: blocks.into_iter().collect(),
        data_version,
        ignored_data: ignored,
        format,
    })
}
pub fn safe_filename(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    !name.is_empty()
        && name.len() <= 160
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        && !name.starts_with('.')
        && (lower.ends_with(".schem") || lower.ends_with(".litematic"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceError {
    Unreachable,
    Material,
    Refused,
    Manual,
    Timeout,
    Unsupported,
}
#[derive(Clone, Debug)]
pub struct PlacementRequest {
    pub target: Pos,
    pub item: String,
    pub state: BlockState,
    pub allowed: BTreeMap<Pos, BlockState>,
}
