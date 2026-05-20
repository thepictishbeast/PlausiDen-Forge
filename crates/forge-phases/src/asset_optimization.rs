//! `asset_optimization` — flag heavy / unoptimized image / video /
//! audio / font assets.
//!
//! Bash parity: `phase_asset_optimization`. Per-asset suggestions:
//!
//! * `*.png` > 100 KB    → suggest webp/avif
//! * `*.jpg` / `*.jpeg`   → require webp/avif sibling
//! * `*.mp4`              → require webm sibling
//! * `*.wav`              → flag (re-encode to opus/aac)
//! * `*.ttf` / `*.otf`    → flag (convert to woff2; add font-display: swap)

use std::fs;
use std::path::Path;

use forge_core::{BuildCtx, BuildError, Finding, Phase};

const PNG_BUDGET: u64 = 100 * 1024;

/// `asset_optimization` phase.
#[derive(Debug, Default)]
pub struct AssetOptimizationPhase;

impl Phase for AssetOptimizationPhase {
    fn name(&self) -> &'static str {
        "asset_optimization"
    }

    fn run(&self, ctx: &BuildCtx) -> Result<Vec<Finding>, BuildError> {
        let mut findings = Vec::new();
        let entries = match fs::read_dir(&ctx.static_dir) {
            Ok(it) => it,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(findings),
            Err(e) => {
                return Err(BuildError::Io {
                    context: format!("{}: read_dir {}", self.name(), ctx.static_dir.display()),
                    source: e,
                });
            }
        };

        // Collect all paths first so we can do sibling lookups
        // without re-reading the directory.
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| BuildError::Io {
                context: format!(
                    "{}: dir entry under {}",
                    self.name(),
                    ctx.static_dir.display()
                ),
                source: e,
            })?;
            paths.push(entry.path());
        }
        let path_set: std::collections::BTreeSet<&Path> =
            paths.iter().map(|p| p.as_path()).collect();

        for path in &paths {
            let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
                continue;
            };
            let ext_lower = ext.to_ascii_lowercase();
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_owned();
            match ext_lower.as_str() {
                "png" => {
                    let sz = file_size(path, self.name())?;
                    if sz > PNG_BUDGET {
                        findings.push(
                            Finding::warn(
                                self.name(),
                                name,
                                format!("{} PNG over budget", iec(sz)),
                            )
                            .citing(["perf-001", "perf-002"])
                            .why("PNG is lossless; for photographic / large UI imagery, webp/avif are 50-80% smaller with imperceptible quality loss. Larger image = slower LCP")
                            .fix("the loom-bridge assets pipeline should emit a webp + avif ladder alongside the PNG (assets-core handles this) — verify forge assets validate is reporting the bundle as complete")
                            .skill("add-loom-primitive"),
                        );
                    }
                }
                "jpg" | "jpeg" if !has_modern_sibling(path, &path_set) => {
                    findings.push(
                        Finding::warn(
                            self.name(),
                            name,
                            "JPG without webp/avif sibling",
                        )
                        .citing(["perf-001", "perf-002"])
                        .why("`<picture>` lets modern browsers fetch a faster format (avif > webp > jpg); without siblings every visitor downloads the legacy format")
                        .fix("re-run loom-bridge asset emission to produce webp/avif siblings; if missing, forge assets validate will diagnose the bundle gap")
                        .skill("add-loom-primitive"),
                    );
                }
                "mp4" if !has_sibling(path, &path_set, "webm") => {
                    findings.push(
                        Finding::warn(
                            self.name(),
                            name,
                            "MP4 without webm sibling",
                        )
                        .citing(["perf-001"])
                        .why("`<video><source>` falls back to MP4 if webm is missing, costing visitors on free-software / lower-bandwidth contexts")
                        .fix("the asset bundler should emit a .webm sibling alongside the .mp4; verify via forge assets validate"),
                    );
                }
                "wav" => {
                    findings.push(
                        Finding::warn(
                            self.name(),
                            name,
                            "WAV file in static — uncompressed audio is bandwidth-hostile",
                        )
                        .citing(["perf-001"])
                        .why("WAV is uncompressed PCM; opus (BSD-licensed, royalty-free, best ratio) or aac (broader compat) compress 10-20× with no perceptible quality loss for speech / typical UI sounds")
                        .fix("re-encode the source to opus (preferred for sovereignty stack) or aac. If this is a transient audio asset, the Loom asset-bundle primitive should emit a compressed variant"),
                    );
                }
                "ttf" | "otf" => {
                    findings.push(
                        Finding::warn(
                            self.name(),
                            name,
                            "TTF/OTF font format — woff2 saves ~30%",
                        )
                        .citing(["perf-001", "perf-002"])
                        .why("woff2 is the modern web font format: ~30% smaller than TTF, universally supported, gzip-optimized. Larger fonts delay FOUT / FOFT and slow LCP")
                        .fix("convert via `woff2_compress` (Google's BSD-licensed tool) and emit only woff2 from the Loom font-asset pipeline. Add `font-display: swap` to the @font-face emission for FOUT protection"),
                    );
                }
                _ => {}
            }
        }

        Ok(findings)
    }
}

fn file_size(path: &Path, phase: &str) -> Result<u64, BuildError> {
    fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| BuildError::Io {
            context: format!("{phase}: stat {}", path.display()),
            source: e,
        })
}

fn has_modern_sibling(path: &Path, path_set: &std::collections::BTreeSet<&Path>) -> bool {
    has_sibling(path, path_set, "webp") || has_sibling(path, path_set, "avif")
}

fn has_sibling(path: &Path, path_set: &std::collections::BTreeSet<&Path>, ext: &str) -> bool {
    let with_ext = path.with_extension(ext);
    path_set.contains(with_ext.as_path())
}

/// Format bytes as IEC. Same impl as `perf_budget::iec` — kept
/// duplicate here to avoid a circular dep inside the phases crate.
/// If we add a third user, factor into a shared module.
fn iec(n: u64) -> String {
    const UNITS: &[&str] = &["B", "K", "M", "G"];
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}{}", n, UNITS[0])
    } else if value >= 100.0 {
        format!("{value:.0}{}", UNITS[unit])
    } else if value >= 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.2}{}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iec_smoke() {
        assert_eq!(iec(0), "0B");
        assert_eq!(iec(102_400), "100K");
    }
}
