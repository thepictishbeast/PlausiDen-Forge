// cmdk.js — Loom CommandPalette wiring.
// Binds Cmd-K / Ctrl-K to open <dialog id="cmdk">, handles
// arrow-up/arrow-down navigation, Enter to fire data-action,
// Esc / outside-click to close. CSP-safe (script-src 'self', no
// inline handlers, no eval).
//
// Each <li role="option"> can carry:
//   data-action  — what to do on Enter / click. Recognized:
//                    "goto:/path"           → window.location
//                    "back"                 → history.back()
//                    "toggle:loom-theme"    → flips data-theme
//                                              between dark/light
//   data-keywords — additional search tokens (e.g. "leaderboard
//                   top earners standings")

(function () {
  "use strict";

  // T36: matches aesthetic.js debug-flag pattern.
  var DEBUG = (function () {
    try { return localStorage.getItem("loom-debug") === "1"; }
    catch (e) { return false; }
  })();
  function dbg() {
    if (!DEBUG) return;
    var args = Array.prototype.slice.call(arguments);
    args.unshift("[loom:cmdk]");
    try { console.log.apply(console, args); } catch (e) {}
  }

  function ready(fn) {
    if (document.readyState !== "loading") fn();
    else document.addEventListener("DOMContentLoaded", fn);
  }

  ready(function () {
    var dialog = document.getElementById("cmdk");
    if (!dialog || typeof dialog.showModal !== "function") return;
    var input = dialog.querySelector('input[type="search"]');
    var list  = dialog.querySelector(".loom-cmdk-list");
    if (!input || !list) return;

    var options = Array.from(list.querySelectorAll('[role="option"]'));
    var activeIdx = 0;

    function setActive(idx) {
      var visible = options.filter(function (o) { return !o.hidden; });
      if (visible.length === 0) return;
      activeIdx = (idx + visible.length) % visible.length;
      options.forEach(function (o) { o.setAttribute("data-active", "false"); });
      visible[activeIdx].setAttribute("data-active", "true");
      visible[activeIdx].scrollIntoView({ block: "nearest" });
    }

    function filter(query) {
      var q = (query || "").trim().toLowerCase();
      options.forEach(function (o) {
        var label = (o.querySelector(".loom-cmdk-label") || {}).textContent || "";
        var keywords = o.getAttribute("data-keywords") || "";
        var hay = (label + " " + keywords).toLowerCase();
        o.hidden = q !== "" && hay.indexOf(q) < 0;
      });
      setActive(0);
    }

    // Same-origin path validation for goto: actions.
    // SECURITY: a malicious or tampered data-action value (e.g.
    // "goto://evil.example.com" or "goto:javascript:alert(1)")
    // would otherwise navigate the visitor off-site or execute
    // script. Accept only paths that look like a same-origin
    // application route: a leading "/" followed by NOT another "/"
    // (which would be protocol-relative), no "://" anywhere, no
    // CR/LF / control chars. Anything else logs + ignores.
    function isSafeAppPath(p) {
      if (typeof p !== "string" || p.length === 0) return false;
      if (p.charAt(0) !== "/") return false;       // must be path-relative
      if (p.charAt(1) === "/") return false;       // no protocol-relative //evil
      if (p.indexOf("://") !== -1) return false;   // belt-and-braces scheme check
      if (/[\x00-\x1f]/.test(p)) return false;     // CR/LF/control → URL smuggling
      return true;
    }

    function fire(option) {
      var action = (option.getAttribute("data-action") || "").trim();
      dbg("fire", action);
      dialog.close();
      if (action.indexOf("goto:") === 0) {
        var dest = action.slice(5);
        if (isSafeAppPath(dest)) {
          window.location.href = dest;
        } else {
          dbg("blocked unsafe goto", dest);
        }
      } else if (action === "back") {
        history.back();
      } else if (action.indexOf("toggle:") === 0) {
        var attr = action.slice(7);
        var cur = document.documentElement.getAttribute(attr.replace("loom-", "data-"));
        var next = (cur === "dark") ? "light" : "dark";
        document.documentElement.setAttribute(attr.replace("loom-", "data-"), next);
        try { localStorage.setItem(attr, next); } catch (e) {}
      }
    }

    // Keybind: cmd-k / ctrl-k toggles open.
    document.addEventListener("keydown", function (ev) {
      if ((ev.metaKey || ev.ctrlKey) && ev.key === "k") {
        ev.preventDefault();
        if (dialog.open) {
          dialog.close();
        } else {
          dialog.showModal();
          input.value = "";
          filter("");
          input.focus();
        }
      }
    });

    input.addEventListener("input", function (ev) {
      filter(ev.target.value);
    });

    dialog.addEventListener("keydown", function (ev) {
      if (ev.key === "ArrowDown") {
        ev.preventDefault();
        setActive(activeIdx + 1);
      } else if (ev.key === "ArrowUp") {
        ev.preventDefault();
        setActive(activeIdx - 1);
      } else if (ev.key === "Enter") {
        ev.preventDefault();
        var visible = options.filter(function (o) { return !o.hidden; });
        if (visible[activeIdx]) fire(visible[activeIdx]);
      }
    });

    list.addEventListener("click", function (ev) {
      var opt = ev.target.closest('[role="option"]');
      if (opt) fire(opt);
    });

    // Outside-click closes.
    dialog.addEventListener("click", function (ev) {
      if (ev.target === dialog) dialog.close();
    });

    // Init filter so first item is "active".
    setActive(0);
  });
})();
