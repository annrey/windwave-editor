//! File System Tools - Read, write, search, and edit files
//!
//! Provides safe file system operations for the Agent within the project root.
//! All operations include path traversal protection to prevent access outside
//! the allowed workspace.
//!
//! ## Tools
//! - `read_file` — Read file contents with optional line range
//! - `write_file` — Write (or create) a file with content
//! - `grep` — Search for regex patterns across files
//! - `edit_file` — Find-and-replace within a file
//! - `list_files` — List directory contents with filtering

use crate::tool::{Tool, ToolCategory, ToolParameter, ToolResult, ParameterType};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// File operation limits (configurable constants)
pub mod fs_limits {
    /// Maximum file size for read/write: 512 KB
    pub const MAX_FILE_SIZE_BYTES: usize = 512 * 1024;
    /// Maximum lines returned by read_file
    pub const MAX_READ_LINES: usize = 2000;
    /// Maximum grep results
    pub const MAX_GREP_RESULTS: usize = 50;
    /// Maximum recursion depth for list_files
    pub const MAX_RECURSION_DEPTH: usize = 10;
    /// Default line limit when not specified
    pub const DEFAULT_LINE_LIMIT: usize = 500;
    /// IO operation timeout in seconds (for spawn_blocking)
    pub const IO_TIMEOUT_SECS: u64 = 30;
}

/// Directories to skip during recursive operations
const SKIP_DIRS: &[&str] = &["target", ".git", "node_modules", "vendor", ".cargo", "build", "dist", "__pycache__", ".venv", "venv", "node_modules"];

type FErr = crate::tool::ToolError;

fn get_workspace_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn fs_resolve_and_validate(path_str: &str) -> Result<PathBuf, FErr> {
    let raw_path = PathBuf::from(path_str);
    let workspace_root = get_workspace_root();
    let resolved = if raw_path.is_absolute() {
        raw_path
    } else {
        workspace_root.join(&raw_path)
    };

    let canonical = resolved
        .canonicalize()
        .map_err(|e| FErr::ExecutionFailed(format!("Path resolution failed: {}", e)))?;

    let root_canonical = workspace_root
        .canonicalize()
        .unwrap_or(workspace_root);

    if !canonical.starts_with(&root_canonical) {
        return Err(FErr::ExecutionFailed(
            "Access denied: path is outside the allowed workspace".into(),
        ));
    }

    Ok(canonical)
}

// ===========================================================================
// ReadFileTool
// ===========================================================================

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }

    fn description(&self) -> &str {
        "Read file contents. Supports optional line range (offset/limit) for large files."
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "path".into(), description: "File path relative to project root (e.g., 'src/main.rs')".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "offset".into(), description: "Line number to start reading from (1-based, default: 1)".into(), param_type: ParameterType::Integer, required: false, default: Some(Value::Number(serde_json::Number::from(1))) },
            ToolParameter { name: "limit".into(), description: "Maximum number of lines to read (default: 500, max: 2000)".into(), param_type: ParameterType::Integer, required: false, default: Some(Value::Number(serde_json::Number::from(500))) },
        ]
    }

    fn category(&self) -> ToolCategory { ToolCategory::Utility }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, FErr> {
        let path_str = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("path".to_string()))?;
        let path = fs_resolve_and_validate(path_str)?;

        if !path.exists() { return Err(FErr::ExecutionFailed(format!("File not found: {}", path.display()))); }
        let meta = std::fs::metadata(&path).map_err(|e| FErr::ExecutionFailed(format!("Cannot read metadata: {}", e)))?;
        if meta.len() > fs_limits::MAX_FILE_SIZE_BYTES as u64 { return Err(FErr::ExecutionFailed(format!("File too large: {} bytes (max {})", meta.len(), fs_limits::MAX_FILE_SIZE_BYTES))); }

        let offset = params.get("offset").and_then(|v| v.as_i64()).unwrap_or(1).max(1) as usize;
        let limit = params.get("limit").and_then(|v| v.as_i64()).unwrap_or(fs_limits::DEFAULT_LINE_LIMIT as i64).min(fs_limits::MAX_READ_LINES as i64) as usize;
        let content = std::fs::read_to_string(&path).map_err(|e| FErr::ExecutionFailed(format!("Read failed: {}", e)))?;
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        let end = (offset + limit - 1).min(total);
        let selected = if offset > total { format!("[Offset {} exceeds total {}]", offset, total) } else {
            lines[offset.saturating_sub(1)..end].iter().enumerate().map(|(i, l)| format!("{}→{}", offset + i, l)).collect::<Vec<_>>().join("\n")
        };

        Ok(ToolResult { success: true, message: format!("Read {} lines ({}-{} of {}) from '{}'", end.saturating_sub(offset)+1, offset, end, total, path_str),
            data: Some(Value::Object([("path".into(), Value::String(path_str.into())), ("total_lines".into(), Value::Number(total.into())), ("content".into(), Value::String(selected))].into_iter().collect())), execution_time_ms: 0 })
    }
}

