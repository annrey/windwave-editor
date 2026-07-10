//! Deterministic QA visual snapshots for open-world verification bundles.
//!
//! This module writes a lightweight PNG from verification evidence. It is a
//! SceneIndex proxy artifact, not a Bevy framebuffer capture.

use crate::open_world_verification::OpenWorldVerificationBundle;
use std::fs::File;
use std::io;
use std::path::Path;

const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;

pub fn write_open_world_visual_snapshot_png(
    bundle: &OpenWorldVerificationBundle,
    path: impl AsRef<Path>,
) -> io::Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut pixels = vec![0_u8; (WIDTH * HEIGHT * 4) as usize];
    fill_rect(&mut pixels, 0, 0, WIDTH, HEIGHT, [18, 24, 32, 255]);
    fill_rect(&mut pixels, 0, 300, WIDTH, 60, [34, 72, 50, 255]);
    fill_rect(&mut pixels, 40, 48, 560, 236, [46, 54, 66, 255]);

    let targets = [
        ("player", [82, 168, 255, 255], 96, 130),
        ("puzzle_switch", [245, 197, 66, 255], 224, 112),
        ("reward_chest", [152, 101, 45, 255], 352, 168),
        ("camp_enemy_01", [224, 74, 74, 255], 480, 122),
    ];

    for (target, color, x, y) in targets {
        let visible = bundle
            .evidence
            .scene_index_observations
            .iter()
            .any(|observation| observation.contains(target));
        let color = if visible { color } else { [82, 88, 96, 255] };
        fill_rect(&mut pixels, x, y, 48, 48, color);
        fill_rect(&mut pixels, x + 8, y + 8, 32, 32, [250, 250, 250, 80]);
    }

    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(file, WIDTH, HEIGHT);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(png_error)?;
    writer.write_image_data(&pixels).map_err(png_error)
}

fn fill_rect(pixels: &mut [u8], x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
    for row in y..(y + height).min(HEIGHT) {
        for col in x..(x + width).min(WIDTH) {
            let offset = ((row * WIDTH + col) * 4) as usize;
            pixels[offset..offset + 4].copy_from_slice(&color);
        }
    }
}

fn png_error(error: png::EncodingError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        OpenWorldPlan, OpenWorldVerificationBundle, PlayableScenario, PlayableScenarioState,
    };

    #[test]
    fn visual_snapshot_writer_creates_png_file() {
        let plan = OpenWorldPlan::open_world_slice01_fixture();
        let scenario = PlayableScenario::open_world_slice01_main_path(&plan);
        let mut state = PlayableScenarioState::from_plan(&plan);
        let report = scenario.run(&mut state);
        let bundle = OpenWorldVerificationBundle::from_playtest(&plan, &scenario, &state, &report);
        let temp_file = tempfile::Builder::new()
            .prefix("windwave-open-world-visual-snapshot-writer-")
            .suffix(".png")
            .tempfile()
            .unwrap();
        let path = temp_file.path().to_path_buf();

        write_open_world_visual_snapshot_png(&bundle, &path).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
