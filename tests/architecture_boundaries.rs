use std::{
    fs,
    path::{Path, PathBuf},
};

fn rust_files(root: &Path) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(dir).expect("read source directory") {
            let path = entry.expect("read directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files
}

fn assert_sources_exclude(root: &Path, forbidden: &str) {
    let violations: Vec<_> = rust_files(root)
        .into_iter()
        .filter(|path| {
            fs::read_to_string(path)
                .expect("read Rust file")
                .contains(forbidden)
        })
        .collect();
    assert!(
        violations.is_empty(),
        "forbidden `{forbidden}` in {violations:#?}"
    );
}

#[test]
fn bevy_adapter_uses_scene_port() {
    assert_sources_exclude(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/bevy-adapter/src"),
        "agent_core::scene_bridge",
    );
}

#[test]
fn agent_ui_has_no_multica_source_dependency() {
    assert_sources_exclude(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/agent-ui/src"),
        "multica_bridge",
    );
}

#[test]
fn agent_ui_manifest_has_no_multica_dependency() {
    let manifest = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("crates/agent-ui/Cargo.toml"),
    )
    .expect("read agent-ui manifest");
    assert!(!manifest.contains("multica-bridge"));
}
