mod components;
mod editor;
pub use forge_domain::{msg, tr};
pub mod i18n;
mod ko;
mod theme;
mod xterm;

pub use components::{Button, ButtonKind, Segmented, SettingRow, Select, StatusDot, Switch, TextArea, TextInput, Tone};
pub use editor::editor_head;
pub use theme::THEME_CSS;
pub use xterm::xterm_head;
