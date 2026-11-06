use dioxus::prelude::*;
use forge_features::{DiskSpace, LOW_DISK_BYTES};
use forge_shared::{tr, Icon, IconKind, IconSize};

/// The desktop app's low-disk banner (observed 2026-11-06): a warning sign, `디스크 공간 부족:
/// 남은 공간 8.6GB`, and a close button, at the bottom right. Closed, it stays away until the app
/// starts again. Nothing without the measurement (no `DiskSpace` in context, or none taken yet).
#[component]
pub fn LowDiskBanner() -> Element {
    let mut closed = use_signal(|| false);
    let Some(DiskSpace(space)) = try_consume_context::<DiskSpace>() else { return rsx! {} };
    let Some(free) = space().filter(|free| *free < LOW_DISK_BYTES) else { return rsx! {} };
    if closed() {
        return rsx! {};
    }
    let amount = format!("{:.1}GB", free as f64 / 1_000_000_000.0);
    rsx! {
        div { class: "low-disk", role: "status",
            span { class: "low-disk-sign", Icon { kind: IconKind::Alert, size: IconSize::Small } }
            span { class: "low-disk-text", {tr!("Low disk space: {amount} left", amount = amount)} }
            button {
                class: "icon-button",
                r#type: "button",
                title: tr!("Close"),
                "aria-label": tr!("Close"),
                onclick: move |_| closed.set(true),
                Icon { kind: IconKind::Close, size: IconSize::Small }
            }
        }
    }
}
