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

fn normalized_use_statements(source: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if current.is_empty() && !trimmed.starts_with("use ") {
            continue;
        }
        current.push_str(trimmed);
        if trimmed.ends_with(';') {
            statements.push(current.chars().filter(|ch| !ch.is_whitespace()).collect());
            current.clear();
        }
    }
    statements
}

fn use_imports_type_from(statement: &str, module: &str, type_name: &str) -> bool {
    let single = format!("use{module}::{type_name};");
    if statement == single {
        return true;
    }

    let group_prefix = format!("use{module}::{{");
    statement
        .strip_prefix(&group_prefix)
        .and_then(|items| items.strip_suffix("};"))
        .is_some_and(|items| {
            items
                .split(',')
                .any(|item| item == type_name || item.starts_with(&format!("{type_name}as")))
        })
}

fn use_statement_imports_type(statement: &str, type_name: &str) -> bool {
    statement
        .trim_start_matches("use")
        .trim_end_matches(';')
        .split(['{', '}', ','])
        .map(|item| item.trim())
        .any(|item| {
            item == type_name
                || item.ends_with(&format!("::{type_name}"))
                || item.starts_with(&format!("{type_name}as"))
                || item.contains(&format!("::{type_name}as"))
        })
}

fn application_import_error(source: &str, type_name: &str) -> Option<String> {
    let uses = normalized_use_statements(source);
    let application_imports = uses
        .iter()
        .filter(|statement| use_imports_type_from(statement, "agent_core::application", type_name))
        .count();
    let legacy_imports: Vec<_> = uses
        .iter()
        .filter(|statement| {
            statement.starts_with("useagent_core::")
                && !statement.starts_with("useagent_core::application::")
                && use_statement_imports_type(statement, type_name)
        })
        .collect();
    let legacy_qualified = source.contains(&format!("agent_core::director::{type_name}"))
        || source.contains(&format!("agent_core::open_world_runtime::{type_name}"))
        || source.contains(&format!("agent_core::open_world_timeline::{type_name}"));

    if application_imports == 1 && legacy_imports.is_empty() && !legacy_qualified {
        None
    } else {
        Some(format!(
            "expected exactly one agent_core::application import for {type_name}; found {application_imports}, legacy imports {legacy_imports:?}, legacy qualified path={legacy_qualified}"
        ))
    }
}

fn assert_source_uses_application_facade(relative_path: &str, type_names: &[&str]) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let source = fs::read_to_string(&path).expect("read Rust source file");
    for type_name in type_names {
        assert!(
            application_import_error(&source, type_name).is_none(),
            "{relative_path}: {}",
            application_import_error(&source, type_name).unwrap()
        );
    }
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

#[test]
fn application_consumers_use_stable_facade() {
    for (relative_path, type_names) in [
        (
            "crates/agent-ui/src/world_timeline_panel.rs",
            &[
                "DirectorRuntime",
                "OpenWorldReplayWorldState",
                "OpenWorldTimeline",
                "OpenWorldTimelineTick",
            ][..],
        ),
        (
            "crates/agent-ui/src/open_world_quest_panel.rs",
            &["OpenWorldReplayWorldState", "OpenWorldRuntimeState"][..],
        ),
        (
            "crates/agent-ui/src/open_world_interaction.rs",
            &["OpenWorldRuntimeError", "OpenWorldRuntimeState"][..],
        ),
        ("src/main.rs", &["DirectorRuntime"][..]),
    ] {
        assert_source_uses_application_facade(relative_path, type_names);
    }

    let formatted_application_fixture = r#"
        use agent_core::application::{
            DirectorRuntime,
            OpenWorldTimeline,
        };
    "#;
    assert!(application_import_error(formatted_application_fixture, "DirectorRuntime").is_none());

    for legacy_fixture in [
        "use agent_core::{DirectorRuntime, EditorMode};",
        "use agent_core::director::DirectorRuntime;",
        "use agent_core::{director::DirectorRuntime, EditorMode};",
        "fn legacy(runtime: agent_core::director::DirectorRuntime) {}",
    ] {
        assert!(
            application_import_error(legacy_fixture, "DirectorRuntime").is_some(),
            "legacy fixture must be rejected: {legacy_fixture}"
        );
    }
}
