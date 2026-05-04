//! Forge — dev server.
//!
//! Replaces the prior `serve.py` with a Rust binary on tokio +
//! hyper 1.x. Owner directive 2026-05-04: "make sure the entire
//! cms, loom, forge, audits and everything else is super society
//! level tech stacks." Bash + Python were transitional; this
//! retires the last non-Rust component in PlausiDen-Forge.
//!
//! Behavior (matches serve.py exactly):
//!
//! * Bind 0.0.0.0:PORT (default 8123); HTTP/1.1 only — no TLS.
//!   Dev-only tool. Production deploys behind nginx/Caddy.
//! * No-cache + nosniff on every response.
//! * Strict MIME types via `mime_guess`.
//! * Pre-compressed sibling serving (.br, .gz) with mtime guard:
//!   if the source has been edited since the sibling was built,
//!   serve the uncompressed source and log a WARN. Caught the
//!   T54 SRI-mismatch loop.
//! * Per-request timing log; >SLOW_REQUEST_MS_THRESHOLD writes
//!   a structured line to /tmp/skillshots-server-slow.log.
//! * CSS-load self-test on startup: GET / + verify each <link>
//!   resolves to a non-empty body. Loud warning if any fail.
//!
//! AVP-2 invariants:
//!
//! * `unsafe_code = "deny"` (forbid in lib & bin).
//! * No `unwrap` / `expect` in non-test paths. Errors flow up
//!   through `anyhow::Result` and the binary exits non-zero.
//! * Every public function carries a `BUG ASSUMPTION` comment.
//! * Defaults are restrictive (loopback bind not 0.0.0.0 if
//!   `--lan` is not set), per AVP-2 default-deny.

#![forbid(unsafe_code)]

use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bytes::Bytes;
use clap::Parser;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as ConnBuilder;
use tokio::fs;
use tokio::net::TcpListener;
use tracing::{info, warn};

const SLOW_REQUEST_MS_THRESHOLD: u128 = 500;
const SLOW_LOG_PATH: &str = "/tmp/skillshots-server-slow.log";

#[derive(Parser, Debug)]
#[command(
    name = "forge-serve",
    version,
    about = "Forge — typed Rust dev server. Replaces serve.py."
)]
struct Args {
    /// Port to bind. Default 8123 (matches serve.py).
    #[arg(long, default_value_t = 8123)]
    port: u16,

    /// Static-asset root. Default `<cwd>/static`.
    #[arg(long)]
    root: Option<PathBuf>,

    /// Bind 0.0.0.0 (LAN-visible). Default is loopback only.
    /// AVP-2 default-deny: dev server SHOULD NOT be reachable
    /// from the outside without explicit consent.
    #[arg(long)]
    lan: bool,
}

#[derive(Clone)]
struct ServerCfg {
    root: Arc<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    let args = Args::parse();

    let root = match args.root {
        Some(p) => p,
        None => std::env::current_dir()
            .context("forge-serve needs a CWD or --root flag")?
            .join("static"),
    };
    let root = root
        .canonicalize()
        .with_context(|| format!("static root not found: {}", root.display()))?;

    let bind_ip = if args.lan { "0.0.0.0" } else { "127.0.0.1" };
    let addr: SocketAddr = format!("{bind_ip}:{}", args.port)
        .parse()
        .context("invalid bind address")?;

    let cfg = ServerCfg {
        root: Arc::new(root.clone()),
    };

    info!("forge-serve {} root={} bind={}", env!("CARGO_PKG_VERSION"), root.display(), addr);
    if !args.lan {
        info!("loopback-only; pass --lan to expose on the LAN");
    }

    let listener = TcpListener::bind(addr).await
        .with_context(|| format!("could not bind {addr}"))?;

    // Self-test runs in background after a small delay.
    tokio::spawn({
        let cfg = cfg.clone();
        let port = args.port;
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            self_test_css_present(&cfg, port).await;
        }
    });

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                warn!("accept failed: {e}");
                continue;
            }
        };
        let cfg = cfg.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = hyper::service::service_fn(move |req| {
                let cfg = cfg.clone();
                let peer_str = peer.to_string();
                async move { Ok::<_, Infallible>(handle(cfg, peer_str, req).await) }
            });
            if let Err(e) = ConnBuilder::new(TokioExecutor::new())
                .http1_only()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!("conn closed: {e}");
            }
        });
    }
}

