// A port of the writing side of Litematica's `LitematicaSchematic` and `LitematicaBitArray`
// (LGPL-3.0, see NOTICE.md): regions of block states as the packed `BlockStates` array and the
// `BlockStatePalette`, the metadata and the root compound, as uncompressed NBT bytes (the caller
// compresses them). Pure, with a small NBT writer of its own. Tile entities and entities are
// written as empty lists, so a file made here holds blocks only.
use std::collections::BTreeMap;

pub type Pos = [i32; 3];
/// The version and sub version the original writes today (`SCHEMATIC_VERSION` 7, 1).
pub const VERSION: i32 = 7;
pub const SUB_VERSION: i32 = 1;

/// `LitematicaBitArray`: `arraySize` values of `bitsPerEntry` bits, one after the other across
/// the longs (not padded per long).
pub struct BitArray {
    words: Vec<i64>,
    bits: u32,
    mask: u64,
    size: usize,
}
impl BitArray {
    pub fn new(bits: u32, size: usize) -> Self {
        assert!((1..=32).contains(&bits));
        let words = (size * bits as usize).div_ceil(64);
        Self {
            words: vec![0; words],
            bits,
            mask: (1u64 << bits) - 1,
            size,
        }
    }
    pub fn len(&self) -> usize {
        self.size
    }
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
    pub fn set(&mut self, index: usize, value: u32) {
        let start = index * self.bits as usize;
        let (a, offset) = (start >> 6, (start & 63) as u32);
        let end = ((index + 1) * self.bits as usize - 1) >> 6;
        let value = u64::from(value) & self.mask;
        let word = self.words[a] as u64;
        self.words[a] = ((word & !(self.mask << offset)) | (value << offset)) as i64;
        if a != end {
            let spill = 64 - offset;
            let rest = self.bits - spill;
            let word = self.words[end] as u64;
            self.words[end] = (((word >> rest) << rest) | (value >> spill)) as i64;
        }
    }
    pub fn get(&self, index: usize) -> u32 {
        let start = index * self.bits as usize;
        let (a, offset) = (start >> 6, (start & 63) as u32);
        let end = ((index + 1) * self.bits as usize - 1) >> 6;
        let first = (self.words[a] as u64) >> offset;
        let value = if a == end {
            first
        } else {
            first | ((self.words[end] as u64) << (64 - offset))
        };
        (value & self.mask) as u32
    }
    pub fn words(&self) -> &[i64] {
        &self.words
    }
}

/// A block state as the palette keeps it: the block and its properties.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct State {
    pub name: String,
    pub properties: BTreeMap<String, String>,
}
impl State {
    pub fn air() -> Self {
        Self {
            name: "minecraft:air".into(),
            properties: BTreeMap::new(),
        }
    }
}

/// `LitematicaBlockStateContainer`: the cells of one region, x fastest, then z, then y, with a
/// palette whose first entry is air.
pub struct Region {
    pub name: String,
    /// The region's corner (its smallest x, y, z) relative to the schematic's origin.
    pub position: Pos,
    pub size: Pos,
    palette: Vec<State>,
    index: std::collections::HashMap<State, u32>,
    cells: Vec<u32>,
}
impl Region {
    pub fn new(name: &str, position: Pos, size: Pos) -> Option<Self> {
        if size.iter().any(|v| *v <= 0) {
            return None;
        }
        let volume = size
            .iter()
            .try_fold(1usize, |acc, v| acc.checked_mul(*v as usize))?;
        let mut region = Self {
            name: name.to_owned(),
            position,
            size,
            palette: Vec::new(),
            index: std::collections::HashMap::new(),
            cells: vec![0; volume],
        };
        region.id(&State::air());
        Some(region)
    }
    fn id(&mut self, state: &State) -> u32 {
        if let Some(i) = self.index.get(state) {
            return *i;
        }
        let i = self.palette.len() as u32;
        self.palette.push(state.clone());
        self.index.insert(state.clone(), i);
        i
    }
    /// `getIndex`: where the cell at (x, y, z) of the region sits.
    pub fn index_of(&self, x: i32, y: i32, z: i32) -> usize {
        ((y as usize * self.size[2] as usize) + z as usize) * self.size[0] as usize + x as usize
    }
    pub fn set(&mut self, x: i32, y: i32, z: i32, state: &State) {
        let i = self.id(state);
        let at = self.index_of(x, y, z);
        self.cells[at] = i;
    }
    pub fn volume(&self) -> usize {
        self.cells.len()
    }
    /// Cells that are not air.
    pub fn blocks(&self) -> usize {
        self.cells.iter().filter(|c| **c != 0).count()
    }
    /// The bits one entry takes: as few as hold the palette, never under two.
    pub fn bits(&self) -> u32 {
        (u32::BITS - (self.palette.len() as u32 - 1).leading_zeros()).max(2)
    }
    fn packed(&self) -> BitArray {
        let mut array = BitArray::new(self.bits(), self.cells.len());
        for (i, c) in self.cells.iter().enumerate() {
            array.set(i, *c);
        }
        array
    }
}

