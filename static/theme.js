// theme.js — Loom theme toggle.
// Runs CSP-compliantly under script-src 'self' (no inline handlers).
// Persists choice to localStorage; honors prefers-color-scheme on first
// visit. Apply ASAP to avoid a flash-of-wrong-theme on cold load.

(function () {
  "use strict";

  // T36: matches aesthetic.js debug-flag pattern. Enable with
  //   localStorage.setItem("loom-debug", "1")
  var DEBUG = (function () {
    try { return localStorage.getItem("loom-debug") === "1"; }
    catch (e) { return false; }
  })();
  function dbg() {
    if (!DEBUG) return;
    var args = Array.prototype.slice.call(arguments);
    args.unshift("[loom:theme]");
    try { console.log.apply(console, args); } catch (e) {}
  }

  var STORAGE_KEY = "loom-theme";
  var root = document.documentElement;
  dbg("init", { stored: (function(){ try {return localStorage.getItem(STORAGE_KEY);} catch(e){return null;}})(), inline: root.getAttribute("data-theme") });

  function preferred() {
    if (window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: light)").matches) {
      return "light";
    }
    return "dark";
  }

  function load() {
    try { return localStorage.getItem(STORAGE_KEY); } catch (e) { return null; }
  }
  function save(v) {
    try { localStorage.setItem(STORAGE_KEY, v); } catch (e) {}
  }

  function apply(theme) {
    root.setAttribute("data-theme", theme);
    var btns = document.querySelectorAll("[data-loom-theme-toggle]");
    for (var i = 0; i < btns.length; i++) {
      btns[i].setAttribute("aria-pressed", theme === "light" ? "true" : "false");
      btns[i].textContent = theme === "light" ? "Dark mode" : "Light mode";
    }
    dbg("apply", theme);
  }

  // Known palette set. theme.js owns light/dark; aesthetic.js owns
  // hc-dark/hc-light/sepia (and any future extension via plugin
  // CSS). If aesthetic.js set one of those before us, DON'T touch
  // it — that was a deliberate user override (URL param or stored
  // pref) and theme.js stomping it caused per-theme contrast bugs
  // on the leaderboard sidebar (T81 root cause).
  // Any theme aesthetic.js may set that theme.js does NOT own.
  // Adding a new community theme: append the data-theme value here
  // AND its :root[data-theme="..."] rule in skin.css.
  var EXTENDED_THEMES = [
    "hc-dark", "hc-light", "sepia",
    "nord", "dracula", "solarized-dark", "gruvbox",
  ];

  // Precedence:
  //   1. extended theme already on <html> (set by aesthetic.js) — DON'T touch
  //   2. user's saved choice (localStorage loom-theme) — always wins for light/dark
  //   3. existing data-theme attribute on <html> (page-level pref)
  //   4. OS prefers-color-scheme
  function initialTheme() {
    var attr = root.getAttribute("data-theme");
    if (attr && EXTENDED_THEMES.indexOf(attr) >= 0) return null; // signal: don't touch
    var saved = load();
    if (saved === "light" || saved === "dark") return saved;
    if (attr === "light" || attr === "dark") return attr;
    return preferred();
  }
  var initial = initialTheme();
  if (initial) apply(initial);

  // Wire up toggles after DOM ready.
  function ready(fn) {
    if (document.readyState !== "loading") fn();
    else document.addEventListener("DOMContentLoaded", fn);
  }
  ready(function () {
    var btns = document.querySelectorAll("[data-loom-theme-toggle]");
    for (var i = 0; i < btns.length; i++) {
      btns[i].addEventListener("click", function () {
        var next = (root.getAttribute("data-theme") === "dark") ? "light" : "dark";
        apply(next);
        save(next);
      });
    }
  });
})();
