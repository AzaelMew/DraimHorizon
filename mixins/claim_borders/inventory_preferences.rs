// Inventory tools settings shared by the tested model and native adapter.
use super::claim_borders_inventory_sort::Mode;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    pub visible: bool,
    pub enabled: bool,
    pub columns: bool,
    pub mode: Mode,
    pub offset: [i16; 2],
    pub ignored: BTreeSet<usize>,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            visible: true,
            enabled: true,
            columns: false,
            mode: Mode::Id,
            offset: [0, 0],
            ignored: BTreeSet::new(),
        }
    }
}
impl Policy {
    pub fn encode(&self) -> Value {
        json!({"visible":self.visible,"enabled":self.enabled,"columns":self.columns,
            "mode":self.mode.number(),"offset":self.offset,"ignored":self.ignored})
    }
    fn decode(v: &Value) -> Result<Self, String> {
        if !v.is_object() {
            return Err("invalid inventory policy".into());
        }
        let ignored = v["ignored"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_u64)
                    .filter(|i| *i < 200)
                    .map(|i| i as usize)
                    .collect()
            })
            .unwrap_or_default();
        let offset = [0, 1].map(|i| v["offset"][i].as_i64().unwrap_or(0).clamp(-200, 200) as i16);
        Ok(Self {
            visible: v["visible"].as_bool().unwrap_or(true),
            enabled: v["enabled"].as_bool().unwrap_or(true),
            columns: v["columns"].as_bool().unwrap_or(false),
            mode: Mode::from_number(v["mode"].as_u64().unwrap_or(0)),
            offset,
            ignored,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub frames: bool,
    pub signs: bool,
    pub defaults: Policy,
    pub policies: BTreeMap<String, Policy>,
    /// Explicit opt-in for virtual server windows. Real storage is detected independently.
    pub virtual_menus: BTreeSet<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            frames: true,
            signs: true,
            defaults: Policy::default(),
            policies: BTreeMap::new(),
            virtual_menus: BTreeSet::new(),
        }
    }
}
impl Preferences {
    pub fn policy(&self, key: &str) -> Policy {
        self.policies.get(key).cloned().unwrap_or_else(|| {
            // A default button layout/order must not silently lock another container's items.
            let mut p = self.defaults.clone();
            p.ignored.clear();
            p
        })
    }
    pub fn encode(&self) -> String {
        let policies: BTreeMap<_, _> = self.policies.iter().map(|(k, p)| (k, p.encode())).collect();
        json!({"schema":1,"frames":self.frames,"signs":self.signs,"defaults":self.defaults.encode(),
            "policies":policies,"virtual_menus":self.virtual_menus})
        .to_string()
    }
    pub fn decode(raw: &str) -> Result<Self, String> {
        if raw.len() > 262144 {
            return Err("inventory settings too large".into());
        }
        let v: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
        if v["schema"].as_u64() != Some(1) {
            return Err("unknown inventory settings schema".into());
        }
        let mut p = Self {
            frames: v["frames"].as_bool().unwrap_or(true),
            signs: v["signs"].as_bool().unwrap_or(true),
            ..Self::default()
        };
        if !v["defaults"].is_null() {
            p.defaults = Policy::decode(&v["defaults"])?;
            p.defaults.ignored.clear();
        }
        if let Some(policies) = v["policies"].as_object() {
            if policies.len() > 256 {
                return Err("too many inventory policies".into());
            }
            for (key, policy) in policies {
                if key.len() > 512 {
                    return Err("inventory policy key too long".into());
                }
                p.policies.insert(key.clone(), Policy::decode(policy)?);
            }
        }
        if let Some(menus) = v["virtual_menus"].as_array() {
            if menus.len() > 256 {
                return Err("too many virtual menus".into());
            }
            p.virtual_menus = menus
                .iter()
                .filter_map(Value::as_str)
                .filter(|s| s.len() <= 512)
                .map(str::to_owned)
                .collect();
        }
        Ok(p)
    }
}
/// Storage layout from an actual block, not the title of an arbitrary plugin menu.
pub fn storage_menu(block: &str, menu: &str) -> bool {
    let b = block.strip_prefix("minecraft:").unwrap_or("");
    match b {
        "chest" | "trapped_chest" => {
            matches!(menu, "minecraft:generic_9x3" | "minecraft:generic_9x6")
        }
        "barrel" | "ender_chest" => menu == "minecraft:generic_9x3",
        "hopper" => menu == "minecraft:hopper",
        "dispenser" | "dropper" => menu == "minecraft:generic_3x3",
        _ if b == "shulker_box" || b.ends_with("_shulker_box") => menu == "minecraft:shulker_box",
        _ => false,
    }
}
pub fn economic_title(title: &str) -> bool {
    let t = title.to_lowercase();
    [
        "market",
        "auction",
        "shop",
        "trade",
        "bank",
        "магазин",
        "аукцион",
        "рынок",
        "торги",
        "обмен",
        "банк",
    ]
    .iter()
    .any(|word| t.contains(word))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policies_roundtrip_defaults_dont_copy_slot_locks() {
        let mut p = Preferences::default();
        p.defaults.ignored.insert(3);
        p.defaults.columns = true;
        let local = Policy { ignored: BTreeSet::from([0, 26]), ..Policy::default() };
        p.policies.insert("inventory/player".into(), local.clone());
        p.virtual_menus.insert("custom bag".into());
        let decoded = Preferences::decode(&p.encode()).unwrap();
        assert_eq!(decoded.policy("inventory/player"), local);
        assert!(decoded.policy("chest").ignored.is_empty());
        assert!(decoded.policy("chest").columns);
        assert!(decoded.virtual_menus.contains("custom bag"));
    }
    #[test]
    fn malformed_and_future_settings_are_not_silently_replaced() {
        for raw in [
            "{}",
            "{",
            r#"{"schema":99}"#,
            r#"{"schema":1,"policies":{"x":null}}"#,
        ] {
            assert!(Preferences::decode(raw).is_err());
        }
        let p = Preferences::decode(
            r#"{"schema":1,"policies":{"x":{"offset":[999,-999],"ignored":[0,999]}}}"#,
        )
        .unwrap();
        assert_eq!(p.policy("x").offset, [200, -200]);
        assert_eq!(p.policy("x").ignored, BTreeSet::from([0]));
    }
    #[test]
    fn physical_storage_and_economic_menu_guards() {
        assert!(storage_menu("minecraft:chest", "minecraft:generic_9x6"));
        assert!(storage_menu(
            "minecraft:red_shulker_box",
            "minecraft:shulker_box"
        ));
        assert!(!storage_menu("minecraft:stone", "minecraft:generic_9x3"));
        assert!(!storage_menu("minecraft:barrel", "minecraft:generic_9x6"));
        for title in ["Auction house", "Market", "Обмен предметами"] {
            assert!(economic_title(title));
        }
        assert!(!economic_title("Мои ресурсы"));
    }
}
