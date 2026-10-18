// Keeps the Enter that confirms an input-method composition (Korean, Japanese, Chinese) away from
// every handler in the page. WKWebView sends compositionend first and Dioxus gets no composition
// events, so that Enter arrives as a plain keydown (isComposing false, keyCode 13) and a rename box,
// search palette, composer or comment box would act on it. What WebKit does give us: just before
// that keydown it fires an `input` that leaves the field's text exactly as it was. We swallow the
// next Enter after such an input (and any Enter that says it is composing). For an Enter that
// says it is composing we leave the default alone, so the input method's own confirmation runs. For
// the one after the unchanged input the text is already committed, and its default action would
// put a newline into a textarea, so that one is also cancelled.
(function () {
  var before = new WeakMap(); // the field's text before the key being handled
  var armed = -Infinity;      // timeStamp of an input that changed nothing
  var armedTarget = null;     // the field that input happened in
  var WINDOW_MS = 150;
  // Only fields that hold typed text. A checkbox, radio, range or button has a .value too, but it
  // never changes with typing, so it must not look like "an input that left the text unchanged".
  var TEXT_TYPES = { text: 1, search: 1, url: 1, email: 1, password: 1, tel: 1, number: 1, "": 1 };

  function text(target) {
    if (!target || typeof target.value !== "string") return null;
    // xterm's hidden textarea: one Enter must run a command, so it is never guarded.
    if (target.classList && target.classList.contains("xterm-helper-textarea")) return null;
    var tag = target.tagName;
    if (tag === "TEXTAREA") return target.value;
    if (tag === "INPUT" && TEXT_TYPES[String(target.getAttribute("type") || "").toLowerCase()] === 1) return target.value;
    return null;
  }

  document.addEventListener("keydown", function (event) {
    var composing = false;
    var confirmed = false;
    if (event.key === "Enter") {
      composing = event.isComposing || event.keyCode === 229;
      confirmed = event.target === armedTarget && event.timeStamp - armed < WINDOW_MS;
    }
    armed = -Infinity;
    armedTarget = null;
    if (composing || confirmed) {
      event.stopImmediatePropagation();
      if (!composing) event.preventDefault();
      return;
    }
    var value = text(event.target);
    if (value !== null) before.set(event.target, value);
  }, true);

  document.addEventListener("input", function (event) {
    var value = text(event.target);
    if (value === null) return;
    var unchanged = before.get(event.target) === value;
    armed = unchanged ? event.timeStamp : -Infinity;
    armedTarget = unchanged ? event.target : null;
    before.set(event.target, value);
  }, true);
})();
