use mirust::get_loadinfo;
use windows::{Win32::UI::WindowsAndMessaging::GetClassNameW, core::PCWSTR};

struct ClientName;

impl ClientName {
    const MIRC: &'static str = "mIRC";
    const MIRC32: &'static str = "mIRC32";
    const ADIIRC: &'static str = "AdiIRC";
    const UNKNOWN: &'static str = "Unknown";
}

pub fn get_name() -> String {
    let hwnd = get_loadinfo().m_hwnd;
    let mut class_name: Vec<u16> = vec![0; 256];
    unsafe {
        if GetClassNameW(hwnd, &mut class_name) > 0 {
            let class_name = PCWSTR(class_name.as_ptr()).to_string().unwrap_or_default(); // Convert to String
            if class_name == ClientName::MIRC {
                return class_name;
            } else if class_name == ClientName::MIRC32 {
                return ClientName::MIRC32.to_string();
            } else {
                return ClientName::ADIIRC.to_string();
            }
        } else {
            return ClientName::UNKNOWN.to_string();
        }
    }
}
