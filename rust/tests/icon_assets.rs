use std::{collections::BTreeSet, fs, path::PathBuf};

fn icon_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("icons")
        .join(file_name)
}

#[test]
fn application_icon_has_an_editable_graphite_knot_source() {
    let path = icon_path("codex-barbar.svg");
    let svg = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} must exist: {error}", path.display()));

    assert!(svg.contains("viewBox=\"0 0 1024 1024\""));
    assert!(svg.contains("#10131A"), "graphite base is missing");
    assert!(svg.contains("#56D98A"), "emerald accent is missing");
    assert!(
        svg.contains("M21.55 10.004"),
        "the shared ChatGPT-style knot geometry is missing"
    );
}

#[test]
fn application_png_is_a_1024_pixel_rgba_asset() {
    let bytes = fs::read(icon_path("codex-barbar.png")).expect("application PNG must be readable");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()), 1024);
    assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 1024);
    assert_eq!(bytes[25], 6, "PNG must use RGBA color type");
}

#[test]
fn application_ico_contains_every_windows_small_size_frame() {
    let bytes = fs::read(icon_path("codex-barbar.ico")).expect("application ICO must be readable");
    assert_eq!(&bytes[..4], &[0, 0, 1, 0]);
    let count = u16::from_le_bytes(bytes[4..6].try_into().unwrap()) as usize;
    assert_eq!(
        count, 8,
        "ICO must contain exactly the eight supported frames"
    );
    let mut dimensions = BTreeSet::new();
    for entry in 0..count {
        let offset = 6 + entry * 16;
        let width = match bytes[offset] {
            0 => 256,
            value => u16::from(value),
        };
        let height = match bytes[offset + 1] {
            0 => 256,
            value => u16::from(value),
        };
        assert_eq!(width, height, "ICO frames must be square");
        dimensions.insert(width);
    }

    assert_eq!(
        dimensions,
        BTreeSet::from([16, 20, 24, 32, 48, 64, 128, 256])
    );
}
