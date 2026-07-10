use agent_core::DirectorRuntime;
use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args_os().skip(1);
    let markdown_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/qa/open-world-slice01.md"));
    let mut timeline_path = None;
    let mut visual_snapshot_path = None;
    while let Some(flag) = args.next() {
        if flag == "--timeline-json" {
            timeline_path = args.next().map(PathBuf::from);
        } else if flag == "--visual-snapshot-png" {
            visual_snapshot_path = args.next().map(PathBuf::from);
        }
    }

    let mut director = DirectorRuntime::new();
    director.write_open_world_slice01_qa_artifacts(
        markdown_path,
        timeline_path,
        visual_snapshot_path,
        Vec::new(),
    )?;

    Ok(())
}