// ===========================================================================
// WriteFileTool
// ===========================================================================

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Write content to a file. Creates parent directories automatically." }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "path".into(), description: "Target file path relative to project root".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "content".into(), description: "Content to write to the file".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "create_dirs".into(), description: "Create parent directories if missing (default: true)".into(), param_type: ParameterType::Boolean, required: false, default: Some(Value::Bool(true)) },
        ]
    }

    fn category(&self) -> ToolCategory { ToolCategory::Utility }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, FErr> {
        let path_str = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("path".to_string()))?;
        let content = params.get("content").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("content".to_string()))?;
        let create_dirs = params.get("create_dirs").and_then(|v| v.as_bool()).unwrap_or(true);
        let path = fs_resolve_and_validate(path_str)?;

        if content.len() > fs_limits::MAX_FILE_SIZE_BYTES { return Err(FErr::ExecutionFailed(format!("Content too large: {} bytes", content.len()))); }
        if create_dirs { if let Some(p) = path.parent() { std::fs::create_dir_all(p).map_err(|e| FErr::ExecutionFailed(format!("Cannot create dir '{}': {}", p.display(), e)))?; } }
        let n = content.len();
        std::fs::write(&path, content).map_err(|e| FErr::ExecutionFailed(format!("Write failed: {}", e)))?;

        Ok(ToolResult { success: true, message: format!("Wrote {} bytes to '{}'", n, path_str),
            data: Some(Value::Object([("path".into(), Value::String(path_str.into())), ("bytes_written".into(), Value::Number(serde_json::Number::from(n)))].into_iter().collect())), execution_time_ms: 0 })
    }
}

// ===========================================================================
// GrepTool
// ===========================================================================

pub struct GrepTool;

impl Tool for GrepTool {
    fn name(&self) -> &str { "grep" }
    fn description(&self) -> &str { "Search for text patterns in files. Returns matching lines with context." }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "pattern".into(), description: "Text pattern to search for (substring match)".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "path".into(), description: "Directory or file to search (default: '.')".into(), param_type: ParameterType::String, required: false, default: Some(Value::String(".".into())) },
            ToolParameter { name: "file_pattern".into(), description: "Glob filter e.g. '*.rs'".into(), param_type: ParameterType::String, required: false, default: None },
            ToolParameter { name: "max_results".into(), description: "Max matches (default: 20, max: 50)".into(), param_type: ParameterType::Integer, required: false, default: Some(Value::Number(serde_json::Number::from(20))) },
        ]
    }

    fn category(&self) -> ToolCategory { ToolCategory::Utility }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, FErr> {
        let pattern = params.get("pattern").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("pattern".to_string()))?.to_lowercase();
        let path_str = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let file_filter = params.get("file_pattern").and_then(|v| v.as_str());
        let max_r = params.get("max_results").and_then(|v| v.as_i64()).unwrap_or(20).min(fs_limits::MAX_GREP_RESULTS as i64) as usize;

        let search_path = fs_resolve_and_validate(path_str)?;

        let mut results = Vec::new();
        let mut total = 0usize;
        let mut searched = 0usize;

        if search_path.is_file() { do_grep_file(&search_path, &pattern, &mut results, &mut total, max_r); searched = 1; }
        else if search_path.is_dir() { do_grep_dir(&search_path, &pattern, file_filter, &mut results, &mut total, &mut searched, max_r); }
        else { return Err(FErr::ExecutionFailed(format!("Path not found: {}", path_str))); }

        let n_files = results.iter().map(|r| r["file"].as_str().unwrap_or("").to_string()).collect::<std::collections::HashSet<_>>().len();

        Ok(ToolResult { success: true, message: format!("Found {} match(es) in {} file(s), searched {}", total, n_files, searched),
            data: Some(Value::Object([("pattern".into(), Value::String(pattern)), ("total_matches".into(), Value::Number(total.into())), ("results".into(), Value::Array(results))].into_iter().collect())), execution_time_ms: 0 })
    }
}

