// Draws a docx as pages and a pptx as slides inside an element of the page. The libraries are
// docx-preview and pptx-preview (assets/OFFICE-LICENSES). Both answer "ok" or "error: <why>".
window.forgeOffice = (function () {
  function bytes(b64) {
    var bin = atob(b64);
    var out = new Uint8Array(bin.length);
    for (var i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
    return out.buffer;
  }
  // A file is not trusted: nothing in the drawn pages may navigate the window or run.
  function tidy(root) {
    root.querySelectorAll("a").forEach(function (a) {
      a.removeAttribute("href");
      a.removeAttribute("target");
    });
    root.querySelectorAll("script, iframe, object, embed, link").forEach(function (n) { n.remove(); });
  }
  // Word writes bullets as private-use characters of the Symbol and Wingdings fonts, which a Mac
  // does not have: they would show as empty boxes. These are the usual ones, as plain characters.
  var SYMBOLS = { f0b7: "\\2022", f0a7: "\\25AA", f0a8: "\\25E6", f0d8: "\\27A2", f0fc: "\\2713", f076: "\\2756", f0e0: "\\2192" };
  function symbolsToText(root) {
    root.querySelectorAll("style").forEach(function (style) {
      var css = style.textContent || "";
      var changed = css.replace(/content:\s*"\\(f0[0-9a-f]{2})"/gi, function (all, code) {
        var plain = SYMBOLS[code.toLowerCase()];
        return plain ? 'content:"' + plain + '"' : all;
      });
      // The same characters when the stylesheet holds them as they are, not as an escape.
      changed = changed.replace(/content:\s*"([\uF000-\uF0FF])(?:\\9)?"/g, function (all, ch) {
        var plain = SYMBOLS[ch.charCodeAt(0).toString(16)];
        return plain ? 'content:"' + plain + '"' : all;
      });
      if (changed !== css) style.textContent = changed.replace(/font-family:\s*(Symbol|Wingdings)[^;}]*/gi, "font-family: inherit");
    });
  }
  // Shrinks the drawn pages to the width of the host (never enlarges them).
  function fit(host, inner, natural) {
    var space = host.clientWidth;
    if (space > 0 && natural > 0) inner.style.zoom = String(Math.min(1, space / natural));
  }
  function watch(host, redo) {
    if (host.__forgeWatch) host.__forgeWatch.disconnect();
    var timer = null;
    host.__forgeWatch = new ResizeObserver(function () {
      clearTimeout(timer);
      timer = setTimeout(redo, 80);
    });
    host.__forgeWatch.observe(host);
  }
  return {
    docx: async function (id, b64) {
      var host = document.getElementById(id);
      if (!host) return "error: no element";
      try {
        host.innerHTML = "";
        await docx.renderAsync(bytes(b64), host, null, {
          className: "docx",
          inWrapper: true,
          breakPages: true,
          ignoreLastRenderedPageBreak: true,
          renderHeaders: true,
          renderFooters: true,
          useBase64URL: true,
        });
        tidy(host);
        symbolsToText(host);
        var wrapper = host.querySelector(".docx-wrapper");
        var page = host.querySelector("section.docx");
        if (wrapper && page) {
          var refit = function () { fit(host, wrapper, page.offsetWidth + 40); };
          refit();
          watch(host, refit);
        }
        return "ok";
      } catch (e) {
        return "error: " + (e && e.message ? e.message : String(e));
      }
    },
    pptx: async function (id, b64) {
      var host = document.getElementById(id);
      if (!host) return "error: no element";
      try {
        host.innerHTML = "";
        var width = Math.max(240, host.clientWidth - 24);
        var inner = document.createElement("div");
        host.appendChild(inner);
        // No height: with one the library keeps the slides in a box of that height and scrolls inside it.
        var previewer = pptxPreview.init(inner, { width: width, mode: "list" });
        await previewer.preview(bytes(b64));
        tidy(host);
        return "ok";
      } catch (e) {
        return "error: " + (e && e.message ? e.message : String(e));
      }
    },
  };
})();
