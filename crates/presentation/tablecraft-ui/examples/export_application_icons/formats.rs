//! Native directories around unchanged, independently rendered PNG payloads.
//!
//! ICO uses little-endian directory offsets and the 0=256 dimension convention.
//! ICNS uses big-endian lengths including each eight-byte chunk header; TOC lists
//! the exact type/length pairs of the image chunks, excluding its own header.
use std::io;

const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const ICO_HEADER_BYTES: usize = 6;
const ICO_ENTRY_BYTES: usize = 16;
const ICNS_HEADER_BYTES: usize = 8;
const ICNS_IMAGES: [([u8; 4], u32); 11] = [
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
];

pub(super) struct PngIcon {
    pub(super) size: u32,
    pub(super) bytes: Vec<u8>,
}

fn length(value: usize) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| io::Error::other("native icon length exceeds u32"))
}

fn image(images: &[PngIcon], size: u32) -> io::Result<&[u8]> {
    images
        .iter()
        .find(|image| image.size == size)
        .map(|image| image.bytes.as_slice())
        .ok_or_else(|| io::Error::other(format!("missing {size}-pixel PNG for native icon")))
}

pub(super) fn ico(images: &[PngIcon]) -> io::Result<Vec<u8>> {
    let mut directory = Vec::new();
    directory.extend_from_slice(&0_u16.to_le_bytes());
    directory.extend_from_slice(&1_u16.to_le_bytes());
    directory.extend_from_slice(&(ICO_SIZES.len() as u16).to_le_bytes());
    let mut offset = length(ICO_HEADER_BYTES + ICO_ENTRY_BYTES * ICO_SIZES.len())?;
    let mut payloads = Vec::new();
    for size in ICO_SIZES {
        let png = image(images, size)?;
        let dimension = match size {
            256 => 0,
            size => u8::try_from(size).map_err(|_| io::Error::other("invalid ICO dimension"))?,
        };
        directory.extend_from_slice(&[dimension, dimension, 0, 0]);
        directory.extend_from_slice(&1_u16.to_le_bytes());
        directory.extend_from_slice(&32_u16.to_le_bytes());
        directory.extend_from_slice(&length(png.len())?.to_le_bytes());
        directory.extend_from_slice(&offset.to_le_bytes());
        offset = offset
            .checked_add(length(png.len())?)
            .ok_or_else(|| io::Error::other("ICO payload offset overflow"))?;
        payloads.extend_from_slice(png);
    }
    directory.extend(payloads);
    Ok(directory)
}

pub(super) fn icns(images: &[PngIcon]) -> io::Result<Vec<u8>> {
    let mut table = Vec::new();
    let mut chunks = Vec::new();
    for (kind, size) in ICNS_IMAGES {
        let png = image(images, size)?;
        let size = png
            .len()
            .checked_add(ICNS_HEADER_BYTES)
            .ok_or_else(|| io::Error::other("ICNS image length overflow"))?;
        let size = length(size)?.to_be_bytes();
        table.extend_from_slice(&kind);
        table.extend_from_slice(&size);
        chunks.extend_from_slice(&kind);
        chunks.extend_from_slice(&size);
        chunks.extend_from_slice(png);
    }
    let table_size = length(ICNS_HEADER_BYTES + table.len())?;
    let total = ICNS_HEADER_BYTES
        .checked_add(table_size as usize)
        .and_then(|size| size.checked_add(chunks.len()))
        .ok_or_else(|| io::Error::other("ICNS total length overflow"))?;
    let mut output = Vec::new();
    output.extend_from_slice(b"icns");
    output.extend_from_slice(&length(total)?.to_be_bytes());
    output.extend_from_slice(b"TOC ");
    output.extend_from_slice(&table_size.to_be_bytes());
    output.extend(table);
    output.extend(chunks);
    Ok(output)
}