fn do_grep_file(fp: &Path, pat: &str, out: &mut Vec<Value>, total: &mut usize, max_r: usize) {
    if let Ok(content) = std::fs::read_to_string(fp) {
        for (i, line) in content.lines().enumerate() {
            if line.to_lowercase().contains(pat) {
                *total += 1;
                if out.len() < max_r { out.push(Value::Object([("file".into(), Value::String(fp.to_string_lossy().into_owned())), ("line".into(), Value::Number((i+1).into())), ("content".into(), Value::String(line.trim_end().into()))].into_iter().collect())); }
            }
        }
    }
}

fn fg_match(name: &str, pat: Option<&str>) -> bool {
    match pat { None => true, Some(p) => if p.starts_with('*') { name.ends_with(p.strip_prefix('*').unwrap()) } else { name.contains(p) } }
}

fn do_grep_dir(dir: &Path, pat: &str, ff: Option<&str>, out: &mut Vec<Value>, total: &mut usize, searched: &mut usize, max_r: usize) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                let skip = p.file_name()
                    .map_or(false, |n| SKIP_DIRS.contains(&n.to_str().unwrap_or("")));
                if !skip { do_grep_dir(&p, pat, ff, out, total, searched, max_r); }
            }
            else if p.is_file() {
                let nm = p.file_name().map_or("", |n| n.to_str().unwrap_or(""));
                if fg_match(nm, ff) { *searched += 1; do_grep_file(&p, pat, out, total, max_r); }
            }
            if *total >= max_r * 2 && !out.is_empty() { break; }
        }
    }
}

// ===========================================================================
// EditFileTool
// ===========================================================================

pub struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &str { "edit_file" }
    fn description(&self) -> &str { "Find and replace text within a file. Supports single or all occurrences." }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "path".into(), description: "File path to edit".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "old_text".into(), description: "Text to find and replace".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "new_text".into(), description: "Replacement text".into(), param_type: ParameterType::String, required: true, default: None },
            ToolParameter { name: "replace_all".into(), description: "Replace all occurrences (default: false)".into(), param_type: ParameterType::Boolean, required: false, default: Some(Value::Bool(false)) },
        ]
    }

    fn category(&self) -> ToolCategory { ToolCategory::Utility }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, FErr> {
        let path_str = params.get("path").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("path".to_string()))?;
        let old = params.get("old_text").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("old_text".to_string()))?;
        let new = params.get("new_text").and_then(|v| v.as_str()).ok_or_else(|| FErr::MissingParameter("new_text".to_string()))?;
        let all = params.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false);
        let path = fs_resolve_and_validate(path_str)?;

        if !path.exists() { return Err(FErr::ExecutionFailed(format!("File not found: {}", path.display()))); }
        let content = std::fs::read_to_string(&path).map_err(|e| FErr::ExecutionFailed(format!("Read failed: {}", e)))?;

        let nc = if all { content.replace(old, new) } else {
            match content.find(old) {
                Some(pos) => format!("{}{}{}", &content[..pos], new, &content[pos+old.len()..]),
                None => return Err(FErr::ExecutionFailed(format!("Pattern '{}' not found", old))),
            }
        };
        let count = if all { nc.matches(new).count() } else { 1 };
        std::fs::write(&path, &nc).map_err(|e| FErr::ExecutionFailed(format!("Write failed: {}", e)))?;

        Ok(ToolResult { success: true, message: format!("Replaced {} occurrence(s) of '{}' in '{}'", count, old, path_str),
            data: Some(Value::Object([("path".into(), Value::String(path_str.into())), ("replacements".into(), Value::Number(count.into()))].into_iter().collect())), execution_time_ms: 0 })
    }
}

// ===========================================================================
// ListFilesTool
// ===========================================================================

pub struct ListFilesTool;

