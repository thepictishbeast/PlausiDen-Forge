// theme.js — Loom theme toggle.
// Runs CSP-compliantly under script-src 'self' (no inline handlers).
// Persists choice to localStorage; honors prefers-color-scheme on first
// visit. Apply ASAP to avoid a flash-of-wrong-theme on cold load.

(function () {
  "use strict";

  var STORAGE_KEY = "loom-theme";
  var root = document.documentElement;

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
  }

  // Precedence:
  //   1. user's saved choice (localStorage) — always wins
  //   2. existing data-theme attribute on <html> (page-level pref)
  //   3. OS prefers-color-scheme
  // This avoids stomping on a hard-coded data-theme set inline.
  function initialTheme() {
    var saved = load();
    if (saved === "light" || saved === "dark") return saved;
    var attr = root.getAttribute("data-theme");
    if (attr === "light" || attr === "dark") return attr;
    return preferred();
  }
  apply(initialTheme());

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
