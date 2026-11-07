use super::*;
use image::{ImageFormat, RgbaImage, imageops::FilterType};
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};

static IMAGES: OnceLock<Vec<PngIcon>> = OnceLock::new();
static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

fn images() -> &'static [PngIcon] {
    IMAGES.get_or_init(|| {
        PNG_SIZES
            .into_iter()
            .map(|size| rasterize(size).unwrap())
            .collect()
    })
}

fn decode(png: &[u8], expected: u32) -> RgbaImage {
    let image = image::load_from_memory_with_format(png, ImageFormat::Png)
        .unwrap()
        .to_rgba8();
    assert_eq!(image.dimensions(), (expected, expected));
    assert_eq!(image.get_pixel(0, 0).0[3], 0, "original transparent canvas");
    assert_eq!(
        image.get_pixel(expected / 2, expected / 2).0,
        [177, 43, 61, 255],
        "original red database cylinder, not an empty or substituted icon"
    );
    image
}

fn standalone(size: u32) -> RgbaImage {
    decode(
        &images()
            .iter()
            .find(|image| image.size == size)
            .unwrap()
            .bytes,
        size,
    )
}

#[test]
fn every_png_decodes_at_native_resolution_with_original_artwork() {
    assert_eq!(images().len(), 9);
    for expected in [16, 24, 32, 48, 64, 128, 256, 512, 1024] {
        standalone(expected);
    }
    let large = standalone(1024);
    let enlarged = image::imageops::resize(&standalone(32), 1024, 1024, FilterType::Nearest);
    assert_ne!(
        large, enlarged,
        "large artwork is freshly rasterized vector geometry"
    );
}

#[test]
fn ico_directory_locates_seven_complete_png_images_with_correct_dimensions() {
    let encoded = formats::ico(images()).unwrap();
    assert_eq!(&encoded[..6], &[0, 0, 1, 0, 7, 0]);
    let mut previous_end = 6 + 7 * 16;
    for (index, expected) in [16, 24, 32, 48, 64, 128, 256].into_iter().enumerate() {
        let entry = &encoded[6 + index * 16..6 + (index + 1) * 16];
        let dimension = |value| if value == 0 { 256 } else { u32::from(value) };
        assert_eq!(
            (dimension(entry[0]), dimension(entry[1])),
            (expected, expected)
        );
        assert_eq!(&entry[2..8], &[0, 0, 1, 0, 32, 0]);
        let bytes = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        assert_eq!(
            offset, previous_end,
            "directory has no overlap or hidden gap"
        );
        let end = offset.checked_add(bytes).unwrap();
        let decoded = decode(encoded.get(offset..end).unwrap(), expected);
        assert_eq!(decoded, standalone(expected));
        previous_end = end;
    }
    assert_eq!(previous_end, encoded.len(), "no unreferenced trailing data");
}

#[test]
fn icns_table_matches_all_decodable_native_and_retina_png_chunks() {
    let encoded = formats::icns(images()).unwrap();
    assert_eq!(&encoded[..4], b"icns");
    assert_eq!(
        u32::from_be_bytes(encoded[4..8].try_into().unwrap()) as usize,
        encoded.len()
    );
    assert_eq!(&encoded[8..12], b"TOC ");
    let table_bytes = u32::from_be_bytes(encoded[12..16].try_into().unwrap()) as usize;
    assert_eq!(table_bytes, 8 + 11 * 8);
    let table = &encoded[16..8 + table_bytes];
    let mut offset = 8 + table_bytes;
    let mut observed = Vec::new();
    for (kind, expected) in [
        (*b"icp4", 16),
        (*b"icp5", 32),
        (*b"icp6", 64),
        (*b"ic07", 128),
        (*b"ic08", 256),
        (*b"ic09", 512),
        (*b"ic10", 1024),
        (*b"ic11", 32),
        (*b"ic12", 64),
        (*b"ic13", 256),
        (*b"ic14", 512),
    ] {
        let header = encoded.get(offset..offset + 8).unwrap();
        assert_eq!(&header[..4], &kind);
        observed.extend_from_slice(header);
        let length = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
        assert!(length > 8);
        let end = offset.checked_add(length).unwrap();
        let decoded = decode(encoded.get(offset + 8..end).unwrap(), expected);
        assert_eq!(decoded, standalone(expected));
        offset = end;
    }
    assert_eq!(
        observed, table,
        "TOC describes the actual encoded image chunks"
    );
    assert_eq!(offset, encoded.len(), "all chunks end at declared EOF");
}

struct OutputDirectory(PathBuf);

impl OutputDirectory {
    fn new() -> Self {
        for _ in 0..16 {
            let id = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
            let path =
                env::temp_dir().join(format!("tablecraft-icon-test-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create owned icon test directory: {error}"),
            }
        }
        panic!("owned icon test directory collision limit");
    }
}

impl Drop for OutputDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn existing_output_directories_and_files_are_never_replaced() {
    let directory = OutputDirectory::new();
    let sentinel = directory.0.join("preserve.txt");
    fs::write(&sentinel, b"preserve original").unwrap();
    assert!(export(&directory.0).is_err());
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    assert!(write_new(&sentinel, b"replacement").is_err());
    assert_eq!(fs::read(&sentinel).unwrap(), b"preserve original");
    assert!(formats::ico(&[]).is_err());
    assert!(formats::icns(&[]).is_err());
}