impl Tool for ListFilesTool {
    fn name(&self) -> &str { "list_files" }
    fn description(&self) -> &str { "List files and directories. Supports recursive listing and glob filtering." }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter { name: "path".into(), description: "Directory path (default: '.')".into(), param_type: ParameterType::String, required: false, default: Some(Value::String(".".into())) },
            ToolParameter { name: "recursive".into(), description: "List recursively (default: false)".into(), param_type: ParameterType::Boolean, required: false, default: Some(Value::Bool(false)) },
            ToolParameter { name: "pattern".into(), description: "Glob filter e.g. '*.rs'".into(), param_type: ParameterType::String, required: false, default: None },
        ]
    }

    fn category(&self) -> ToolCategory { ToolCategory::Utility }

    fn execute(&self, params: HashMap<String, Value>) -> Result<ToolResult, FErr> {
        let path_str = params.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let recursive = params.get("recursive").and_then(|v| v.as_bool()).unwrap_or(false);
        let pattern = params.get("pattern").and_then(|v| v.as_str());
        let dir_path = fs_resolve_and_validate(path_str)?;

        if !dir_path.exists() { return Err(FErr::ExecutionFailed(format!("Path not found: {}", path_str))); }
        if !dir_path.is_dir() { return Err(FErr::ExecutionFailed(format!("Not a directory: {}", path_str))); }

        let mut entries = Vec::new();
        collect_entries(&dir_path, "", recursive, pattern, &mut entries, 0);
        let dirs = entries.iter().filter(|e| e["is_dir"].as_bool().unwrap_or(false)).count();
        let files = entries.len() - dirs;

        Ok(ToolResult { success: true, message: format!("Found {} item(s): {} file(s), {} dir(s)", entries.len(), files, dirs),
            data: Some(Value::Object([("path".into(), Value::String(path_str.into())), ("entries".into(), Value::Array(entries))].into_iter().collect())), execution_time_ms: 0 })
    }
}

fn collect_entries(base: &Path, rel: &str, recursive: bool, pat: Option<&str>, out: &mut Vec<Value>, depth: usize) {
    if depth > fs_limits::MAX_RECURSION_DEPTH { return; }
    let mut items: Vec<(String, bool)> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(base) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_d = e.path().is_dir();
            if fg_match(&name, pat) || is_d { items.push((name, is_d)); }
        }
    }
    items.sort_by(|a, b| match (a.1, b.1) { (true, false) => std::cmp::Ordering::Less, (false, true) => std::cmp::Ordering::Greater, _ => a.0.cmp(&b.0) });

    for (name, is_dir) in items {
        let r = if rel.is_empty() { name.clone() } else { format!("{}/{}", rel, name) };
        out.push(Value::Object([("name".into(), Value::String(name.clone())), ("relative_path".into(), Value::String(r.clone())), ("is_dir".into(), Value::Bool(is_dir))].into_iter().collect()));
        if is_dir && recursive && !SKIP_DIRS.contains(&name.as_str()) { collect_entries(&base.join(&name), &r, recursive, pat, out, depth + 1); }
    }
}

// ===========================================================================
// Registration
// ===========================================================================

