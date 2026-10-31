mod components;
mod editor;
mod ime;
mod office;
pub use forge_domain::{msg, tr};
pub mod i18n;
mod ko;
#[cfg(test)]
mod glyph_scan;
mod theme;
mod xterm;

pub use components::{Button, ButtonKind, Icon, IconKind, IconSize, LoadingRows, Segmented, SettingRow, Select, StatusDot, Switch, TextArea, TextInput, Tone};
pub use components::{COPY_ICON_SVG, PENDING_ICON_SVG, RUN_ICON_SVG, TERMINAL_ICON_SVG};
pub use editor::editor_head;
pub use ime::ime_guard_head;
pub use office::{office_head, pptx_library};
pub use theme::THEME_CSS;
pub use xterm::xterm_head;
