//! Starts the operating system's dictation for the focused text field.
//!
//! macOS: `startDictation:` sent down the responder chain, which the web view's focused field takes
//! (the same as Edit ▸ Start Dictation). Windows: the voice typing keys, Windows+H. Elsewhere there is
//! none, and the button is not shown.

use forge_ports::DictationPort;

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemDictation;

impl DictationPort for SystemDictation {
    fn available(&self) -> bool {
        cfg!(any(target_os = "macos", windows))
    }

    fn start(&self) -> Result<(), String> {
        start()
    }
}

#[cfg(target_os = "macos")]
fn start() -> Result<(), String> {
    use objc2::sel;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let main = MainThreadMarker::new().ok_or_else(|| "Dictation can only start from the app's window.".to_string())?;
    let app = NSApplication::sharedApplication(main);
    // SAFETY: `startDictation:` takes the sender as its one argument, as every action does.
    let taken = unsafe { app.sendAction_to_from(sel!(startDictation:), None, None) };
    if taken {
        Ok(())
    } else {
        Err("NSApplication did not take startDictation:".to_string())
    }
}

#[cfg(windows)]
fn start() -> Result<(), String> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_H, VK_LWIN};

    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: 0 } },
    };
    let inputs = [key(VK_LWIN, false), key(VK_H, false), key(VK_H, true), key(VK_LWIN, true)];
    // SAFETY: the array lives for the call and the size is that of one INPUT, as SendInput asks.
    let sent = unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        Err(format!("SendInput sent {sent} of {} keys", inputs.len()))
    }
}

#[cfg(not(any(target_os = "macos", windows)))]
fn start() -> Result<(), String> {
    Err("This system has no dictation Forge can start.".to_string())
}
