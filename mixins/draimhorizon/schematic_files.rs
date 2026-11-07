// Bounded, background-only filesystem and GZip access for schematic imports.
use crate::game::claim_borders_schematic as model;
use std::{
    io::Read,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};
pub enum Response {
    Files(Vec<String>),
    Loaded(model::Blueprint),
}
pub fn spawn(
    folder: PathBuf,
    file: Option<String>,
    origin: [i32; 3],
    rotation: u8,
) -> Result<Receiver<Result<Response, String>>, String> {
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("horizon-schematic-import".into())
        .spawn(move || {
            horizon_threads::idle_current();
            let result = (|| {
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let folder = folder.canonicalize().map_err(|e| e.to_string())?;
                if let Some(file) = file {
                    if !model::safe_filename(&file) {
                        return Err("invalid schematic filename".into());
                    }
                    let path = folder
                        .join(file)
                        .canonicalize()
                        .map_err(|e| e.to_string())?;
                    if !path.starts_with(&folder) {
                        return Err("schematic path leaves its directory".into());
                    }
                    let reader = std::fs::File::open(path).map_err(|e| e.to_string())?;
                    if reader.metadata().map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
                        return Err("compressed schematic exceeds 8 MiB".into());
                    }
                    let mut bytes = Vec::new();
                    reader
                        .take(8 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    if bytes.len() > 8 * 1024 * 1024 {
                        return Err("compressed schematic exceeds 8 MiB".into());
                    }
                    let data = horizon_world::compression::Compression::Gzip
                        .decode(&bytes, model::MAX_BYTES)
                        .map_err(|e| e.to_string())?;
                    let document = model::decode(&data)?.transformed(origin, rotation)?;
                    if document.data_version > horizon_world::DATA_VERSION {
                        return Err("schematic data version is newer than this client".into());
                    }
                    Ok(Response::Loaded(document))
                } else {
                    let mut files = Vec::new();
                    for entry in std::fs::read_dir(folder)
                        .map_err(|e| e.to_string())?
                        .take(256)
                    {
                        let entry = entry.map_err(|e| e.to_string())?;
                        if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                            let name = entry.file_name().to_string_lossy().into_owned();
                            if model::safe_filename(&name) {
                                files.push(name);
                            }
                        }
                    }
                    files.sort();
                    files.truncate(64);
                    Ok(Response::Files(files))
                }
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    Ok(rx)
}
