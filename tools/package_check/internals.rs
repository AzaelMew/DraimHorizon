//! Run the launcher's actual declared-internal resolver against a project and SDK catalog.
use std::path::PathBuf;
use sunrise_mod_package::{Package, internals};

fn check() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let project = PathBuf::from(args.next().ok_or("expected project folder")?);
    let catalog = PathBuf::from(args.next().ok_or("expected SDK internals.json")?);
    let package = Package::read(&project)?;
    let entries = internals::read(&catalog)?;
    for declared in &package.manifest.internals {
        internals::check(&entries, &package.manifest.id, declared).map_err(|e| e.to_string())?;
    }
    println!(
        "INTERNALS OK: {} declarations accepted for {}",
        package.manifest.internals.len(),
        package.manifest.id
    );
    Ok(())
}

fn main() {
    if let Err(error) = check() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
