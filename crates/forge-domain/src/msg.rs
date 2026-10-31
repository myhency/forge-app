/// Marks a sentence that is translated later, where it is drawn (`i18n::text(label)`). It is the
/// sentence itself; the scan test of `forge-shared` counts it like `tr!`. It lives here so the
/// domain's own labels can be marked too.
#[macro_export]
macro_rules! msg {
    ($text:literal) => {
        $text
    };
}
