//! Asset Import Pipeline — Independent import subsystem
//!
//! CocoIndex-inspired: hash-based incremental, modular format handlers.
//! Interacts with: ProjectManifest (assets_dir), AssetBrowser (display),
//! Scene (apply texture to Sprite).
//!
//! Architecture:
//!   AssetImportManager (core) → AssetImportEvent (signal) → handlers

use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Import format registry
// ---------------------------------------------------------------------------

/// Supported import formats with handler capabilities
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    Texture,   // png, jpg, webp, gif, bmp, tga
    Audio,     // wav, mp3, ogg, flac
    Model3D,   // gltf, glb, obj, fbx (limited to gltf for Bevy)
    Font,      // ttf, otf
    Scene,     // .scene (Bevy scene files)
    Prefab,    // .prefab
    Unknown,
}

impl ImportFormat {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tga" => ImportFormat::Texture,
            "wav" | "mp3" | "ogg" | "flac" => ImportFormat::Audio,
            "gltf" | "glb" => ImportFormat::Model3D,
            "obj" | "fbx" => ImportFormat::Unknown, // Bevy doesn't natively support
            "ttf" | "otf" => ImportFormat::Font,
            "scene" => ImportFormat::Scene,
            "prefab" => ImportFormat::Prefab,
            _ => ImportFormat::Unknown,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Texture => "Texture",
            Self::Audio => "Audio",
            Self::Model3D => "3D Model",
            Self::Font => "Font",
            Self::Scene => "Scene",
            Self::Prefab => "Prefab",
            Self::Unknown => "Unknown",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Self::Texture => "🖼️", Self::Audio => "🔊", Self::Model3D => "🔷",
            Self::Font => "🔤", Self::Scene => "🗺️", Self::Prefab => "📦",
            Self::Unknown => "📄",
        }
    }
}

// ---------------------------------------------------------------------------
// Import manifest — tracks imported assets per project
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportManifest {
    pub version: u32,
    /// Map file_path → file hash (sha256 hex) for incremental detection
    pub imported: HashMap<String, ImportedAsset>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportedAsset {
    pub path: String,
    pub format: String,
    pub hash: String,
    pub size_bytes: u64,
    pub imported_at: String,
}

impl ImportManifest {
    pub fn new() -> Self {
        Self { version: 1, imported: HashMap::new() }
    }

    pub fn is_already_imported(&self, path: &str, hash: &str) -> bool {
        self.imported.get(path).map_or(false, |a| a.hash == hash)
    }

    pub fn mark_imported(&mut self, path: &str, format: ImportFormat, hash: &str, size: u64) {
        self.imported.insert(path.to_string(), ImportedAsset {
            path: path.to_string(),
            format: format!("{:?}", format),
            hash: hash.to_string(),
            size_bytes: size,
            imported_at: chrono::Utc::now().to_rfc3339(),
        });
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, json)
    }

    pub fn load(path: &Path) -> std::io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::new()),
            Err(e) => Err(e),
        }
    }
}

// ---------------------------------------------------------------------------
// File scanner — walks assets_dir, detects new/changed files
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub new_files: Vec<PathBuf>,
    pub modified_files: Vec<PathBuf>,
    pub unchanged: usize,
    pub total: usize,
}

/// Scan the assets directory and return new/modified files.
/// Uses size+mtime fingerprint for lightweight incremental detection.
pub fn scan_assets(
    assets_dir: &Path,
    manifest: &ImportManifest,
) -> std::io::Result<ScanResult> {
    let mut result = ScanResult {
        new_files: Vec::new(), modified_files: Vec::new(), unchanged: 0, total: 0,
    };
    if !assets_dir.exists() { return Ok(result); }
    scan_dir(assets_dir, assets_dir, manifest, &mut result)?;
    Ok(result)
}

fn scan_dir(
    base: &Path, dir: &Path, manifest: &ImportManifest, result: &mut ScanResult,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden dirs and build artifacts
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "target" || name == "node_modules" { continue; }
            scan_dir(base, &path, manifest, result)?;
        } else if path.is_file() {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ImportFormat::from_extension(ext) == ImportFormat::Unknown { continue; }
            result.total += 1;
            let meta = entry.metadata()?;
            let fp = file_fingerprint(&meta);
            let rel = path.strip_prefix(base).unwrap_or(&path).to_string_lossy().to_string();
            if manifest.is_already_imported(&rel, &fp) {
                result.unchanged += 1;
            } else if manifest.imported.contains_key(&rel) {
                result.modified_files.push(path);
            } else {
                result.new_files.push(path);
            }
        }
    }
    Ok(())
}

fn file_fingerprint(meta: &std::fs::Metadata) -> String {
    format!("{}:{}", meta.len(), meta.modified().map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()).unwrap_or(0))
}

// ---------------------------------------------------------------------------
// Bevy asset import helpers
// ---------------------------------------------------------------------------

/// Determine if a texture path is valid for Bevy AssetServer
pub fn is_supported_texture(path: &Path) -> bool {
    matches!(path.extension().and_then(|e| e.to_str()),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tga"))
}

/// Build the asset path relative to assets_dir for Bevy AssetServer
pub fn to_asset_path(full_path: &Path, assets_dir: &Path) -> String {
    full_path.strip_prefix(assets_dir)
        .unwrap_or(Path::new(full_path.file_name().unwrap_or(full_path.as_ref())))
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_from_extension() {
        assert_eq!(ImportFormat::from_extension("png"), ImportFormat::Texture);
        assert_eq!(ImportFormat::from_extension("wav"), ImportFormat::Audio);
        assert_eq!(ImportFormat::from_extension("gltf"), ImportFormat::Model3D);
        assert_eq!(ImportFormat::from_extension("ttf"), ImportFormat::Font);
        assert_eq!(ImportFormat::from_extension("scene"), ImportFormat::Scene);
        assert_eq!(ImportFormat::from_extension("exe"), ImportFormat::Unknown);
    }

    #[test]
    fn test_manifest_save_load() {
        let tmp = std::env::temp_dir().join("test_import_manifest.json");
        let mut m = ImportManifest::new();
        m.mark_imported("tex/player.png", ImportFormat::Texture, "abc123", 1024);
        m.save(&tmp).unwrap();

        let loaded = ImportManifest::load(&tmp).unwrap();
        assert!(loaded.is_already_imported("tex/player.png", "abc123"));
        assert!(!loaded.is_already_imported("tex/player.png", "wrong"));
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn test_scan_empty_dir() {
        let dir = std::env::temp_dir().join("test_empty_assets");
        let _ = std::fs::create_dir_all(&dir);
        let manifest = ImportManifest::new();
        let result = scan_assets(&dir, &manifest).unwrap();
        assert_eq!(result.total, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
