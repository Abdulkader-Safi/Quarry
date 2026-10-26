//! NFR-04: a damaged file of any kind gives a plain error, never a crash.
//! Real files are truncated and bit-flipped many ways and fed to every reader.

use quarry_io::settings::{Dirs, Settings};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Deterministic damaged copies: truncations and random byte flips.
fn damaged(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for k in 1..=24 {
        out.push(bytes[..bytes.len() * k / 25].to_vec());
    }
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..40 {
        let mut copy = bytes.to_vec();
        for _ in 0..8 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let at = (seed as usize) % copy.len();
            copy[at] ^= (seed >> 32) as u8 | 1;
        }
        out.push(copy);
    }
    out
}

fn never_panics(kind: &str, file: &Path, read: impl Fn(&Path)) {
    let original =
        std::fs::read(file).unwrap_or_else(|_| panic!("{kind} sample missing: {}", file.display()));
    let dir = std::env::temp_dir().join(format!("quarry-corrupt-{kind}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ext = file.extension().unwrap().to_string_lossy().into_owned();
    for (i, bytes) in damaged(&original).into_iter().enumerate() {
        let path = dir.join(format!("case{i}.{ext}"));
        std::fs::write(&path, &bytes).unwrap();
        let outcome = catch_unwind(AssertUnwindSafe(|| read(&path)));
        assert!(outcome.is_ok(), "{kind} case {i} panicked");
    }
}

#[test]
fn damaged_projects_images_gltf_and_settings_never_crash() {
    let r = root();
    never_panics(
        "project",
        &r.join("assets/samples/sample-robot.quarry"),
        |p| {
            let _ = quarry_io::project::read_project(p);
        },
    );
    never_panics("png", &r.join("assets/samples/tiles-16px.png"), |p| {
        let _ = quarry_io::images::load_image(p);
    });
    never_panics("glb", &r.join("tests/goldens/room.glb"), |p| {
        let _ = quarry_io::import::import(p);
    });
    never_panics("skinned glb", &r.join("tests/goldens/anim.glb"), |p| {
        let _ = quarry_io::import::import(p);
    });
    // Settings: write damaged JSON where Quarry looks for it.
    let dir = std::env::temp_dir().join(format!("quarry-corrupt-settings-{}", std::process::id()));
    let dirs = Dirs::in_one(dir.clone());
    std::fs::create_dir_all(&dir).unwrap();
    let good = serde_json::to_vec_pretty(&Settings::default()).unwrap();
    for bytes in damaged(&good) {
        std::fs::write(dirs.settings_file(), &bytes).unwrap();
        let (settings, _) =
            catch_unwind(|| Settings::load(&dirs)).expect("settings load never panics");
        assert!((1.0..=2.0).contains(&settings.ui_scale));
    }
}

#[test]
fn damaged_psd_never_crashes() {
    // A minimal valid PSD (as in the unit tests), then damaged.
    let mut psd = Vec::new();
    psd.extend_from_slice(b"8BPS\0\x01\0\0\0\0\0\0\0\x03\0\0\0\x02\0\0\0\x02\0\x08\0\x03");
    psd.extend_from_slice(&[0; 12]);
    psd.extend_from_slice(&[0, 0]);
    psd.extend((0..12u8).map(|i| i * 20));
    for bytes in damaged(&psd) {
        let outcome = catch_unwind(|| quarry_io::psd::read_psd(&bytes));
        assert!(outcome.is_ok(), "psd reader panicked");
    }
}
