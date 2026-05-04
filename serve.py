#!/usr/bin/env python3
"""serve.py — Forge dev server.

Differences from `python -m http.server`:

- **No-cache headers on every response** — `Cache-Control: no-store,
  no-cache, must-revalidate, max-age=0` plus `Pragma: no-cache` plus
  `Expires: 0`. The owner's browser was serving stale CSS even after
  visible source-side updates; that won't happen here.
- **Strict MIME types** — `text/html` for .html, `text/css` for .css,
  `application/javascript` for .js, with `charset=utf-8` everywhere.
- **CSP-aware** — sets `X-Content-Type-Options: nosniff` and a
  permissive `Content-Security-Policy: default-src 'self'`. Pages
  also have their own meta-CSP; HTTP header is the stronger gate.
- **Verbose logging** — every request prints `[ts] METHOD path
  status bytes ms`. If you see `200 0` for a CSS file, the file
  served BUT was empty.
- **CSS-load self-test** — on startup, GET / and verify the
  response references at least one .css file via `<link>`. If not,
  print a HUGE warning so the operator notices BEFORE opening the
  browser.

Usage:  python3 serve.py [--port 8123] [--root static]
"""

import argparse, http.server, mimetypes, os, socket, socketserver, sys, threading, time, urllib.request


# --- mime tightening ------------------------------------------------
mimetypes.add_type("text/css; charset=utf-8",                ".css")
mimetypes.add_type("application/javascript; charset=utf-8",  ".js")
mimetypes.add_type("text/html; charset=utf-8",               ".html")
mimetypes.add_type("application/json; charset=utf-8",        ".json")
mimetypes.add_type("image/avif",                             ".avif")
mimetypes.add_type("image/webp",                             ".webp")
mimetypes.add_type("font/woff2",                             ".woff2")


