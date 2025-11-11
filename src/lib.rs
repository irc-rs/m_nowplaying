use mirust::mirust_fn;
use windows::{Win32::Foundation::HWND, core::BOOL};

use std::sync::{
    Condvar, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager, MediaPropertiesChangedEventArgs,
};
use windows::Media::MediaPlaybackType;
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

mod client;

// Small shared state used to coordinate wait_for_media/halt and expose metadata
#[derive(Default)]
struct MediaState {
    // Core metadata
    title: Option<String>,
    artist: Option<String>,
    album_title: Option<String>,
    album_artist: Option<String>,
    genres: Option<Vec<String>>,
    subtitle: Option<String>,
    track_number: Option<u32>,
    album_track_count: Option<u32>,
    playback_type: Option<String>,
    thumbnail_path: Option<String>,

    // Control
    version: u64,
    cancelled: bool,
}

static GLOBAL_MEDIA: OnceLock<(Mutex<MediaState>, Condvar)> = OnceLock::new();
static MEDIA_WATCHER_STARTED: OnceLock<()> = OnceLock::new();
static MEDIA_LISTENING: AtomicBool = AtomicBool::new(false);

#[derive(Default, Clone)]
struct MediaSnapshot {
    title: Option<String>,
    artist: Option<String>,
    album_title: Option<String>,
    album_artist: Option<String>,
    genres: Option<Vec<String>>,
    subtitle: Option<String>,
    track_number: Option<u32>,
    album_track_count: Option<u32>,
    playback_type: Option<String>,
    thumbnail_path: Option<String>,
}

fn any_changed<T: PartialEq>(a: &Option<T>, b: &Option<T>) -> bool {
    a != b
}

fn update_state_with(new: Option<MediaSnapshot>) {
    let (lock, cvar) =
        GLOBAL_MEDIA.get_or_init(|| (Mutex::new(MediaState::default()), Condvar::new()));

    let mut state = lock.lock().unwrap();
    match new {
        Some(newm) => {
            let mut changed = false;
            if any_changed(&state.title, &newm.title) {
                state.title = newm.title;
                changed = true;
            }
            if any_changed(&state.artist, &newm.artist) {
                state.artist = newm.artist;
                changed = true;
            }
            if any_changed(&state.album_title, &newm.album_title) {
                state.album_title = newm.album_title;
                changed = true;
            }
            if any_changed(&state.album_artist, &newm.album_artist) {
                state.album_artist = newm.album_artist;
                changed = true;
            }
            if any_changed(&state.genres, &newm.genres) {
                state.genres = newm.genres;
                changed = true;
            }
            if any_changed(&state.subtitle, &newm.subtitle) {
                state.subtitle = newm.subtitle;
                changed = true;
            }
            if any_changed(&state.track_number, &newm.track_number) {
                state.track_number = newm.track_number;
                changed = true;
            }
            if any_changed(&state.album_track_count, &newm.album_track_count) {
                state.album_track_count = newm.album_track_count;
                changed = true;
            }
            if any_changed(&state.playback_type, &newm.playback_type) {
                state.playback_type = newm.playback_type;
                changed = true;
            }
            if any_changed(&state.thumbnail_path, &newm.thumbnail_path) {
                state.thumbnail_path = newm.thumbnail_path;
                changed = true;
            }

            if changed {
                state.version = state.version.wrapping_add(1);
                state.cancelled = false;
                cvar.notify_all();
            }
        }
        None => {
            // No metadata available; avoid spurious wake-ups for None->None
            if state.title.is_some()
                || state.artist.is_some()
                || state.album_title.is_some()
                || state.album_artist.is_some()
                || state.genres.is_some()
                || state.subtitle.is_some()
                || state.track_number.is_some()
                || state.album_track_count.is_some()
                || state.playback_type.is_some()
                || state.thumbnail_path.is_some()
            {
                state.title = None;
                state.artist = None;
                state.album_title = None;
                state.album_artist = None;
                state.genres = None;
                state.subtitle = None;
                state.track_number = None;
                state.album_track_count = None;
                state.playback_type = None;
                state.thumbnail_path = None;
                state.version = state.version.wrapping_add(1);
                state.cancelled = false;
                cvar.notify_all();
            }
        }
    }
}

