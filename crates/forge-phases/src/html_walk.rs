//! Shared helper: iterate `static/*.html` files in deterministic
//! order, reading each as UTF-8 text. Most phases that scan HTML
//! pages reuse this.
//!
//! BUG ASSUMPTION: files are read fully into memory. The PoC's
//! largest HTML file is ~16 KB; for sites in the 50+ MB-per-page
//! range this would be a memory hazard. A streaming reader is
//! queued for the day a real CMS lands.

use std::fs;
use std::path::{Path, PathBuf};

use forge_core::BuildError;

/// One HTML file in the static dir.
pub struct HtmlFile {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Display name (basename, e.g. `index.html`).
    pub name: String,
    /// Decoded contents.
    pub body: String,
}

/// Walk `<static_dir>/*.html` in lexicographic order and read each.
///
/// I/O errors during read map to `BuildError::Io`. The directory
/// being missing is NOT an error — it's an empty walk; phases get
/// an empty Vec and report no findings, which mirrors the bash
/// forge behavior.
pub fn walk_html(static_dir: &Path, phase: &str) -> Result<Vec<HtmlFile>, BuildError> {
    let mut out: Vec<HtmlFile> = Vec::new();
    let entries = match fs::read_dir(static_dir) {
        Ok(it) => it,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => {
            return Err(BuildError::Io {
                context: format!("{phase}: read_dir {}", static_dir.display()),
                source: e,
            });
        }
    };
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| BuildError::Io {
            context: format!("{phase}: dir entry under {}", static_dir.display()),
            source: e,
        })?;
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) == Some("html") {
            paths.push(p);
        }
    }
    paths.sort();
    for path in paths {
        let body = fs::read_to_string(&path).map_err(|e| BuildError::Io {
            context: format!("{phase}: read {}", path.display()),
            source: e,
        })?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_owned();
        out.push(HtmlFile { path, name, body });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn td() -> tempdir::TempDir {
        tempdir::TempDir::new("forge-walk").expect("tempdir create")
    }

    // The crate doesn't depend on tempdir; for these tests we
    // create files in a manually-named directory and clean up
    // after. (Avoiding new dev-deps until we have several test
    // patterns demanding them.)

    #[test]
    fn empty_dir_returns_empty() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-walk-empty-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let result = walk_html(&tmp, "test").expect("walk_html should succeed");
        assert_eq!(result.len(), 0);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn missing_dir_returns_empty_not_error() {
        let nonexistent = std::env::temp_dir().join("forge-walk-DOES-NOT-EXIST-xyz");
        let _ = std::fs::remove_dir_all(&nonexistent);
        let result = walk_html(&nonexistent, "test").expect("walk should succeed on missing dir");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn returns_html_in_lexicographic_order() {
        let tmp = std::env::temp_dir().join(format!(
            "forge-walk-order-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("z.html"), "<p>z</p>").unwrap();
        std::fs::write(tmp.join("a.html"), "<p>a</p>").unwrap();
        std::fs::write(tmp.join("m.html"), "<p>m</p>").unwrap();
        // Non-HTML files should be skipped.
        std::fs::write(tmp.join("README.md"), "ignore me").unwrap();

        let files = walk_html(&tmp, "test").unwrap();
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.html", "m.html", "z.html"]);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

// REGRESSION-GUARD: tempdir is NOT a dependency. The above tests
// build their own temp paths and clean up explicitly; this keeps
// the dependency tree minimal.
mod tempdir {
    /// Unused — kept as a hint that we deliberately did NOT pull
    /// the tempdir crate. Tests above use `std::env::temp_dir()`
    /// + a process-id-derived suffix instead.
    pub struct TempDir;
    impl TempDir {
        #[allow(dead_code)]
        pub fn new(_: &str) -> Result<Self, std::io::Error> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "tempdir module is intentionally a stub — use std::env::temp_dir + cleanup in tests",
            ))
        }
    }
}