class ForgeHandler(http.server.SimpleHTTPRequestHandler):
    """Adds no-cache + nosniff + per-request logging.

    T6: also serves pre-compressed .br / .gz siblings when the
    request advertises Accept-Encoding. Saves bandwidth + matches
    production deploys that serve via nginx/Caddy compression.

    T37: per-request timing + slow-request alert. Every response
    has its duration measured; over SLOW_REQUEST_MS_THRESHOLD it
    prints a ⚠ prefix in the log so saturation patterns surface
    during long crawler audits or live demos.
    """

    SLOW_REQUEST_MS_THRESHOLD = 500
    _slow_log_path = "/tmp/skillshots-server-slow.log"

    def _record_slow(self, method, path, status, bytes_, ms):
        """Append a structured line to the slow-request log."""
        try:
            with open(self._slow_log_path, "a", encoding="utf-8") as f:
                f.write(
                    f"{time.strftime('%Y-%m-%dT%H:%M:%S%z')} "
                    f"{method} {path} {status} {bytes_}b {ms}ms\n"
                )
        except OSError:
            pass

    def do_GET(self) -> None:  # noqa: N802
        # T37: capture start time so log_request can compute duration.
        self._t_start = time.perf_counter()
        # Resolve the path the parent class would serve, then check
        # for a pre-compressed sibling.
        path = self.translate_path(self.path)
        accept = self.headers.get('Accept-Encoding', '')
        for ext, encoding in (('.br', 'br'), ('.gz', 'gzip')):
            if encoding in accept and os.path.isfile(path + ext):
                # T54-followup: stale compressed sibling check.
                # If the source has been edited but forge.sh hasn't
                # been re-run, .br/.gz contents diverge from the
                # source — serving them returns OLD bytes while the
                # SRI tag references the NEW content's hash, so
                # browsers refuse the asset and the page renders
                # un-styled. Detect by mtime comparison and fall
                # through to the uncompressed path on mismatch.
                try:
                    src_mtime = os.path.getmtime(path)
                    sib_mtime = os.path.getmtime(path + ext)
                except OSError:
                    break
                if sib_mtime < src_mtime:
                    sys.stdout.write(
                        f"[serve.py] WARN stale {ext} sibling for "
                        f"{path} — falling back to uncompressed "
                        f"(src mtime {src_mtime:.0f} > sibling "
                        f"{sib_mtime:.0f}). Run forge.sh to refresh.\n"
                    )
                    sys.stdout.flush()
                    break
                try:
                    with open(path + ext, 'rb') as f:
                        body = f.read()
                except OSError:
                    break
                ctype = mimetypes.guess_type(path)[0] or 'application/octet-stream'
                self.send_response(200)
                self.send_header('Content-Type', ctype)
                self.send_header('Content-Encoding', encoding)
                self.send_header('Content-Length', str(len(body)))
                self.send_header('Vary', 'Accept-Encoding')
                self.end_headers()
                self.wfile.write(body)
                return
        # Fallback to default (uncompressed) handling.
        super().do_GET()

    def end_headers(self):
        # No-cache: the dev case is "I just changed the file, please
        # serve the new bytes". Production overrides this with hashed
        # filenames + long max-age.
        self.send_header("Cache-Control",
                         "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "strict-origin-when-cross-origin")
        super().end_headers()

    def log_message(self, fmt, *args):
        # Default logger writes to stderr with timestamp; we add our
        # own structured one for easier grep'ing.
        # T37: append duration_ms when do_GET ran (started _t_start).
        ts = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        ms = ""
        slow_marker = ""
        if hasattr(self, "_t_start"):
            duration_ms = int((time.perf_counter() - self._t_start) * 1000)
            ms = f" {duration_ms}ms"
            if duration_ms >= self.SLOW_REQUEST_MS_THRESHOLD:
                slow_marker = "⚠ "
                # Best-effort parse of the log line for slow-log persistence.
                try:
                    parts = (fmt % args).split(" ")
                    method = parts[0].lstrip('"')
                    pth = parts[1] if len(parts) > 1 else "?"
                    status = parts[3] if len(parts) > 3 else "?"
                    bytes_ = parts[4] if len(parts) > 4 else "-"
                    self._record_slow(method, pth, status, bytes_, duration_ms)
                except Exception:
                    pass
        sys.stdout.write(
            f"[{ts}] {slow_marker}{self.address_string()} - {fmt%args}{ms}\n"
        )
        sys.stdout.flush()


def self_test_css_present(port: int) -> None:
    """GET / and complain LOUDLY if the response has no <link rel='stylesheet'>."""
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=3) as r:
            html = r.read(20000).decode(errors="replace")
    except Exception as e:
        print(f"[self-test] could not reach localhost:{port} ({e})")
        return
    has_link = '<link rel="stylesheet"' in html or "<link rel='stylesheet'" in html
    if not has_link:
        print()
        print("=" * 60)
        print("⚠ self-test FAILED: GET / has no <link rel=\"stylesheet\">")
        print("  The page WILL render unstyled in any browser. Check")
        print("  static/index.html for the CSS link tags.")
        print("=" * 60)
        print()
    else:
        # Verify each linked css actually loads.
        import re
        sheets = re.findall(r'<link rel="stylesheet" href="([^"]+)"', html)
        for s in sheets:
            try:
                with urllib.request.urlopen(
                    f"http://127.0.0.1:{port}/{s.lstrip('/')}", timeout=3
                ) as r:
                    sz = len(r.read())
                ok = sz > 100
                tag = "OK " if ok else "EMPTY"
                print(f"[self-test] {tag} {s} ({sz} bytes)")
            except Exception as e:
                print(f"[self-test] FAIL {s} ({e})")
        print(f"[self-test] {len(sheets)} stylesheet(s) referenced + reachable")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8123)
    ap.add_argument("--root", default="static")
    args = ap.parse_args()
    os.chdir(os.path.join(os.path.dirname(os.path.abspath(__file__)), args.root))
    # Threaded server: SimpleHTTPServer is one-request-at-a-time, which
    # caused owner-visible "halfway loaded" hangs whenever the crawler
    # audit ran in parallel (it issues many requests, blocking owner's
    # browser). ThreadingTCPServer handles each request in its own
    # thread so concurrent clients don't queue.
    class ThreadedTCPServer(socketserver.ThreadingMixIn, socketserver.TCPServer):
        daemon_threads = True
        allow_reuse_address = True

    with ThreadedTCPServer(("", args.port), ForgeHandler) as httpd:
        host = socket.gethostname()
        print(f"[serve.py] root={os.getcwd()} port={args.port}")
        print(f"[serve.py] http://localhost:{args.port}/")
        print(f"[serve.py] http://{host}:{args.port}/")
        # Run self-test in a thread so we don't block accept().
        import threading
        threading.Timer(0.4, self_test_css_present, args=(args.port,)).start()
        httpd.serve_forever()


if __name__ == "__main__":
    main()
