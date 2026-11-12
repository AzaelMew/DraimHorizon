// Schematic previews shared with friends, in the game. The owner's shown previews of the world are
// listed to every friend online (`[DHS1]` frames over the encrypted relay, again when they change
// and when a friend comes online); a friend without a file asks for it and gets it in pieces,
// checked against its hash and kept by content in `shared-schematics/`. A friend sees the owner's
// previews (ghost blocks and box, drawn by `schematic_view`) where the owner placed them, and can
// only show or hide each on their own screen; the owner moves, turns, hides or removes them.
// Files are read and written on threads of their own, never on the game thread.
use crate::game::{
    Game, claim_borders as claims, claim_borders_client as client,
    claim_borders_schematic_places as places, claim_borders_schematic_share as model,
    claim_borders_schematic_view as view, claim_borders_workshop_view as workshop,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, mpsc},
    time::{Duration, Instant},
};
/// A friend's previews, as last listed by them.
struct Friend {
    name: String,
    items: Vec<model::Listed>,
}
enum Read {
    Pending(mpsc::Receiver<Result<Arc<Vec<u8>>, String>>),
    Ready(Arc<Vec<u8>>, u64),
    Failed,
}
#[derive(Default)]
struct State {
    // Owner side.
    /// The owner's files as read for sharing, by file name.
    files: HashMap<String, Read>,
    /// The list last made, and per friend (relay code) the list sent to them.
    list: Vec<model::Listed>,
    sent: BTreeMap<String, Vec<model::Listed>>,
    last_list: Option<Instant>,
    /// Pieces asked for: (friend, file hash, next piece), sent a few a second.
    queue: Vec<(String, u64, u32)>,
    last_piece: Option<Instant>,
    // Friend side.
    friends: BTreeMap<u128, Friend>,
    /// Files kept (by cache name), read from the folder once.
    have: BTreeSet<String>,
    scanned: Option<mpsc::Receiver<BTreeSet<String>>>,
    downloads: HashMap<u64, (model::Download, String, u128)>,
    last_want: Option<Instant>,
    /// Previews this player hid ("uuid:id").
    hidden: Option<BTreeSet<String>>,
}
fn state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    f(&mut S
        .get_or_init(|| Mutex::new(State::default()))
        .lock()
        .unwrap_or_else(|e| e.into_inner()))
}
const HIDDEN: &str = "schematic_friend_hidden";
pub(super) fn shared_folder(game: &Game<'_>) -> PathBuf {
    game.native_data_dir(claims::SETTINGS_ID)
        .join("shared-schematics")
}
fn hidden_key(owner: u128, id: u32) -> String {
    format!("{owner:032x}:{id}")
}
fn load_hidden(game: &Game<'_>, s: &mut State) {
    if s.hidden.is_none() {
        s.hidden = Some(
            game.native_setting(claims::SETTINGS_ID, HIDDEN)
                .map(|v| v.lines().map(str::to_owned).take(256).collect())
                .unwrap_or_default(),
        );
    }
}
fn dimension(game: &Game<'_>) -> Option<String> {
    let p = game.native_player()?;
    Some(if p.dimension.contains(':') {
        p.dimension.to_string()
    } else {
        format!("minecraft:{}", p.dimension)
    })
}
/// The owner's number for a preview: from its file, and which of the previews of that file it is.
fn preview_id(file: &str, nth: usize) -> u32 {
    (model::content_hash(file.as_bytes()) as u32) ^ (nth as u32).wrapping_mul(0x9e37_79b9)
}
/// The friends' previews to draw here: (cache file name, placement), shown and kept.
pub(super) fn shown(game: &Game<'_>) -> Vec<(String, places::Placement)> {
    let Some(here) = dimension(game) else {
        return Vec::new();
    };
    state(|s| {
        load_hidden(game, s);
        let hidden = s.hidden.clone().unwrap_or_default();
        s.friends
            .iter()
            .flat_map(|(uuid, f)| f.items.iter().map(move |i| (*uuid, i)))
            .filter(|(uuid, i)| {
                i.dimension == here
                    && s.have.contains(&i.cache_name())
                    && !hidden.contains(&hidden_key(*uuid, i.id))
            })
            .map(|(_, i)| {
                (
                    i.cache_name(),
                    places::Placement {
                        scope: String::new(),
                        file: i.cache_name(),
                        origin: i.origin,
                        rotation: i.rotation,
                        visible: true,
                    },
                )
            })
            .collect()
    })
}
/// The friends' previews for the Schematics tab: label (file, friend, place) and whether shown.
pub(super) fn rows(game: &Game<'_>) -> Vec<(String, bool)> {
    let here = dimension(game).unwrap_or_default();
    state(|s| {
        load_hidden(game, s);
        let hidden = s.hidden.clone().unwrap_or_default();
        let mut rows = Vec::new();
        for (uuid, f) in &s.friends {
            for i in f.items.iter().filter(|i| i.dimension == here) {
                let [x, y, z] = i.origin;
                let state = if s.have.contains(&i.cache_name()) {
                    String::new()
                } else {
                    let got = s
                        .downloads
                        .get(&i.hash)
                        .and_then(|(d, _, _)| d.next_missing())
                        .unwrap_or(0);
                    format!(" · {}/{}", got, i.pieces())
                };
                rows.push((
                    format!("{} · {} · {x} {y} {z}{state}", i.name, f.name),
                    !hidden.contains(&hidden_key(*uuid, i.id)),
                ));
            }
        }
        rows
    })
}
/// Shows or hides on this screen the friend's preview at `index` of `rows`.
pub(super) fn toggle(game: &Game<'_>, index: usize) {
    let here = dimension(game).unwrap_or_default();
    state(|s| {
        load_hidden(game, s);
        let Some(key) = s
            .friends
            .iter()
            .flat_map(|(uuid, f)| f.items.iter().map(move |i| (*uuid, i)))
            .filter(|(_, i)| i.dimension == here)
            .nth(index)
            .map(|(uuid, i)| hidden_key(uuid, i.id))
        else {
            return;
        };
        let hidden = s.hidden.get_or_insert_with(BTreeSet::new);
        if !hidden.remove(&key) {
            hidden.insert(key);
        }
        let value = hidden.iter().cloned().collect::<Vec<_>>().join("\n");
        game.native_set_setting(claims::SETTINGS_ID, HIDDEN, &value);
    });
}
/// Files that could not be read for sharing are read again (Refresh in the Schematics tab).
pub(super) fn forget_files() {
    state(|s| s.files.clear());
}
/// A frame from a friend (checked by the client: a pinned friend, its only session).
pub(super) fn receive(peer: &claim_borders_relay_client::Peer, body: &str) {
    let Ok(frame) = model::Frame::decode(body) else {
        return;
    };
    state(|s| match frame {
        model::Frame::List(items) => {
            // Downloads of files no longer listed by anyone are dropped.
            s.friends.insert(
                peer.uuid,
                Friend {
                    name: peer.name.clone(),
                    items,
                },
            );
            let listed: BTreeSet<u64> = s
                .friends
                .values()
                .flat_map(|f| f.items.iter().map(|i| i.hash))
                .collect();
            s.downloads.retain(|hash, _| listed.contains(hash));
        }
        model::Frame::Want { hash, from } => {
            if s.list.iter().any(|i| i.hash == hash) && s.queue.len() < 16 {
                s.queue
                    .retain(|(code, h, _)| !(code == &peer.code && *h == hash));
                s.queue.push((peer.code.clone(), hash, from));
            }
        }
        model::Frame::Piece {
            hash,
            index,
            count,
            data,
        } => {
            let Some((download, name, owner)) = s.downloads.get_mut(&hash) else {
                return;
            };
            // Pieces only from the friend who listed the file.
            if *owner != peer.uuid {
                return;
            }
            if let Some(bytes) = download.add(index, count, data) {
                let name = name.clone();
                s.downloads.remove(&hash);
                WRITES.with_sender(|tx| {
                    let _ = tx.send((name, bytes));
                });
            }
        }
    });
}
/// Kept files written by a thread of their own; each written name comes back on `written`.
struct Writes {
    tx: Mutex<Option<mpsc::Sender<(String, Vec<u8>)>>>,
    written: Mutex<Option<mpsc::Receiver<String>>>,
    folder: OnceLock<PathBuf>,
}
static WRITES: Writes = Writes {
    tx: Mutex::new(None),
    written: Mutex::new(None),
    folder: OnceLock::new(),
};
impl Writes {
    fn start(&self, folder: PathBuf) {
        if self.folder.set(folder.clone()).is_err() {
            return;
        }
        let (tx, rx) = mpsc::channel::<(String, Vec<u8>)>();
        let (done_tx, done_rx) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("horizon-shared-schematics".into())
            .spawn(move || {
                horizon_threads::idle_current();
                while let Ok((name, bytes)) = rx.recv() {
                    let ok = std::fs::create_dir_all(&folder).is_ok() && {
                        let pending = folder.join(format!("{name}.pending"));
                        std::fs::write(&pending, &bytes).is_ok()
                            && std::fs::rename(&pending, folder.join(&name)).is_ok()
                    };
                    if ok {
                        let _ = done_tx.send(name);
                    }
                }
            });
        *self.tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
        *self.written.lock().unwrap_or_else(|e| e.into_inner()) = Some(done_rx);
    }
    fn with_sender(&self, f: impl FnOnce(&mpsc::Sender<(String, Vec<u8>)>)) {
        if let Some(tx) = self.tx.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            f(tx);
        }
    }
    fn written(&self) -> Vec<String> {
        self.written
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default()
    }
}
/// The connection went or the world changed: friends' lists are forgotten (they list again).
pub(super) fn reset() {
    state(|s| {
        s.friends.clear();
        s.downloads.clear();
        s.sent.clear();
        s.queue.clear();
    });
}
/// The relay connected again: every friend gets the list once more.
pub(super) fn network_reset() {
    state(|s| s.sent.clear());
}
/// Every tick: lists the owner's shown previews to friends, sends pieces asked for, asks for
/// missing files of friends' previews.
pub(super) fn frame(game: &mut Game<'_>) {
    if !game.native_on(claims::ID) || game.remote.is_none() {
        return;
    }
    let folder = shared_folder(game);
    WRITES.start(folder.clone());
    // The kept files, read from the folder once (on a thread).
    state(|s| {
        if s.scanned.is_none() && s.have.is_empty() {
            let (tx, rx) = mpsc::channel();
            let dir = folder.clone();
            let _ = std::thread::Builder::new()
                .name("horizon-shared-schematics-scan".into())
                .spawn(move || {
                    let names = std::fs::read_dir(&dir)
                        .map(|d| {
                            d.flatten()
                                .filter_map(|e| e.file_name().into_string().ok())
                                .filter(|n| n.ends_with(".schem") || n.ends_with(".litematic"))
                                .take(4096)
                                .collect()
                        })
                        .unwrap_or_default();
                    let _ = tx.send(names);
                });
            s.scanned = Some(rx);
        }
        if let Some(names) = s.scanned.as_ref().and_then(|rx| rx.try_recv().ok()) {
            s.have.extend(names);
        }
        s.have.extend(WRITES.written());
    });
    let Some(here) = dimension(game) else {
        return;
    };
    // The owner's shown previews of this world, with their files read for sharing.
    let own = view::own_shown(game);
    let source = workshop::folder(game);
    let list = state(|s| {
        let mut list = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for pl in own.iter().rev().take(model::MAX_LISTED) {
            let nth = seen.entry(pl.file.clone()).or_insert(0);
            let id = preview_id(&pl.file, *nth);
            *nth += 1;
            let read = s.files.entry(pl.file.clone()).or_insert_with(|| {
                let (tx, rx) = mpsc::channel();
                let path = source.join(&pl.file);
                let _ = std::thread::Builder::new()
                    .name("horizon-shared-schematic-read".into())
                    .spawn(move || {
                        let read = std::fs::metadata(&path)
                            .map_err(|e| e.to_string())
                            .and_then(|m| {
                                if m.len() as usize > model::MAX_FILE {
                                    Err("too large to share".into())
                                } else {
                                    std::fs::read(&path).map_err(|e| e.to_string())
                                }
                            });
                        let _ = tx.send(read.map(Arc::new));
                    });
                Read::Pending(rx)
            });
            if let Read::Pending(rx) = read {
                match rx.try_recv() {
                    Ok(Ok(bytes)) => {
                        let hash = model::content_hash(&bytes);
                        *read = Read::Ready(bytes, hash);
                    }
                    Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => *read = Read::Failed,
                    Err(mpsc::TryRecvError::Empty) => {}
                }
            }
            if let Read::Ready(bytes, hash) = read
                && !bytes.is_empty()
            {
                list.push(model::Listed {
                    id,
                    name: pl.file.chars().take(48).collect(),
                    dimension: here.clone(),
                    origin: pl.origin,
                    rotation: pl.rotation,
                    hash: *hash,
                    size: bytes.len() as u32,
                });
            }
        }
        s.list = list.clone();
        list
    });
    let friends = client::friend_peers(game);
    let now = Instant::now();
    // The list, to each friend that has not got this one (one frame a second).
    let out = state(|s| {
        s.sent
            .retain(|code, _| friends.iter().any(|f| &f.code == code));
        s.queue
            .retain(|(code, _, _)| friends.iter().any(|f| &f.code == code));
        // A friend gone (offline, no longer a friend): their previews go with them.
        s.friends
            .retain(|uuid, _| friends.iter().any(|f| f.uuid == *uuid));
        if s.last_list
            .is_some_and(|t| now - t < Duration::from_secs(1))
        {
            return None;
        }
        let friend = friends
            .iter()
            .find(|f| s.sent.get(&f.code) != Some(&list))?;
        s.sent.insert(friend.code.clone(), list.clone());
        s.last_list = Some(now);
        Some((
            friend.code.clone(),
            model::Frame::List(list.clone()).encode(),
        ))
    });
    if let Some((code, body)) = out
        && !client::claim_send(&code, &body)
    {
        state(|s| {
            s.sent.remove(&code);
        });
    }
    // Pieces asked for, four a second.
    let piece = state(|s| {
        if s.last_piece
            .is_some_and(|t| now - t < Duration::from_millis(250))
        {
            return None;
        }
        let (code, hash, from) = s.queue.first().cloned()?;
        let bytes = s.files.values().find_map(|r| match r {
            Read::Ready(b, h) if *h == hash => Some(b.clone()),
            _ => None,
        });
        let Some(bytes) = bytes else {
            s.queue.remove(0);
            return None;
        };
        let frame = model::pieces(&bytes, hash, from).next();
        match frame {
            Some(frame) => {
                s.queue[0].2 = from + 1;
                s.last_piece = Some(now);
                Some((code, frame.encode()))
            }
            None => {
                s.queue.remove(0);
                None
            }
        }
    });
    if let Some((code, body)) = piece {
        let _ = client::claim_send(&code, &body);
    }
    // Missing files of friends' previews here: asked for every two seconds, from the first piece
    // still missing (a piece lost on the way is asked for again).
    let want = state(|s| {
        if s.last_want
            .is_some_and(|t| now - t < Duration::from_secs(2))
        {
            return None;
        }
        let missing = s
            .friends
            .iter()
            .flat_map(|(uuid, f)| f.items.iter().map(move |i| (*uuid, i)))
            .find(|(_, i)| i.dimension == here && !s.have.contains(&i.cache_name()))
            .map(|(uuid, i)| (uuid, i.clone()))?;
        let (uuid, item) = missing;
        let (download, _, _) = s
            .downloads
            .entry(item.hash)
            .or_insert_with(|| (model::Download::new(&item), item.cache_name(), uuid));
        let from = download.next_missing()?;
        s.last_want = Some(now);
        let code = friends.iter().find(|f| f.uuid == uuid)?.code.clone();
        Some((
            code,
            model::Frame::Want {
                hash: item.hash,
                from,
            }
            .encode(),
        ))
    });
    if let Some((code, body)) = want {
        let _ = client::claim_send(&code, &body);
    }
}
