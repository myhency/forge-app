use dioxus::prelude::*;
use forge_features::{use_workspace, Active};

// A click on a file name in an answer: the page sends "<pane index>\n<path>" to Rust. One handler
// serves every pane; it is replaced each time (`dioxus.send` belongs to the eval that created it).
const WATCH_FILE_LINKS: &str = "if (window.__forgeFileLinkHandler) { document.removeEventListener('click', window.__forgeFileLinkHandler); } window.__forgeFileLinkHandler = (event) => { const link = event.target && event.target.closest ? event.target.closest('.file-link') : null; if (link && link.dataset.path) { const pane = link.closest('[data-pane]'); dioxus.send((pane ? pane.dataset.pane : '0') + '\\n' + link.dataset.path); } }; document.addEventListener('click', window.__forgeFileLinkHandler);";

/// Draws nothing. Opens the file a person clicked in the chat of the pane they clicked in.
#[component]
pub fn FileLinkWatcher() -> Element {
    let workspace = use_workspace();
    use_future(move || async move {
        let mut links = document::eval(WATCH_FILE_LINKS);
        while let Ok(text) = links.recv::<String>().await {
            let Some((pane, path)) = text.split_once('\n') else { continue };
            let Ok(index) = pane.parse::<usize>() else { continue };
            if let Some(Active::Live(id)) = workspace.pane_target(index) {
                workspace.open_file(id, path.to_string());
            }
        }
    });
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handler_names_the_pane_of_the_link_and_replaces_its_old_self() {
        assert!(WATCH_FILE_LINKS.contains("removeEventListener('click'"));
        assert!(WATCH_FILE_LINKS.contains("closest('[data-pane]')"));
        assert!(WATCH_FILE_LINKS.contains("dataset.pane"));
        // `dioxus.send` belongs to the eval that installed the listener, so the old one goes first.
        assert!(WATCH_FILE_LINKS.contains(".file-link"));
        assert!(WATCH_FILE_LINKS.contains("dioxus.send"));
        assert!(WATCH_FILE_LINKS.contains("dataset.path"));
    }
}