pub fn register_file_tools(registry: &mut crate::tool::ToolRegistry) {
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(GrepTool);
    registry.register(EditFileTool);
    registry.register(ListFilesTool);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn test_read_params() { let t = ReadFileTool; assert_eq!(t.name(), "read_file"); assert_eq!(t.category(), ToolCategory::Utility); assert!(t.parameters()[0].required); }
    #[test] fn test_write_params() { let t = WriteFileTool; assert_eq!(t.name(), "write_file"); assert!(t.parameters().iter().any(|p| p.name == "content" && p.required)); }
    #[test] fn test_grep_params() { let t = GrepTool; assert_eq!(t.name(), "grep"); assert!(t.parameters().iter().any(|p| p.name == "pattern" && p.required)); }
    #[test] fn test_edit_params() { let t = EditFileTool; assert_eq!(t.name(), "edit_file"); assert_eq!(t.parameters().len(), 4); }
    #[test] fn test_list_params() { let t = ListFilesTool; assert_eq!(t.name(), "list_files"); assert_eq!(t.category(), ToolCategory::Utility); }

    #[test]
    fn test_write_read_roundtrip() {
        let td = std::env::temp_dir().join("ft_test");
        let _ = std::fs::create_dir_all(&td);
        let f = td.join("rt.txt");

        let w = WriteFileTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String(f.to_string_lossy().into_owned()));
        p.insert("content".into(), Value::String("hello\nworld\n".into()));
        assert!(w.execute(p).unwrap().success);

        let r = ReadFileTool;
        let mut p2 = HashMap::new();
        p2.insert("path".into(), Value::String(f.to_string_lossy().into_owned()));
        let res = r.execute(p2).unwrap();
        assert!(res.data.unwrap()["content"].as_str().unwrap().contains("hello"));
        let _ = std::fs::remove_dir_all(td);
    }

    #[test]
    fn test_grep_finds() {
        let td = std::env::temp_dir().join("ft_grep");
        let _ = std::fs::create_dir_all(&td);
        let f = td.join("g.rs");
        std::fs::write(&f, "fn main() {\n    println!(\"hi\");\n}\n").unwrap();

        let g = GrepTool;
        let mut p = HashMap::new();
        p.insert("pattern".into(), Value::String("println".into()));
        p.insert("path".into(), Value::String(f.to_string_lossy().into_owned()));
        let res = g.execute(p).unwrap();
        assert!(!res.data.unwrap()["results"].as_array().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(td);
    }

    #[test]
    fn test_edit_replace() {
        let td = std::env::temp_dir().join("ft_edit");
        let _ = std::fs::create_dir_all(&td);
        let f = td.join("e.txt");
        std::fs::write(&f, "foo bar baz foo qux").unwrap();

        let e = EditFileTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String(f.to_string_lossy().into_owned()));
        p.insert("old_text".into(), Value::String("foo".into()));
        p.insert("new_text".into(), Value::String("OK".into()));
        p.insert("replace_all".into(), Value::Bool(true));
        let res = e.execute(p).unwrap();
        assert!(res.message.contains("2"));
        let c = std::fs::read_to_string(&f).unwrap();
        assert!(c.contains("OK")); assert!(!c.contains("foo"));
        let _ = std::fs::remove_dir_all(td);
    }

    #[test]
    fn test_list_files() {
        let td = std::env::temp_dir().join("ft_list");
        let _ = std::fs::create_dir_all(&td);
        std::fs::write(td.join("a.rs"), "//a").unwrap();
        std::fs::write(td.join("b.toml"), "[b]").unwrap();

        let l = ListFilesTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String(td.to_string_lossy().into_owned()));
        let res = l.execute(p).unwrap();
        assert!(res.data.unwrap()["entries"].as_array().unwrap().len() >= 2);
        let _ = std::fs::remove_dir_all(td);
    }

    #[test]
    fn test_traversal_blocked() {
        let r = ReadFileTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String("../../../etc/passwd".into()));
        assert!(r.execute(p).is_err());
    }

    #[test]
    fn test_absolute_path_outside_workspace() {
        let r = ReadFileTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String("/etc/passwd".into()));
        assert!(r.execute(p).is_err());
    }

    #[test]
    fn test_symlink_traversal_blocked() {
        let r = ReadFileTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String("./normal/../secret".into()));
        assert!(r.execute(p).is_err());
    }

    #[test]
    fn test_edit_not_found() {
        let td = std::env::temp_dir().join("ft_enf");
        let _ = std::fs::create_dir_all(&td);
        let f = td.join("nope.txt");
        std::fs::write(&f, "hi").unwrap();

        let e = EditFileTool;
        let mut p = HashMap::new();
        p.insert("path".into(), Value::String(f.to_string_lossy().into_owned()));
        p.insert("old_text".into(), Value::String("NOPE".into()));
        p.insert("new_text".into(), Value::String("x".into()));
        assert!(e.execute(p).is_err());
        let _ = std::fs::remove_dir_all(td);
    }

    #[test]
    fn test_register() {
        use crate::tool::ToolRegistry;
        let mut r = ToolRegistry::new();
        register_file_tools(&mut r);
        let names: std::collections::HashSet<String> = r.list_tools().into_iter().collect();
        for n in &["read_file","write_file","grep","edit_file","list_files"] { assert!(names.contains(&n.to_string())); }
        assert_eq!(r.list_tools().len(), 5);
    }
}
