//! Performance benchmarks for file system tools.
//!
//! Measures read, write, grep, and edit operations under realistic file sizes.
use agent_core::file_tools::{EditFileTool, GrepTool, ListFilesTool, ReadFileTool, WriteFileTool};
use agent_core::tool::Tool;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Create a temporary directory with benchmark files.
fn setup_bench_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agent_bench_{}", std::process::id()));
    let _ = fs::create_dir_all(&dir);

    // Create a medium file (~50KB) for read benchmarks
    let content: String = (0..2000)
        .map(|i| format!("Line {}: fn process_entity_{}(&mut self, params: &ParamSet) -> Result<(), Error> {{\n    let entity = self.world.get_mut::<Transform>(entity_{})?;\n    entity.translation = Vec3::new({:.1}, {:.1}, {:.1});\n    Ok(())\n}}\n", i, i, i, i as f32, (i as f32) * 1.5, (i as f32) * 0.5))
        .collect();
    fs::write(dir.join("bench_file.rs"), &content).unwrap();

    // Create a small directory tree for grep/list benchmarks
    let sub = dir.join("subdir");
    fs::create_dir_all(&sub).unwrap();
    for i in 0..20 {
        let cf: String = (0..100)
            .map(|j| {
                format!(
                    "fn helper_{}_{}(&self) -> u32 {{ {} }}\n",
                    i,
                    j,
                    i * 100 + j
                )
            })
            .collect();
        fs::write(sub.join(format!("mod_{}.rs", i)), &cf).unwrap();
    }

    dir
}

fn bench_read_file(c: &mut Criterion) {
    let dir = setup_bench_dir();
    let tool = ReadFileTool;
    let file_path = dir.join("bench_file.rs").to_string_lossy().into_owned();

    let mut group = c.benchmark_group("read_file");
    group.sample_size(100);

    group.bench_function("read_full", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(file_path.clone()));
            let _ = tool.execute(black_box(params));
        });
    });

    group.bench_function("read_offset_limit", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(file_path.clone()));
            params.insert("offset".into(), Value::Number(500.into()));
            params.insert("limit".into(), Value::Number(100.into()));
            let _ = tool.execute(black_box(params));
        });
    });

    group.finish();
    let _ = fs::remove_dir_all(&dir);
}

fn bench_grep(c: &mut Criterion) {
    let dir = setup_bench_dir();
    let tool = GrepTool;

    let mut group = c.benchmark_group("grep");
    group.sample_size(100);

    let dir_str = dir.to_string_lossy().into_owned();

    group.bench_function("grep_single_file", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("pattern".into(), Value::String("process_entity".into()));
            params.insert(
                "path".into(),
                Value::String(dir.join("bench_file.rs").to_string_lossy().into_owned()),
            );
            params.insert("recursive".into(), Value::Bool(false));
            let _ = tool.execute(black_box(params));
        });
    });

    group.bench_function("grep_recursive_dir", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("pattern".into(), Value::String("helper".into()));
            params.insert("path".into(), Value::String(dir_str.clone()));
            params.insert("recursive".into(), Value::Bool(true));
            let _ = tool.execute(black_box(params));
        });
    });

    group.finish();
    let _ = fs::remove_dir_all(&dir);
}

fn bench_write_edit(c: &mut Criterion) {
    let dir = setup_bench_dir();
    let tmp_file = dir.join("tmp_edit.rs");
    fs::write(&tmp_file, "line one\nline two\nline three\nline four\n").unwrap();
    let file_path = tmp_file.to_string_lossy().into_owned();

    let mut group = c.benchmark_group("write_edit");
    group.sample_size(200);

    group.bench_function("write_small", |b| {
        let wf = dir.join("tmp_write.rs");
        let wp = wf.to_string_lossy().into_owned();
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(wp.clone()));
            params.insert(
                "content".into(),
                Value::String("benchmark content\n".into()),
            );
            params.insert("create_dirs".into(), Value::Bool(false));
            let tool = WriteFileTool;
            let _ = tool.execute(black_box(params));
        });
    });

    group.bench_function("edit_single_occurrence", |b| {
        let edit = EditFileTool;
        b.iter(|| {
            // Reset file each iteration
            fs::write(&tmp_file, "line one\nline two\nline three\nline four\n").unwrap();
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(file_path.clone()));
            params.insert("old_str".into(), Value::String("line two".into()));
            params.insert("new_str".into(), Value::String("REPLACED".into()));
            let _ = edit.execute(black_box(params));
        });
    });

    group.finish();
    let _ = fs::remove_dir_all(&dir);
}

fn bench_list_files(c: &mut Criterion) {
    let dir = setup_bench_dir();
    let tool = ListFilesTool;
    let dir_str = dir.to_string_lossy().into_owned();

    let mut group = c.benchmark_group("list_files");
    group.sample_size(200);

    group.bench_function("list_flat", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(dir_str.clone()));
            params.insert("recursive".into(), Value::Bool(false));
            let _ = tool.execute(black_box(params));
        });
    });

    group.bench_function("list_recursive", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(dir_str.clone()));
            params.insert("recursive".into(), Value::Bool(true));
            let _ = tool.execute(black_box(params));
        });
    });

    group.bench_function("list_with_pattern", |b| {
        b.iter(|| {
            let mut params = HashMap::new();
            params.insert("path".into(), Value::String(dir_str.clone()));
            params.insert("pattern".into(), Value::String("*.rs".into()));
            let _ = tool.execute(black_box(params));
        });
    });

    group.finish();
    let _ = fs::remove_dir_all(&dir);
}

criterion_group!(
    benches,
    bench_read_file,
    bench_grep,
    bench_write_edit,
    bench_list_files
);
criterion_main!(benches);
