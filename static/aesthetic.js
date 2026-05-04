// aesthetic.js — runtime swap of theme / font / density.
// CSP-safe (script-src 'self', no inline). Owner directive
// 2026-05-04: easy swap of themes and aesthetics, fonts, color
// schemes, etc.
//
// Sets / persists three independent attributes on <html>:
//   data-theme   — dark | light | hc-dark | hc-light | sepia
//   data-font    — display | serif | mono | rounded
//   data-density — comfortable | compact | spacious
//
// Each persists to its own localStorage key. Apply ASAP to
// avoid flash-of-wrong-aesthetic. Picker UI elements with the
// matching `data-loom-aesthetic-set="theme:<value>"` attribute
// trigger swaps on click.

(function () {
  "use strict";

  var root = document.documentElement;
  var DIMENSIONS = ["theme", "font", "density"];

  function load(dim) {
    try { return localStorage.getItem("loom-" + dim); } catch (e) { return null; }
  }
  function save(dim, value) {
    try { localStorage.setItem("loom-" + dim, value); } catch (e) {}
  }
  function apply(dim, value) {
    if (value) {
      root.setAttribute("data-" + dim, value);
    } else {
      root.removeAttribute("data-" + dim);
    }
    var btns = document.querySelectorAll(
      '[data-loom-aesthetic-set^="' + dim + ':"]'
    );
    for (var i = 0; i < btns.length; i++) {
      var spec = btns[i].getAttribute("data-loom-aesthetic-set");
      var v = spec.slice(dim.length + 1);
      btns[i].setAttribute("aria-pressed", v === value ? "true" : "false");
    }
  }

  // T42: URL-param overrides for theme/font/density. Lets the
  // crawler matrix sweep all 5 themes × 4 fonts × 3 densities by
  // simply varying the query string — no JS injection needed.
  // ?theme=hc-dark&font=mono&density=spacious applies all three.
  // Persisted via save() so subsequent same-origin nav keeps them.
  function readQueryParam(name) {
    try {
      var qs = (window.location.search || "").slice(1).split("&");
      for (var i = 0; i < qs.length; i++) {
        var kv = qs[i].split("=");
        if (decodeURIComponent(kv[0]) === name) {
          return kv.length > 1 ? decodeURIComponent(kv[1]) : "";
        }
      }
    } catch (e) {}
    return null;
  }

  // First-paint application — respects existing inline attribute
  // OR URL-param OR localStorage choice. theme.js handles theme
  // separately for finer-grained OS-preference fallback.
  for (var i = 0; i < DIMENSIONS.length; i++) {
    var dim = DIMENSIONS[i];
    var fromQuery = readQueryParam(dim);
    if (fromQuery) {
      apply(dim, fromQuery);
      save(dim, fromQuery);
      continue;
    }
    var saved = load(dim);
    if (saved) apply(dim, saved);
  }

  // T26 RTL: ?dir=rtl|ltr URL param sets the document direction.
  // dir is a real HTML attribute (not data-*), so it goes on the
  // <html> element directly without a 'data-' prefix.
  var dirParam = readQueryParam("dir");
  if (dirParam === "rtl" || dirParam === "ltr") {
    root.setAttribute("dir", dirParam);
    try { localStorage.setItem("loom-dir", dirParam); } catch (e) {}
  } else {
    try {
      var savedDir = localStorage.getItem("loom-dir");
      if (savedDir === "rtl" || savedDir === "ltr") {
        root.setAttribute("dir", savedDir);
      }
    } catch (e) {}
  }

  function ready(fn) {
    if (document.readyState !== "loading") fn();
    else document.addEventListener("DOMContentLoaded", fn);
  }
  ready(function () {
    var btns = document.querySelectorAll("[data-loom-aesthetic-set]");
    for (var i = 0; i < btns.length; i++) {
      btns[i].addEventListener("click", function (ev) {
        var spec = ev.currentTarget.getAttribute(
          "data-loom-aesthetic-set"
        );
        var idx = spec.indexOf(":");
        if (idx < 0) return;
        var dim = spec.slice(0, idx);
        var value = spec.slice(idx + 1);
        apply(dim, value);
        save(dim, value);
      });
    }
    var resets = document.querySelectorAll("[data-loom-aesthetic-reset]");
    for (var j = 0; j < resets.length; j++) {
      resets[j].addEventListener("click", function () {
        for (var k = 0; k < DIMENSIONS.length; k++) {
          var d = DIMENSIONS[k];
          try { localStorage.removeItem("loom-" + d); } catch (e) {}
          root.removeAttribute("data-" + d);
        }
        location.reload();
      });
    }
  });
})();
