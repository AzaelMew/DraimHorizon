// Local image work and schematic writes run on one bounded worker, never the render thread.
use crate::game::{Game, claim_borders_map_art as art, claim_borders_schematic as schema};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver},
    },
};

use art::View;
enum Reply {
    Generated(Arc<art::Plan>),
    Saved(String),
}
#[derive(Default)]
struct State {
    scope: String,
    view: View,
    worker: Option<Receiver<Result<Reply, String>>>,
    origin: [i32; 3],
}
static STATE: Mutex<Option<State>> = Mutex::new(None);
fn scope(game: &Game<'_>) -> String {
    crate::game::claim_borders_navigation_view::scope(game).unwrap_or_default()
}
pub(super) fn snapshot(game: &mut Game<'_>) -> View {
    let current = scope(game);
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.get_or_insert_with(State::default);
    if s.scope != current {
        // Keep the receiver until the old worker exits, but never apply its result to this world.
        s.view = View::default();
        if s.worker.is_none() {
            s.scope = current.clone();
        }
    }
    if let Some(result) = s.worker.as_ref().and_then(|rx| match rx.try_recv() {
        Ok(result) => Some(result),
        Err(mpsc::TryRecvError::Disconnected) => {
            Some(Err("Image worker stopped / Обработка прервана".into()))
        }
        Err(mpsc::TryRecvError::Empty) => None,
    }) {
        s.worker = None;
        s.view.busy = false;
        if s.scope == current {
            match result {
                Ok(Reply::Generated(plan)) => {
                    s.view.status = format!(
                        "{}×{}×{} · {} blocks / блоков",
                        plan.width,
                        plan.height,
                        plan.depth,
                        plan.document.blocks.len()
                    );
                    s.view.plan = Some(plan);
                }
                Ok(Reply::Saved(name)) => {
                    s.view.status = format!("Saved / Сохранено: {name}");
                    crate::game::claim_borders_schematic_view::preview(game, name, s.origin, 0);
                    let _ = crate::game::claim_borders_workshop_view::refresh(game);
                }
                Err(error) => s.view.status = error,
            }
        } else {
            s.scope = current;
        }
    }
    s.view.busy = s.worker.is_some();
    s.view.clone()
}
pub(super) fn folder(game: &Game<'_>) -> PathBuf {
    crate::game::claim_borders_workshop_view::folder(game).join("map-art-images")
}
fn material(game: &Game<'_>, name: &str) -> Result<art::Material, String> {
    let state = schema::BlockState::parse(name)?;
    if !state.properties.is_empty() {
        return Err("Use block names without properties / Укажите имена блоков без свойств".into());
    }
    let id = game
        .pack
        .assets
        .registry
        .default_state(&state.name)
        .ok_or_else(|| format!("Unknown block / Неизвестный блок: {name}"))?;
    let facts = game.res.facts().get(id);
    let color = game
        .block_state_map_color(id)
        .filter(|c| *c > 0 && *c != 12)
        .ok_or("Block has no supported map colour / Нет подходящего цвета карты")?;
    if (!facts.full_collision && !state.name.ends_with("_carpet"))
        || facts.block_entity
        || facts.fluid_level > 0
        || facts.waterlogged
        || facts.item.is_empty()
        || facts.item.as_ref() == "minecraft:air"
        || crate::game::claim_borders_construction::gravity(&state.name)
    {
        return Err(format!(
            "Unsupported material / Неподходящий материал: {name}"
        ));
    }
    let mut shades = [[0; 3]; 3];
    for (i, shade) in shades.iter_mut().enumerate() {
        let rgba = game
            .block_state_map_color_rgba(color * 4 + i as u8)
            .ok_or("Missing map colour")?;
        shade.copy_from_slice(&rgba[..3]);
    }
    Ok(art::Material { state, shades })
}
pub(super) fn generate(game: &Game<'_>, request: art::Request) -> Result<(), String> {
    let names = request.names()?;
    request.background_rgb()?;
    let palette = names
        .iter()
        .map(|name| material(game, name))
        .collect::<Result<Vec<_>, _>>()?;
    let floor = material(game, &request.floor)?.state;
    let floor_id = game
        .pack
        .assets
        .registry
        .default_state(&floor.name)
        .ok_or("Unknown floor")?;
    if game.res.facts().get(floor_id).sturdy_full & 2 == 0 {
        return Err(
            "Floor needs a solid upper face / У основания должна быть твёрдая верхняя грань".into(),
        );
    }
    let raw = request.image.trim().trim_matches('"');
    let input = PathBuf::from(raw);
    let path = if input.is_absolute() {
        input
    } else {
        folder(game).join(input)
    };
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if raw.is_empty() || !matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
        return Err("Choose a PNG/JPG file / Укажите файл PNG/JPG".into());
    }
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard.get_or_insert_with(State::default);
    if s.worker.is_some() {
        return Err("Please wait / Подождите завершения".into());
    }
    s.scope = scope(game);
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("draimhorizon-map-art".into())
        .spawn(move || {
            horizon_threads::idle_current();
            let result = (|| {
                let reader = std::fs::File::open(path).map_err(|e| e.to_string())?;
                let mut bytes = Vec::new();
                reader
                    .take(16 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 16 * 1024 * 1024 {
                    return Err("Image exceeds 16 MiB / Картинка больше 16 МиБ".into());
                }
                let (w, h, rgba) =
                    horizon_client::hud::decode_image_bytes(&bytes).map_err(|e| e.to_string())?;
                art::generate(request, [w as usize, h as usize], &rgba, &palette, floor)
                    .map(|plan| Reply::Generated(Arc::new(plan)))
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    s.worker = Some(rx);
    s.view.busy = true;
    s.view.plan = None;
    s.view.status = "Generating… / Генерация…".into();
    Ok(())
}
pub(super) fn place(game: &Game<'_>, request: &art::Request) -> Result<(), String> {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let s = guard
        .as_mut()
        .ok_or("Generate a preview first / Сначала создайте предпросмотр")?;
    if s.worker.is_some() || s.scope != scope(game) {
        return Err("Please wait / Подождите".into());
    }
    let plan = s
        .view
        .plan
        .clone()
        .filter(|p| &p.request == request)
        .ok_or("Settings changed: generate again / Настройки изменились: повторите генерацию")?;
    let p = game
        .native_player()
        .ok_or("Join a world first / Сначала зайдите в мир")?;
    // Scale-zero map boundaries: north-west corner is 128*k - 64.
    s.origin = [
        ((p.x.floor() as i32 + 64).div_euclid(128)) * 128 - 64,
        p.y.floor() as i32,
        ((p.z.floor() as i32 + 64).div_euclid(128)) * 128 - 64,
    ];
    let folder = crate::game::claim_borders_workshop_view::folder(game);
    let version = horizon_world::DATA_VERSION;
    let (tx, rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("draimhorizon-map-art-save".into())
        .spawn(move || {
            horizon_threads::idle_current();
            let result = (|| {
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?;
                let name = format!("map-art-{}.litematic", stamp.as_millis());
                let meta = crate::game::claim_borders_litematica_writer::Metadata {
                    name: name.clone(),
                    author: String::new(),
                    description: "Local map-art generator".into(),
                    time_ms: stamp.as_millis() as i64,
                    data_version: version,
                };
                let nbt = crate::game::claim_borders_litematica_writer::write(
                    &meta,
                    &art::regions(&plan)?,
                );
                let bytes = horizon_world::compression::Compression::Gzip
                    .encode(&nbt)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 8 * 1024 * 1024 {
                    return Err("Schematic exceeds import limit".into());
                }
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(folder.join(&name))
                    .map_err(|e| e.to_string())?;
                output.write_all(&bytes).map_err(|e| e.to_string())?;
                Ok(Reply::Saved(name))
            })();
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
    s.worker = Some(rx);
    s.view.busy = true;
    s.view.status = "Saving… / Сохранение…".into();
    Ok(())
}
pub(super) fn materials(game: &mut Game<'_>) -> Result<(), String> {
    use horizon_client::clipboard::Clipboard;
    let guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let plan = guard
        .as_ref()
        .filter(|s| s.scope == scope(game))
        .and_then(|s| s.view.plan.as_ref())
        .ok_or("Generate first / Сначала выполните генерацию")?;
    let text = plan
        .materials
        .iter()
        .map(|(name, n)| format!("{name}: {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    game.clipboard.set(&text);
    Ok(())
}