/// `SchematicMetadata`.
pub struct Metadata {
    pub name: String,
    pub author: String,
    pub description: String,
    pub time_ms: i64,
    pub data_version: i32,
}

// ---- NBT ----
enum Nbt {
    Int(i32),
    Long(i64),
    String(String),
    Longs(Vec<i64>),
    /// A list of one tag type (`10` compounds); empty lists are written with the end type.
    Compounds(Vec<Vec<(String, Nbt)>>),
    Compound(Vec<(String, Nbt)>),
}
impl Nbt {
    fn id(&self) -> u8 {
        match self {
            Nbt::Int(_) => 3,
            Nbt::Long(_) => 4,
            Nbt::String(_) => 8,
            Nbt::Compounds(_) => 9,
            Nbt::Compound(_) => 10,
            Nbt::Longs(_) => 12,
        }
    }
}
fn string(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    out.extend_from_slice(&(bytes.len().min(0xffff) as u16).to_be_bytes());
    out.extend_from_slice(&bytes[..bytes.len().min(0xffff)]);
}
fn payload(out: &mut Vec<u8>, tag: &Nbt) {
    match tag {
        Nbt::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
        Nbt::String(s) => string(out, s),
        Nbt::Longs(v) => {
            out.extend_from_slice(&(v.len() as i32).to_be_bytes());
            for w in v {
                out.extend_from_slice(&w.to_be_bytes());
            }
        }
        Nbt::Compounds(list) => {
            out.push(if list.is_empty() { 0 } else { 10 });
            out.extend_from_slice(&(list.len() as i32).to_be_bytes());
            for item in list {
                fields(out, item);
            }
        }
        Nbt::Compound(items) => fields(out, items),
    }
}
fn fields(out: &mut Vec<u8>, items: &[(String, Nbt)]) {
    for (name, tag) in items {
        out.push(tag.id());
        string(out, name);
        payload(out, tag);
    }
    out.push(0);
}
fn field(name: &str, tag: Nbt) -> (String, Nbt) {
    (name.to_owned(), tag)
}
fn vec3(p: Pos) -> Nbt {
    Nbt::Compound(vec![
        field("x", Nbt::Int(p[0])),
        field("y", Nbt::Int(p[1])),
        field("z", Nbt::Int(p[2])),
    ])
}

