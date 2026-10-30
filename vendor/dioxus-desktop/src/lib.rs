#![doc = include_str!("readme.md")]
#![doc(html_logo_url = "https://avatars.githubusercontent.com/u/79236386")]
#![doc(html_favicon_url = "https://avatars.githubusercontent.com/u/79236386")]
#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(unexpected_cfgs)]

mod android_sync_lock;
mod app;
mod assets;
mod config;
mod default_icon;
mod desktop_context;
mod document;
mod edits;
mod element;
mod event_handlers;
mod events;
mod file_upload;
mod hooks;
mod ipc;
mod menubar;
mod mobile;
mod protocol;
mod query;
mod shortcut;
mod waker;
mod webview;

// mobile shortcut is only supported on mobile platforms
#[cfg(any(target_os = "ios", target_os = "android"))]
mod mobile_shortcut;

/// The main entrypoint for this crate
pub mod launch;

// Reexport tao and wry, might want to re-export other important things
pub use tao;
pub use tao::dpi::{LogicalPosition, LogicalSize};
pub use tao::event::WindowEvent;
pub use tao::window::WindowBuilder;
pub use wry;
// Reexport muda only if we are on desktop platforms that support menus
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub use muda;

// Tray icon
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub mod trayicon;

// Public exports
pub use assets::AssetRequest;
pub use config::{Config, WindowCloseBehaviour};
pub use default_icon::{default_icon, icon_from_memory, icon_from_path};
pub use desktop_context::{
    window, DesktopContext, DesktopService, PendingDesktopContext, WeakDesktopContext,
};
pub use event_handlers::WryEventHandler;
pub use hooks::*;
pub use shortcut::{HotKeyState, ShortcutHandle, ShortcutRegistryError};
pub use wry::RequestAsyncResponder;

/// Forge patch: whether an IPC message sent from a frame at `url` may reach the app. The app's own page is
/// served from the `dioxus` protocol (`dioxus://` on macOS and Linux, `http(s)://dioxus.` on Windows); a
/// frame inside it (a srcdoc, data or blob page, or any other address) is not trusted.
pub fn ipc_origin_allowed(url: &str) -> bool {
    url.starts_with("dioxus://") || url.starts_with("http://dioxus.") || url.starts_with("https://dioxus.")
}

#[cfg(test)]
mod forge_ipc_tests {
    use super::ipc_origin_allowed;

    #[test]
    fn only_the_apps_own_page_may_send() {
        assert!(ipc_origin_allowed("dioxus://index.html/"));
        assert!(ipc_origin_allowed("http://dioxus.index.html/"));
        assert!(ipc_origin_allowed("https://dioxus.index.html/"));
        for url in ["about:srcdoc", "about:blank", "data:text/html,x", "blob:dioxus://x", "https://evil.example/", "null", "", "dioxus.evil://x", "http://dioxusevil.com/"] {
            assert!(!ipc_origin_allowed(url), "{url}");
        }
    }
}
