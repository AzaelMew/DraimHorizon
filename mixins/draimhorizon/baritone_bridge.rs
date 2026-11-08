// Commands for the horizon-baritone native mod (https://github.com/luhnox/horizon-baritone), a
// port of Baritone 1.21.4. When that mod is installed and on, the Baritone tabs hand their task
// to it as a `/baritone <line>` command through Horizon's native command hook. Nothing is sent to
// the server: a command no mod ran stays local and the tab falls back to the built-in driver.
pub const MOD_ID: &str = "baritone";
pub const PAUSE: &str = "pause";
pub const RESUME: &str = "resume";
pub const CANCEL: &str = "cancel";

pub fn goto(at: [i32; 3]) -> String {
    format!("goto {} {} {}", at[0], at[1], at[2])
}
/// Sets and draws the goal without moving.
pub fn goal(at: [i32; 3]) -> String {
    format!("goal {} {} {}", at[0], at[1], at[2])
}
/// `mine [count] <blocks...>`; a count of 0 mines without a target amount.
pub fn mine(count: u16, blocks: &[&str]) -> Option<String> {
    let blocks: Vec<&str> = blocks.iter().copied().filter(|b| valid_id(b)).collect();
    if blocks.is_empty() {
        return None;
    }
    let list = blocks.join(" ");
    Some(if count == 0 {
        format!("mine {list}")
    } else {
        format!("mine {count} {list}")
    })
}
/// `tunnel <height> <width> <depth>` in the direction the player faces.
pub fn tunnel(height: u8, width: u8, depth: u16) -> String {
    format!("tunnel {height} {width} {depth}")
}
pub fn farm(range: u8) -> String {
    format!("farm {range}")
}
/// `follow player <name>`; refuses names that could carry extra arguments.
pub fn follow(name: &str) -> Option<String> {
    let ok = (1..=16).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    ok.then(|| format!("follow player {name}"))
}
/// `build <file> <x> <y> <z>`; the file name is relative to Baritone's schematics folder and
/// must be one word, because Baritone splits arguments on spaces.
pub fn build(file: &str, origin: [i32; 3]) -> Option<String> {
    let ok = !file.is_empty()
        && !file.contains(char::is_whitespace)
        && !file.contains(['/', '\\'])
        && file != "."
        && file != "..";
    ok.then(|| format!("build {file} {} {} {}", origin[0], origin[1], origin[2]))
}
/// A file name safe for `build`: whitespace becomes `_`, path separators are dropped.
pub fn schematic_name(file: &str) -> String {
    let name: String = file
        .chars()
        .filter(|c| !matches!(c, '/' | '\\'))
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    if name.is_empty() || name == "." || name == ".." {
        "schematic".into()
    } else {
        name
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_:./-".contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_use_baritone_syntax() {
        assert_eq!(goto([1, -64, -3]), "goto 1 -64 -3");
        assert_eq!(goal([0, 70, 0]), "goal 0 70 0");
        assert_eq!(
            mine(
                16,
                &["minecraft:diamond_ore", "minecraft:deepslate_diamond_ore"]
            )
            .as_deref(),
            Some("mine 16 minecraft:diamond_ore minecraft:deepslate_diamond_ore")
        );
        assert_eq!(
            mine(0, &["minecraft:stone"]).as_deref(),
            Some("mine minecraft:stone")
        );
        assert_eq!(mine(8, &["bad name"]), None);
        assert_eq!(tunnel(2, 1, 32), "tunnel 2 1 32");
        assert_eq!(farm(6), "farm 6");
    }
    #[test]
    fn arguments_cannot_smuggle_extra_words() {
        assert_eq!(
            follow("Steve_01").as_deref(),
            Some("follow player Steve_01")
        );
        assert_eq!(follow("a b"), None);
        assert_eq!(follow(""), None);
        assert_eq!(
            build("house.schem", [1, 2, 3]).as_deref(),
            Some("build house.schem 1 2 3")
        );
        assert_eq!(build("my house.schem", [0, 0, 0]), None);
        assert_eq!(build("../x.schem", [0, 0, 0]), None);
        assert_eq!(schematic_name("my house.schem"), "my_house.schem");
        assert_eq!(schematic_name("a/../b.schem"), "a..b.schem");
        assert_eq!(schematic_name(".."), "schematic");
    }
}