/// `writeToData`: the whole file, as uncompressed NBT (the root compound has the empty name).
pub fn write(meta: &Metadata, regions: &[Region]) -> Vec<u8> {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    let mut volume = 0usize;
    let mut blocks = 0usize;
    let mut map: Vec<(String, Nbt)> = Vec::new();
    for r in regions {
        for i in 0..3 {
            lo[i] = lo[i].min(r.position[i]);
            hi[i] = hi[i].max(r.position[i] + r.size[i] - 1);
        }
        volume += r.volume();
        blocks += r.blocks();
        let palette: Vec<Vec<(String, Nbt)>> = r
            .palette
            .iter()
            .map(|s| {
                let mut tag = vec![field("Name", Nbt::String(s.name.clone()))];
                if !s.properties.is_empty() {
                    tag.push(field(
                        "Properties",
                        Nbt::Compound(
                            s.properties
                                .iter()
                                .map(|(k, v)| field(k, Nbt::String(v.clone())))
                                .collect(),
                        ),
                    ));
                }
                tag
            })
            .collect();
        map.push(field(
            &r.name,
            Nbt::Compound(vec![
                field("BlockStatePalette", Nbt::Compounds(palette)),
                field("BlockStates", Nbt::Longs(r.packed().words().to_vec())),
                field("TileEntities", Nbt::Compounds(Vec::new())),
                field("Entities", Nbt::Compounds(Vec::new())),
                field("Position", vec3(r.position)),
                field("Size", vec3(r.size)),
            ]),
        ));
    }
    let enclosing = if regions.is_empty() {
        [0; 3]
    } else {
        [0, 1, 2].map(|i| hi[i] - lo[i] + 1)
    };
    let metadata = Nbt::Compound(vec![
        field("Name", Nbt::String(meta.name.clone())),
        field("Author", Nbt::String(meta.author.clone())),
        field("Description", Nbt::String(meta.description.clone())),
        field("RegionCount", Nbt::Int(regions.len() as i32)),
        field(
            "TotalVolume",
            Nbt::Int(volume.min(i32::MAX as usize) as i32),
        ),
        field(
            "TotalBlocks",
            Nbt::Int(blocks.min(i32::MAX as usize) as i32),
        ),
        field("TimeCreated", Nbt::Long(meta.time_ms)),
        field("TimeModified", Nbt::Long(meta.time_ms)),
        field("EnclosingSize", vec3(enclosing)),
    ]);
    let root = vec![
        field("MinecraftDataVersion", Nbt::Int(meta.data_version)),
        field("Version", Nbt::Int(VERSION)),
        field("SubVersion", Nbt::Int(SUB_VERSION)),
        field("Metadata", metadata),
        field("Regions", Nbt::Compound(map)),
    ];
    let mut out = vec![10];
    string(&mut out, "");
    fields(&mut out, &root);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bit_array_packs_across_longs_and_reads_back() {
        for bits in [2u32, 3, 5, 7, 13] {
            let n = 300usize;
            let mut a = BitArray::new(bits, n);
            let max = (1u32 << bits) - 1;
            for i in 0..n {
                a.set(i, (i as u32 * 7 + 3) & max);
            }
            for i in 0..n {
                assert_eq!(a.get(i), (i as u32 * 7 + 3) & max, "bits {bits} at {i}");
            }
            assert_eq!(a.words().len(), (n * bits as usize).div_ceil(64));
        }
        // Setting a value again replaces it without touching the neighbours.
        let mut a = BitArray::new(5, 40);
        for i in 0..40 {
            a.set(i, 31);
        }
        a.set(12, 0);
        assert_eq!((a.get(11), a.get(12), a.get(13)), (31, 0, 31));
    }

    #[test]
    fn a_region_keeps_air_first_and_at_least_two_bits() {
        let mut r = Region::new("a", [0; 3], [2, 1, 2]).unwrap();
        assert_eq!(r.bits(), 2);
        let stone = State {
            name: "minecraft:stone".into(),
            properties: BTreeMap::new(),
        };
        r.set(1, 0, 1, &stone);
        assert_eq!((r.volume(), r.blocks()), (4, 1));
        assert_eq!(r.index_of(1, 0, 1), 3);
        assert!(Region::new("a", [0; 3], [0, 1, 1]).is_none());
        for i in 0..40 {
            r.set(
                0,
                0,
                0,
                &State {
                    name: format!("minecraft:b{i}"),
                    properties: BTreeMap::new(),
                },
            );
        }
        // Forty-two states need six bits.
        assert_eq!(r.bits(), 6);
    }
}
