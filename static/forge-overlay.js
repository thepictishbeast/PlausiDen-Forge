// forge-overlay.js — in-browser error console for PoC builds.
// Reads window.__FORGE_FINDINGS__ (emitted by forge.sh) and pins
// a collapsible panel to the bottom-right of every page so the
// missing backends + phantom buttons are CONSTANTLY visible.
//
// Hidden by default in production mode. In poc mode, default
// state is "collapsed" (a small chip you can expand).
//
// CSP-safe: external script, no inline handlers.

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
    args.unshift("[loom:forge-overlay]");
    try { console.log.apply(console, args); } catch (e) {}
  }

  function ready(fn) {
    if (document.readyState !== "loading") fn();
    else document.addEventListener("DOMContentLoaded", fn);
  }

  ready(function () {
    var data = (window.__FORGE_FINDINGS__) || null;
    dbg("ready", { hasData: !!data, mode: data && data.mode, findings: data && (data.findings || []).length });
    if (!data) return;
    if (data.mode !== "poc") return; // production: don't render overlay

    var n = (data.findings || []).length;
    if (n === 0) return;

    // Owner directive 2026-05-04: forge findings must be background
    // logs, not user-facing UI. The overlay is OPT-IN — it only
    // renders when ?forge=show is in the URL OR localStorage flag
    // is set. Findings still land in window.__FORGE_FINDINGS__ for
    // anyone who DOES want to inspect via DevTools console.
    var optedIn = false;
    try {
      var qs = (window.location.search || "");
      if (qs.indexOf("forge=show") >= 0) optedIn = true;
      if (window.localStorage && localStorage.getItem("forge-overlay") === "1") {
        optedIn = true;
      }
    } catch (_) { /* private mode etc. */ }
    if (!optedIn) {
      // Background log mode: print a single concise line to console
      // so the operator knows findings exist without polluting the page.
      try {
        if (window.console && console.info) {
          console.info(
            "%c⚠ forge: " + n + " finding(s) — append ?forge=show to URL " +
            "OR localStorage.setItem('forge-overlay','1') to render the panel.",
            "color:#f59e0b;font-weight:600;"
          );
        }
      } catch (_) { /* ignore */ }
      return;
    }

    // Inject minimal CSS scoped to the overlay only. Safe because
    // it's same-origin (CSP style-src 'self' allows our own JS to
    // mutate the DOM with classed nodes; we avoid inline style
    // attributes by appending a <style> element which IS allowed).
    var style = document.createElement("style");
    style.textContent =
      ".forge-overlay{position:fixed;bottom:var(--loom-space-4,1rem);" +
        "right:var(--loom-space-4,1rem);z-index:9999;font-size:" +
        "var(--loom-font-xs,12px);font-family:ui-monospace,'JetBrains Mono'," +
        "Menlo,monospace;max-width:min(420px,calc(100vw - 2rem));" +
        "background:var(--loom-color-surface-muted,#11151c);" +
        "border:2px solid var(--loom-color-warn,#f59e0b);" +
        "border-radius:var(--loom-radius-lg,0.5rem);" +
        "color:var(--loom-color-ink,#e2e8f0);" +
        "box-shadow:0 12px 40px rgba(0,0,0,0.5);overflow:hidden}" +
      ".forge-overlay header{display:flex;align-items:center;gap:" +
        "var(--loom-space-2,0.5rem);padding:var(--loom-space-3,0.75rem);" +
        "cursor:pointer;background:var(--loom-color-warn,#f59e0b);" +
        "color:#000;font-weight:700;text-transform:uppercase;" +
        "letter-spacing:0.06em}" +
      ".forge-overlay[data-state=collapsed] header{border-radius:" +
        "calc(var(--loom-radius-lg,0.5rem) - 2px)}" +
      ".forge-overlay[data-state=collapsed] .body{display:none}" +
      ".forge-overlay .body{padding:var(--loom-space-3,0.75rem);" +
        "max-height:50vh;overflow-y:auto}" +
      ".forge-overlay .group{margin-bottom:var(--loom-space-3,0.75rem)}" +
      ".forge-overlay .group:last-child{margin-bottom:0}" +
      ".forge-overlay h4{font-size:var(--loom-font-xs,12px);" +
        "font-weight:700;margin-bottom:var(--loom-space-1,0.25rem);" +
        "color:var(--loom-color-warn,#f59e0b);text-transform:uppercase;" +
        "letter-spacing:0.06em}" +
      ".forge-overlay ul{list-style:none;padding:0;margin:0}" +
      ".forge-overlay li{padding:var(--loom-space-1,0.25rem) 0;" +
        "border-bottom:1px solid var(--loom-color-border,#1e293b);" +
        "line-height:1.4}" +
      ".forge-overlay li:last-child{border-bottom:0}" +
      ".forge-overlay .path{color:var(--loom-color-ink-muted,#94a3b8);" +
        "font-weight:600}" +
      ".forge-overlay .msg{display:block;margin-top:2px}" +
      ".forge-overlay .badge{margin-left:auto;font-size:10px;" +
        "padding:2px 8px;background:rgba(0,0,0,0.25);border-radius:" +
        "var(--loom-radius-full,9999px)}" +
      ".forge-overlay .footer{font-size:10px;color:" +
        "var(--loom-color-ink-muted,#94a3b8);margin-top:" +
        "var(--loom-space-3,0.75rem);padding-top:var(--loom-space-2,0.5rem);" +
        "border-top:1px solid var(--loom-color-border,#1e293b)}" +
      ".forge-overlay button{background:none;border:0;color:#000;" +
        "font:inherit;cursor:pointer;font-weight:700}";
    document.head.appendChild(style);

    // Group findings by phase for the panel layout.
    var byPhase = {};
    var strictCount = 0, warnCount = 0;
    for (var i = 0; i < data.findings.length; i++) {
      var f = data.findings[i];
      (byPhase[f.phase] = byPhase[f.phase] || []).push(f);
      if (f.severity === "STRICT") strictCount++;
      else warnCount++;
    }

    // Build the DOM — using createElement only (CSP-safe).
    var root = document.createElement("aside");
    root.className = "forge-overlay";
    root.setAttribute("role", "log");
    root.setAttribute("aria-live", "polite");
    root.setAttribute("aria-label", "Forge build findings");
    root.setAttribute("data-state", "collapsed");

    var hdr = document.createElement("header");
    var label = document.createElement("span");
    label.textContent = "⚠ forge: " + n + " finding" + (n === 1 ? "" : "s");
    var spacer = document.createElement("span");
    spacer.style.flex = "1";
    var chip = document.createElement("span");
    chip.className = "badge";
    chip.textContent = data.mode;
    var toggle = document.createElement("button");
    toggle.type = "button";
    toggle.textContent = "▾";
    toggle.setAttribute("aria-label", "Toggle Forge overlay");
    // Dev-tool, not user-facing — opt out of the 44px tap-target floor.
    toggle.setAttribute("data-tap", "compact");
    toggle.style.minWidth = "32px";
    toggle.style.minHeight = "32px";
    hdr.appendChild(label);
    hdr.appendChild(spacer);
    hdr.appendChild(chip);
    hdr.appendChild(toggle);

    var body = document.createElement("div");
    body.className = "body";

    Object.keys(byPhase).sort().forEach(function (phase) {
      var group = document.createElement("div");
      group.className = "group";
      var h = document.createElement("h4");
      h.textContent = phase + " (" + byPhase[phase].length + ")";
      var ul = document.createElement("ul");
      byPhase[phase].forEach(function (f) {
        var li = document.createElement("li");
        var p = document.createElement("span");
        p.className = "path";
        p.textContent = (f.severity === "STRICT" ? "✗ " : "· ") + f.path;
        var m = document.createElement("span");
        m.className = "msg";
        m.textContent = f.message;
        li.appendChild(p);
        li.appendChild(m);
        ul.appendChild(li);
      });
      group.appendChild(h);
      group.appendChild(ul);
      body.appendChild(group);
    });

    var footer = document.createElement("div");
    footer.className = "footer";
    footer.textContent =
      "Mode: " + data.mode + " · " + strictCount + " strict, " +
      warnCount + " warn. Set forge.toml mode=production to make " +
      "warns fatal too.";
    body.appendChild(footer);

    function flip() {
      var s = root.getAttribute("data-state");
      root.setAttribute("data-state", s === "collapsed" ? "open" : "collapsed");
      toggle.textContent = s === "collapsed" ? "▴" : "▾";
    }
    hdr.addEventListener("click", flip);

    root.appendChild(hdr);
    root.appendChild(body);
    document.body.appendChild(root);
  });
})();