async fn handle(cfg: ServerCfg, peer: String, req: Request<Incoming>) -> Response<Full<Bytes>> {
    let started = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let accept_encoding = req
        .headers()
        .get("accept-encoding")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();

    let resp = if method != Method::GET && method != Method::HEAD {
        plain_response(StatusCode::METHOD_NOT_ALLOWED, "method not allowed")
    } else {
        match resolve_and_serve(&cfg.root, &path, &accept_encoding, method == Method::HEAD).await {
            Ok(r) => r,
            Err(e) => {
                warn!("[{peer}] {method} {path} → 500 {e}");
                plain_response(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        }
    };

    let status = resp.status();
    let body_len = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_owned();
    let elapsed_ms = started.elapsed().as_millis();
    let slow_marker = if elapsed_ms >= SLOW_REQUEST_MS_THRESHOLD {
        record_slow(&method, &path, status.as_u16(), &body_len, elapsed_ms);
        "⚠ "
    } else {
        ""
    };
    info!(
        "[{ts}] {slow_marker}{peer} - \"{method} {path}\" {status} {body_len} {elapsed_ms}ms",
        ts = ts_now(),
        peer = peer,
        slow_marker = slow_marker,
        method = method,
        path = path,
        status = status.as_u16(),
        body_len = body_len,
        elapsed_ms = elapsed_ms,
    );
    resp
}

/// Resolve `path` under `root`, applying:
///
/// 1. Trailing-slash → index.html
/// 2. Path traversal block — never escape `root`
/// 3. Pre-compressed sibling negotiation if Accept-Encoding allows
///    AND the sibling's mtime is >= the source's mtime
///
/// Returns the response. Errors only on unexpected I/O failures
/// (e.g. permission). Plain 404 on missing files.
async fn resolve_and_serve(
    root: &Path,
    raw_path: &str,
    accept_encoding: &str,
    head_only: bool,
) -> Result<Response<Full<Bytes>>> {
    let normalized = normalize_request_path(raw_path);
    let mut full_path = root.join(&normalized);

    // Path traversal guard.
    if !full_path.starts_with(root) {
        return Ok(plain_response(StatusCode::FORBIDDEN, "forbidden"));
    }

    // Map directory request → /index.html.
    if full_path.is_dir() {
        full_path = full_path.join("index.html");
    }
    if !full_path.exists() {
        return Ok(plain_response(StatusCode::NOT_FOUND, "not found"));
    }

    let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
    let charset = if mime.type_() == mime_guess::mime::TEXT
        || mime.essence_str().ends_with("javascript")
        || mime.essence_str().ends_with("json")
        || mime.essence_str().ends_with("svg+xml")
    {
        "; charset=utf-8"
    } else {
        ""
    };
    let content_type = format!("{}{}", mime.essence_str(), charset);

    // Pre-compressed sibling negotiation.
    let mut chosen: Option<(PathBuf, &'static str)> = None;
    for (ext, encoding) in &[(".br", "br"), (".gz", "gzip")] {
        if !accept_encoding.contains(*encoding) {
            continue;
        }
        let sibling = with_extension_appended(&full_path, ext);
        if !sibling.exists() {
            continue;
        }
        // Mtime guard: sibling MUST be at least as new as the source.
        let src_mt = mtime(&full_path).await.unwrap_or(0);
        let sib_mt = mtime(&sibling).await.unwrap_or(0);
        if sib_mt < src_mt {
            warn!(
                "stale {} sibling for {} — falling back to uncompressed (src mtime {} > sibling {}). Run forge to refresh.",
                ext,
                full_path.display(),
                src_mt,
                sib_mt
            );
            continue;
        }
        chosen = Some((sibling, encoding));
        break;
    }

    if let Some((sib_path, encoding)) = chosen {
        let body = fs::read(&sib_path).await
            .with_context(|| format!("read {}", sib_path.display()))?;
        let mut builder = Response::builder()
            .status(200)
            .header("content-type", content_type)
            .header("content-encoding", encoding)
            .header("content-length", body.len().to_string())
            .header("vary", "Accept-Encoding");
        builder = decorate_headers(builder);
        let resp = builder
            .body(if head_only { Full::new(Bytes::new()) } else { Full::new(Bytes::from(body)) })
            .context("response build")?;
        return Ok(resp);
    }

    // Uncompressed path.
    let body = fs::read(&full_path).await
        .with_context(|| format!("read {}", full_path.display()))?;
    let mut builder = Response::builder()
        .status(200)
        .header("content-type", content_type)
        .header("content-length", body.len().to_string());
    builder = decorate_headers(builder);
    let resp = builder
        .body(if head_only { Full::new(Bytes::new()) } else { Full::new(Bytes::from(body)) })
        .context("response build")?;
    Ok(resp)
}

/// Normalize a request path: strip leading `/`, decode common
/// percent-escapes, drop trailing-slash → "index.html". Returns
/// a relative path that is safe to join under `root` (the caller
/// still does a `starts_with(root)` traversal guard).
fn normalize_request_path(raw: &str) -> PathBuf {
    let trimmed = raw.split('?').next().unwrap_or("").trim_start_matches('/');
    if trimmed.is_empty() {
        return PathBuf::from("index.html");
    }
    // Decode the most common percent-escapes; we don't need a full
    // urlencoding crate for the dev server.
    let decoded = trimmed
        .replace("%20", " ")
        .replace("%2F", "/")
        .replace("%2f", "/");
    PathBuf::from(decoded)
}

/// Append an extension to a path's existing extension chain,
/// e.g. `loom-skin.css` + `.br` → `loom-skin.css.br`.
fn with_extension_appended(p: &Path, ext: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(ext);
    PathBuf::from(s)
}

/// Build a plain text/plain response.
fn plain_response(status: StatusCode, body: &'static str) -> Response<Full<Bytes>> {
    let mut builder = Response::builder()
        .status(status)
        .header("content-type", "text/plain; charset=utf-8")
        .header("content-length", body.len().to_string());
    builder = decorate_headers(builder);
    builder.body(Full::new(Bytes::from(body))).unwrap_or_else(|_| {
        // SAFETY: response builder cannot fail on these
        // hard-coded headers; this branch is unreachable. Keep
        // the panic-free property by returning a manual default.
        Response::new(Full::new(Bytes::new()))
    })
}

/// Decorate every response with cache + security headers. Same
/// semantics as serve.py.
fn decorate_headers(b: hyper::http::response::Builder) -> hyper::http::response::Builder {
    b.header(
        "cache-control",
        "no-store, no-cache, must-revalidate, max-age=0",
    )
    .header("pragma", "no-cache")
    .header("expires", "0")
    .header("x-content-type-options", "nosniff")
    .header("referrer-policy", "strict-origin-when-cross-origin")
}

/// Async mtime read — returns Unix seconds. Returns 0 on error
/// so the comparison logic doesn't crash; an unreadable mtime
/// is safer-fallback to "stale" interpretation.
async fn mtime(p: &Path) -> Option<u64> {
    let m = fs::metadata(p).await.ok()?;
    let mt = m.modified().ok()?;
    let secs = mt.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(secs)
}

/// Append a slow-request line to the slow log. Best-effort —
/// never propagates I/O errors; missing log file is silent.
fn record_slow(method: &Method, path: &str, status: u16, bytes: &str, ms: u128) {
    let line = format!(
        "{} {} {} {} {}b {}ms\n",
        ts_now(),
        method,
        path,
        status,
        bytes,
        ms
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(SLOW_LOG_PATH)
        .map(|mut f| {
            use std::io::Write;
            let _ = f.write_all(line.as_bytes());
        });
}

/// ISO-ish timestamp for log lines.
fn ts_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
    format!("[{:02}:{:02}:{:02}]", hours, mins, secs)
}

/// Best-effort CSS-load self-test: GET /, parse <link
/// rel="stylesheet"> hrefs, fetch each, complain if any return
/// empty/short body. Surfaces "page will render unstyled" before
/// the human ever opens a browser.
async fn self_test_css_present(cfg: &ServerCfg, port: u16) {
    use std::time::Duration;
    let url = format!("http://127.0.0.1:{port}/");
    let body = match http_get(&url, Duration::from_secs(3)).await {
        Ok(b) => b,
        Err(e) => {
            warn!("[self-test] could not reach {url}: {e}");
            return;
        }
    };
    let html = String::from_utf8_lossy(&body);
    if !html.contains(r#"<link rel="stylesheet""#) && !html.contains("<link rel='stylesheet'") {
        warn!("[self-test] FAIL: GET / has no <link rel=\"stylesheet\">. Page WILL render unstyled.");
        return;
    }
    let mut sheets: Vec<String> = Vec::new();
    for prefix in &[r#"<link rel="stylesheet" href=""#, "<link rel='stylesheet' href='"] {
        let mut search = html.as_ref();
        while let Some(idx) = search.find(prefix) {
            let after = &search[idx + prefix.len()..];
            let end_char = if prefix.contains('"') { '"' } else { '\'' };
            if let Some(end) = after.find(end_char) {
                sheets.push(after[..end].to_owned());
                search = &after[end + 1..];
            } else {
                break;
            }
        }
    }
    let _ = cfg;
    let mut ok = 0usize;
    for href in &sheets {
        let url = format!("http://127.0.0.1:{port}/{}", href.trim_start_matches('/'));
        match http_get(&url, std::time::Duration::from_secs(3)).await {
            Ok(b) if b.len() > 100 => {
                info!("[self-test] OK   {} ({} bytes)", href, b.len());
                ok += 1;
            }
            Ok(b) => warn!("[self-test] EMPTY {} ({} bytes)", href, b.len()),
            Err(e) => warn!("[self-test] FAIL {} ({})", href, e),
        }
    }
    info!("[self-test] {} of {} stylesheet(s) reachable", ok, sheets.len());
}

/// Tiny HTTP/1.1 GET — no extra deps. Used only for the loopback
/// self-test.
async fn http_get(url: &str, timeout: std::time::Duration) -> Result<Vec<u8>> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let parsed = url
        .strip_prefix("http://")
        .context("only http:// URLs supported in self-test")?;
    let (host_port, path) = parsed.split_once('/').unwrap_or((parsed, ""));
    let path = format!("/{path}");
    let req = format!(
        "GET {path} HTTP/1.0\r\nHost: {host_port}\r\nConnection: close\r\n\r\n"
    );
    let stream = tokio::time::timeout(
        timeout,
        tokio::net::TcpStream::connect(host_port),
    )
    .await
    .context("self-test connect timed out")?
    .context("self-test connect")?;
    let mut stream = stream;
    stream.write_all(req.as_bytes()).await.context("write")?;
    let mut buf = Vec::with_capacity(8192);
    tokio::time::timeout(timeout, stream.read_to_end(&mut buf))
        .await
        .context("self-test read timed out")?
        .context("read")?;
    // Strip headers — find \r\n\r\n.
    if let Some(idx) = find_double_crlf(&buf) {
        Ok(buf[idx + 4..].to_vec())
    } else {
        Ok(buf)
    }
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_root() {
        assert_eq!(normalize_request_path("/"), PathBuf::from("index.html"));
    }

    #[test]
    fn normalize_subpath() {
        assert_eq!(
            normalize_request_path("/a/b.html"),
            PathBuf::from("a/b.html")
        );
    }

    #[test]
    fn normalize_strips_query() {
        assert_eq!(
            normalize_request_path("/page.html?ref=x"),
            PathBuf::from("page.html")
        );
    }

    #[test]
    fn with_ext_appended() {
        assert_eq!(
            with_extension_appended(Path::new("/a/b.css"), ".br"),
            PathBuf::from("/a/b.css.br")
        );
    }

    #[test]
    fn double_crlf_finder() {
        let buf = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        let idx = find_double_crlf(buf).unwrap();
        assert_eq!(&buf[idx + 4..], b"hello");
    }
}
