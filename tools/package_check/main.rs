use horizon_mod_package::Package;
use std::path::Path;
fn main() {
    let dir = std::env::args().nth(1).expect("project folder or .hmod");
    let dir = Path::new(&dir);
    if dir.extension().is_some_and(|e| e == "hmod") {
        // What the launcher runs on a downloaded release asset (launcher/src/github_mods.rs `checked`).
        match horizon_mod_package::index::inspect(&horizon_mod_package::index::Kind::NativeMod, dir) {
            Ok(facts) => println!("HMOD OK: id={:?} version={:?}", facts.id, facts.version),
            Err(e) => { println!("HMOD ERROR: {e}"); std::process::exit(1); }
        }
        return;
    }
    // The launcher's own gate first (launcher/src/native_mods/package.rs): the old layout is refused.
    for old in ["horizon-native.json", "horizon-mixin.toml", "native", "rust"] {
        if dir.join(old).exists() {
            println!("REFUSED: unsupported package path {old}");
            std::process::exit(2);
        }
    }
    match Package::read(dir) {
        Ok(p) => {
            let m = &p.manifest;
            let n = m.native.as_ref().expect("native");
            println!("READ OK: id={} version={} api={} side={:?} mixins={} modules={} bridges={}", m.id, m.version, m.api, m.side, n.mixin.len(), n.module.len(), n.bridge.len());
        }
        Err(e) => { println!("READ ERROR: {e}"); std::process::exit(1); }
    }
    match p_files(dir) { Ok(n) => println!("FILES OK: {n} entries"), Err(e) => { println!("FILES ERROR: {e}"); std::process::exit(1); } }
    match Package::pack_directory(dir) { Ok(b) => println!("PACK OK: {} bytes", b.len()), Err(e) => { println!("PACK ERROR: {e}"); std::process::exit(1); } }
}
fn p_files(dir: &Path) -> Result<usize, String> {
    let p = Package::read(dir)?;
    Ok(p.directory_files(dir)?.len())
}
