//! Resource-defined random-spread candidates; final validation belongs to the generator.
use std::collections::BTreeMap;

pub struct Spread {
    spacing: i32,
    range: i32,
    pub salt: i32,
    triangular: bool,
}

pub fn load(raw: &BTreeMap<String, String>) -> Result<BTreeMap<String, Spread>, String> {
    let mut spreads = BTreeMap::new();
    for (id, text) in raw {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let placement = &value["placement"];
        if !matches!(
            placement["type"].as_str(),
            Some("minecraft:random_spread" | "random_spread")
        ) {
            continue;
        }
        let number = |key: &str, default: i32| {
            placement[key]
                .as_i64()
                .map_or(Ok(default), |n| i32::try_from(n))
                .map_err(|_| format!("invalid {key} in structure set {id}"))
        };
        let spacing = number("spacing", 1)?;
        let separation = number("separation", 0)?;
        let salt = number("salt", 0)?;
        if !(1..=4096).contains(&spacing) || separation < 0 || separation >= spacing {
            return Err(format!("invalid random-spread spacing in structure set {id}"));
        }
        spreads.insert(
            id.clone(),
            Spread {
                spacing,
                range: spacing - separation,
                salt,
                triangular: matches!(
                    placement["spread_type"].as_str(),
                    Some("triangular" | "minecraft:triangular")
                ),
            },
        );
    }
    Ok(spreads)
}

impl Spread {
    pub fn candidate(&self, region: [i32; 2], mut next: impl FnMut(i32) -> i32) -> [i32; 2] {
        region.map(|coordinate| {
            let first = next(self.range);
            let offset = if self.triangular {
                (first + next(self.range)) / 2
            } else {
                first
            };
            coordinate.wrapping_mul(self.spacing).wrapping_add(offset)
        })
    }
}
