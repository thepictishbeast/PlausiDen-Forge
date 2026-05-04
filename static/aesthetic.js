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

  // First-paint application — respects existing inline attribute
  // OR localStorage choice. theme.js handles theme separately
  // for finer-grained OS-preference fallback.
  for (var i = 0; i < DIMENSIONS.length; i++) {
    var dim = DIMENSIONS[i];
    var saved = load(dim);
    if (saved) apply(dim, saved);
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
