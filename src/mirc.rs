use debug_print::debug_eprintln as deprintln;
use std::sync::Mutex;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;
use windows::core::PCWSTR;
use windows::{Win32::Foundation::HWND, core::BOOL};

#[repr(C)]
#[allow(non_snake_case)]
#[derive(Clone)]
pub struct LOADINFO {
    pub mVersion: u32,
    pub mHwnd: HWND,
    pub mKeep: BOOL,
    pub mUnicode: BOOL,
    pub mBeta: u32,
    pub mBytes: u32,
}

// Implement Send and Sync for LOADINFO
unsafe impl Send for LOADINFO {}
unsafe impl Sync for LOADINFO {}

#[repr(i32)]
pub enum MircReturn {
    Halt = 0,
    Continue = 1,
    Command = 2,
    Result = 3,
}

#[repr(i32)]
#[allow(dead_code)]
pub enum TimeoutReason {
    Unload = 0,
    Inactive = 1,
    Exit = 2,
}

pub static M_LOADINFO: Mutex<LOADINFO> = Mutex::new(LOADINFO {
    mVersion: 0,
    mHwnd: HWND(std::ptr::null_mut()),
    mKeep: BOOL(0),
    mUnicode: BOOL(0),
    mBeta: 0,
    mBytes: 0,
});

pub fn LoadDll(li: *mut LOADINFO) {
    deprintln!("Loading DLL");
    unsafe {
        deprintln!("Set mKeep = 1, mUnicode = 1");
        (*li).mKeep = BOOL(1); // Keep the DLL loaded
        (*li).mUnicode = BOOL(1); // Use Unicode

        deprintln!("Store LOADINFO");
        *M_LOADINFO.lock().unwrap() = (*li).clone();
    }
}

pub fn UnloadDll(reason: TimeoutReason) -> i32 {
    match reason {
        TimeoutReason::Unload => 0,
        TimeoutReason::Inactive => 1,
        TimeoutReason::Exit => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "stdcall" fn version(
    _m_wnd: HWND,
    _a_wnd: HWND,
    data: PCWSTR,
    _parms: PCWSTR,
    _show: BOOL,
    _nopause: BOOL,
) -> MircReturn {
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    let author = env!("CARGO_PKG_AUTHORS");
    let arch = std::env::consts::ARCH;

    let m_client = get_client_name();
    let loadinfo = M_LOADINFO.lock().unwrap();

    let m_version = loadinfo.mVersion;
    let m_version_low = m_version & 0xFFFF;
    let m_version_high = m_version >> 16;

    let input = format!(
        // Format the string with the following variables
        "{} {} by {} on {} v{}.{} ({})\0",
        name, version, author, m_client, m_version_low, m_version_high, arch
    );
    let message: Vec<u16> = input.encode_utf16().collect(); // Convert to UTF-16
    unsafe {
        std::ptr::copy_nonoverlapping(message.as_ptr(), data.0 as *mut u16, message.len() + 1);
    }
    MircReturn::Result // Return the result to mIRC
}

struct ClientName;

impl ClientName {
    const MIRC: &'static str = "mIRC";
    const ADIIRC: &'static str = "AdiIRC";
    const UNKNOWN: &'static str = "Unknown";
}

pub fn get_client_name() -> String {
    let hwnd = get_loadinfo().mHwnd;
    let mut class_name: Vec<u16> = vec![0; 256];
    unsafe {
        if GetClassNameW(hwnd, &mut class_name) > 0 {
            let class_name = PCWSTR(class_name.as_ptr()).to_string().unwrap_or_default(); // Convert to String
            if class_name == ClientName::MIRC {
                return class_name;
            } else {
                return ClientName::ADIIRC.to_string();
            }
        } else {
            return ClientName::UNKNOWN.to_string();
        }
    }
}

pub fn get_loadinfo() -> LOADINFO {
    M_LOADINFO.lock().unwrap().clone()
}

pub fn is_dllcall() -> bool {
    let hwnd = get_loadinfo().mHwnd;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, None) };
    let current_thread_id = unsafe { GetCurrentThreadId() };
    thread_id != current_thread_id
}
