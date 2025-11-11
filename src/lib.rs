mod mirc;

use debug_print::debug_eprintln as deprintln;
use mIRC::{LOADINFO, MircReturn, TimeoutReason};
use mirc as mIRC;
use std::sync::atomic::{AtomicI32, Ordering};
use windows::Foundation::TypedEventHandler;
use windows::Media::{
    Control::{
        CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession,
        GlobalSystemMediaTransportControlsSessionManager,
        GlobalSystemMediaTransportControlsSessionMediaProperties, MediaPropertiesChangedEventArgs,
    },
    MediaPlaybackType,
};
use windows::core::{PCWSTR, w};
use windows::{Win32::Foundation::HWND, core::BOOL};

// Global variables
static M_WAITING_FOR_MEDIA_CHANGE: AtomicI32 = AtomicI32::new(0);

#[unsafe(no_mangle)]
pub extern "system" fn wait_for_media(
    _m_wnd: HWND,
    _a_wnd: HWND,
    data: PCWSTR,
    _parms: PCWSTR,
    _show: BOOL,
    _nopause: BOOL,
) -> MircReturn {
    if !mIRC::is_dllcall() {
        // Synchronous calls using $dll or /dll block the GUI thread, prevent this.
        deprintln!("Prevented attempt to block GUI thread in wait_for_media()");
        let result = w!(
            "!echo -estgc info * The DLL routine you attempted to use is not allowed on the GUI thread, try using $ $+ dllcall() instead."
        );
        unsafe {
            std::ptr::copy_nonoverlapping(result.as_ptr(), data.0 as *mut u16, result.len() + 1);
        }
        return MircReturn::Command;
    }

    if M_WAITING_FOR_MEDIA_CHANGE.load(Ordering::SeqCst) != 0 {
        // If we're already looping in another thread, return immediately.
        return MircReturn::Continue;
    }

    M_WAITING_FOR_MEDIA_CHANGE.store(1, Ordering::SeqCst);
    while M_WAITING_FOR_MEDIA_CHANGE.load(Ordering::SeqCst) != 0 {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    MircReturn::Continue // Return the result to mIRC
}

#[unsafe(no_mangle)]
pub extern "system" fn halt(
    _m_wnd: HWND,
    _a_wnd: HWND,
    data: PCWSTR,
    _parms: PCWSTR,
    _show: BOOL,
    _nopause: BOOL,
) -> MircReturn {
    M_WAITING_FOR_MEDIA_CHANGE.store(0, Ordering::SeqCst);
    let result = w!("!echo -estgc info * All $dllcall() calls have completed.");
    unsafe {
        std::ptr::copy_nonoverlapping(result.as_ptr(), data.0 as *mut u16, result.len() + 1);
    }
    MircReturn::Command // Return the result to mIRC
}

#[unsafe(no_mangle)]
pub extern "system" fn LoadDll(li: *mut LOADINFO) {
    mIRC::LoadDll(li);
    watch_media_session_changes();
}

#[unsafe(no_mangle)]
pub extern "system" fn UnloadDll(reason: TimeoutReason) -> i32 {
    mIRC::UnloadDll(reason)
}

static mut CURRENT_SESSION: Option<GlobalSystemMediaTransportControlsSession> = None;
static mut CURRENT_MEDIA_PROPERTIES: Option<
    GlobalSystemMediaTransportControlsSessionMediaProperties,
> = None;

// Note: When we change session, we're not getting the current media.
fn watch_media_session_changes() {
    let session_manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
        .unwrap()
        .GetResults()
        .unwrap();
    let session_manager_clone = session_manager.clone();

    let current_session_changed = TypedEventHandler::<
        GlobalSystemMediaTransportControlsSessionManager,
        CurrentSessionChangedEventArgs,
    >::new(move |_, _| {
        deprintln!("Session changed");
        let session_manager = session_manager_clone.clone();
        match (session_manager.GetCurrentSession()) {
            Ok(session) => unsafe {
                deprintln!("New session found");
                // TODO: Do we need to remove old session handler? Or does it get removed automatically?
                //let session_clone = session.clone();
                let session_clone = session.clone();
                let current_media_properties_changed = TypedEventHandler::<
                    GlobalSystemMediaTransportControlsSession,
                    MediaPropertiesChangedEventArgs,
                >::new({
                    move |_, _| {
                        println!("Media properties changed");
                        let media_properties = session_clone
                            .TryGetMediaPropertiesAsync()
                            .unwrap()
                            .GetResults();
                        match media_properties {
                            Ok(media_properties) => {
                                deprintln!("New media properties found");
                                CURRENT_MEDIA_PROPERTIES = Some(media_properties);
                            }
                            Err(_) => {
                                deprintln!("Failed to get media properties");
                                CURRENT_MEDIA_PROPERTIES = None;
                            }
                        }
                        M_WAITING_FOR_MEDIA_CHANGE.store(0, Ordering::SeqCst);
                        Ok(())
                    }
                });
                let _ = session.MediaPropertiesChanged(&current_media_properties_changed);
                let _ = current_media_properties_changed.Invoke(&session, None);
                CURRENT_SESSION = Some(session);
            },
            Err(_) => unsafe {
                deprintln!("No session found");
                CURRENT_SESSION = None;
                CURRENT_MEDIA_PROPERTIES = None;
            },
        }
        Ok(())
    });
    let _ = session_manager.CurrentSessionChanged(&current_session_changed);
    let _ = current_session_changed.Invoke(&session_manager, None);
}

#[unsafe(no_mangle)]
pub extern "system" fn title(
    _m_wnd: HWND,
    _a_wnd: HWND,
    data: PCWSTR,
    _parms: PCWSTR,
    _show: BOOL,
    _nopause: BOOL,
) -> MircReturn {
    let title = unsafe {
        match CURRENT_MEDIA_PROPERTIES {
            Some(ref media_properties) => media_properties.Title().unwrap(),
            None => windows::core::HSTRING::from("null"),
        }
    };
    unsafe {
        std::ptr::copy_nonoverlapping(title.as_ptr(), data.0 as *mut u16, title.len() + 1);
    }
    MircReturn::Result
}

#[unsafe(no_mangle)]
pub extern "system" fn artist(
    _m_wnd: HWND,
    _a_wnd: HWND,
    data: PCWSTR,
    _parms: PCWSTR,
    _show: BOOL,
    _nopause: BOOL,
) -> MircReturn {
    let artist = unsafe {
        match CURRENT_MEDIA_PROPERTIES {
            Some(ref media_properties) => media_properties.Artist().unwrap(),
            None => windows::core::HSTRING::from("null"),
        }
    };
    deprintln!("Artist: {:?} len: {}", artist, artist.len());
    unsafe {
        std::ptr::copy_nonoverlapping(artist.as_ptr(), data.0 as *mut u16, artist.len() + 1);
    }
    MircReturn::Result
}