fn playback_type_to_string(pt: MediaPlaybackType) -> &'static str {
    match pt {
        MediaPlaybackType::Music => "Music",
        MediaPlaybackType::Video => "Video",
        MediaPlaybackType::Image => "Image",
        _ => "Unknown",
    }
}

// Returns the global (Mutex, Condvar), initializing to defaults if necessary
fn ensure_state() -> (&'static Mutex<MediaState>, &'static Condvar) {
    GLOBAL_MEDIA.get_or_init(|| (Mutex::new(MediaState::default()), Condvar::new()));
    // Once initialized, unwrap to refs
    let (m, c) = GLOBAL_MEDIA.get().unwrap();
    (m, c)
}

// Ensures watcher is running and returns a locked guard to the MediaState
// removed start_and_lock; accessors now avoid starting the watcher and check listening state

fn fetch_current(
    manager: &GlobalSystemMediaTransportControlsSessionManager,
) -> Option<MediaSnapshot> {
    if let Ok(session) = manager.GetCurrentSession() {
        if let Ok(props_op) = session.TryGetMediaPropertiesAsync() {
            // Wait for the async properties operation to complete (Completed == 1)
            loop {
                match props_op.Status() {
                    Ok(s) if s.0 == 1 => break,
                    Ok(_) => thread::sleep(Duration::from_millis(20)),
                    _ => return None,
                }
            }

            if let Ok(props) = props_op.GetResults() {
                let title = props.Title().unwrap_or_default().to_string();
                let artist = props.Artist().unwrap_or_default().to_string();
                let album_title = props.AlbumTitle().ok().map(|s| s.to_string());
                let album_artist = props.AlbumArtist().ok().map(|s| s.to_string());
                let subtitle = props.Subtitle().ok().map(|s| s.to_string());
                let track_number = props.TrackNumber().ok().map(|v| v as u32); // API returns i32
                // PlaybackType is an IReference<MediaPlaybackType>; use Value() accessor
                let playback_type = props
                    .PlaybackType()
                    .ok()
                    .and_then(|iref| iref.Value().ok())
                    .map(|p| playback_type_to_string(p).to_string());

                // Genres
                let genres = match props.Genres() {
                    Ok(gv) => {
                        let mut v = Vec::new();
                        if let Ok(sz) = gv.Size() {
                            let mut i = 0;
                            while i < sz {
                                if let Ok(item) = gv.GetAt(i) {
                                    v.push(item.to_string());
                                }
                                i += 1;
                            }
                        }
                        if v.is_empty() { None } else { Some(v) }
                    }
                    Err(_) => None,
                };

                // Treat empty metadata as None so transient states don't trigger wakeups
                if title.trim().is_empty() && artist.trim().is_empty() {
                    return None;
                }

                return Some(MediaSnapshot {
                    title: Some(title),
                    artist: Some(artist),
                    album_title,
                    album_artist,
                    genres,
                    subtitle,
                    track_number,
                    album_track_count: None, // Not provided by API
                    playback_type,
                    thumbnail_path: None, // Not implemented yet
                });
            }
        }
    }
    None
}

fn is_listening() -> bool {
    MEDIA_LISTENING.load(Ordering::SeqCst)
}

fn start_media_watcher() {
    if MEDIA_WATCHER_STARTED.get().is_some() {
        return;
    }

    MEDIA_WATCHER_STARTED.get_or_init(|| {
        thread::spawn(|| {
            // Initialize COM on this background thread
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }

            // Request the session manager. The windows crate gives us an IAsyncOperation; poll its status until completed.
            if let Ok(op) = GlobalSystemMediaTransportControlsSessionManager::RequestAsync() {
                loop {
                    match op.Status() {
                        Ok(s) if s.0 == 1 => break,
                        Ok(_) => thread::sleep(Duration::from_millis(20)),
                        _ => return,
                    }
                }

                if let Ok(manager) = op.GetResults() {
                    // Register for session changes. When the current session changes, fetch properties and update state.
                    let mgr_clone = manager.clone();
                    let handler = TypedEventHandler::<
                        GlobalSystemMediaTransportControlsSessionManager,
                        CurrentSessionChangedEventArgs,
                    >::new(move |_mgr, _args| {
                        if !is_listening() {
                            return Ok(());
                        }
                        if let Some(cur) = fetch_current(&mgr_clone) {
                            update_state_with(Some(cur));
                        } else {
                            update_state_with(None);
                        }
                        Ok(())
                    });
                    let _ = manager.CurrentSessionChanged(&handler);

                    // Register for media property changes on the current session (if present)
                    if let Ok(session) = manager.GetCurrentSession() {
                        let mgr_clone2 = manager.clone();
                        let handler = TypedEventHandler::<
                            GlobalSystemMediaTransportControlsSession,
                            MediaPropertiesChangedEventArgs,
                        >::new(move |_s, _args| {
                            if !is_listening() {
                                return Ok(());
                            }
                            if let Some(cur) = fetch_current(&mgr_clone2) {
                                update_state_with(Some(cur));
                            } else {
                                update_state_with(None);
                            }
                            Ok(())
                        });
                        let _ = session.MediaPropertiesChanged(&handler);
                    }

                    // Populate initial state so waiters have an initial baseline
                    if is_listening() {
                        if let Some(cur) = fetch_current(&manager) {
                            update_state_with(Some(cur));
                        } else {
                            update_state_with(None);
                        }
                    }

                    // Leave thread alive so handler tokens remain in scope and events keep firing
                    loop {
                        thread::sleep(Duration::from_secs(60));
                    }
                }
            }
        });
        ()
    });
}

#[mirust_fn(dllcall = true)]
pub extern "system" fn wait_for_media(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    let (lock, cvar) = ensure_state();
    MEDIA_LISTENING.store(true, Ordering::SeqCst);
    start_media_watcher();

    let mut state = lock.lock().unwrap();
    let initial_version = state.version;
    state.cancelled = false;

    while state.version == initial_version && !state.cancelled {
        state = cvar.wait(state).unwrap();
    }

    mirust::MircResult {
        code: 1,
        data: None,
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn halt(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    let (lock, cvar) = ensure_state();

    let mut state = lock.lock().unwrap();
    MEDIA_LISTENING.store(false, Ordering::SeqCst);
    state.cancelled = true;
    cvar.notify_all();

    mirust::MircResult {
        code: 3,
        data: Some("S_OK".to_string()),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn title(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    // Avoid duplicate content policy: return just the title, or empty if unknown
    let value = state
        .title
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();

    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn albumartist(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .album_artist
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn albumtitle(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .album_title
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn genres(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .genres
        .as_ref()
        .map(|v| v.join(", "))
        .unwrap_or_else(|| "".to_string());
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn playbacktype(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .playback_type
        .as_ref()
        .map(|s| s.as_str())
        .unwrap_or("");
    mirust::MircResult {
        code: 3,
        data: Some(value.to_string()),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn subtitle(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .subtitle
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn tracknumber(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .track_number
        .map(|n| n.to_string())
        .unwrap_or_else(|| "".to_string());
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn albumtrackcount(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .album_track_count
        .map(|n| n.to_string())
        .unwrap_or_else(|| "".to_string());
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn thumbnail(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    // Not implemented: return empty string or a path if we add extraction later
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state.thumbnail_path.clone().unwrap_or_default();
    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn artist(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    if !is_listening() {
        return mirust::MircResult {
            code: 3,
            data: Some(String::new()),
            parms: None,
        };
    }
    let (lock, _cvar) = ensure_state();
    let state = lock.lock().unwrap();
    let value = state
        .artist
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();

    mirust::MircResult {
        code: 3,
        data: Some(value),
        parms: None,
    }
}

#[mirust_fn]
pub extern "system" fn version(
    _m_wnd: HWND,
    _a_wnd: HWND,
    _data: String,
    _parms: String,
    _show: BOOL,
    _nopause: BOOL,
) -> mirust::MircResult {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    let arch = std::env::consts::ARCH;
    let m_client = client::get_name();
    let m_version = mirust::get_loadinfo().m_version;
    let m_version_low = m_version & 0xFFFF;
    let m_version_high = m_version >> 16;
    let data = format!(
        "{} {} on {} v{}.{} ({})",
        name, version, m_client, m_version_low, m_version_high, arch
    );
    mirust::MircResult {
        code: 3,
        data: Some(data),
        parms: None,
    }
}
