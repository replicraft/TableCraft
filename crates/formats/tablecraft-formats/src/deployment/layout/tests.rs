//! Virtual CAB geometry controls, not authored native database/decompression evidence.
use super::super::hybrid;
use super::*;
use std::io::{self, SeekFrom};
use std::{cell::Cell, io::Cursor};

struct VirtualCabinet {
    prefix: Vec<u8>,
    position: u64,
    physical_bytes: u64,
    data_offset: u64,
    block_stride: u64,
    maximum_request: usize,
}

impl VirtualCabinet {
    fn new(blocks: u16, reserve_bytes: u8) -> Self {
        // Independent literal CAB layout: one folder, one file named b, MSZIP.
        // Payload bytes are virtual zeros and deliberately not valid compressed
        // data. These tests exercise only the pre-decoder geometry boundary.
        let mut prefix = vec![0; if reserve_bytes == 0 { 62 } else { 66 }];
        prefix[..4].copy_from_slice(b"MSCF");
        prefix[24..26].copy_from_slice(&[3, 1]);
        prefix[26..28].copy_from_slice(&[1, 0]);
        prefix[28..30].copy_from_slice(&[1, 0]);
        let folder = if reserve_bytes == 0 { 36 } else { 40 };
        if reserve_bytes != 0 {
            prefix[30..32].copy_from_slice(&[4, 0]);
            prefix[36..40].copy_from_slice(&[0, 0, 0, reserve_bytes]);
        }
        let file = folder + 8;
        prefix[16..20].copy_from_slice(&(file as u32).to_le_bytes());
        let data_offset = prefix.len() as u64;
        prefix[folder..folder + 4].copy_from_slice(&(data_offset as u32).to_le_bytes());
        prefix[folder + 4..folder + 6].copy_from_slice(&blocks.to_le_bytes());
        prefix[folder + 6..folder + 8].copy_from_slice(&[1, 0]);
        prefix[file..file + 4].copy_from_slice(&(u32::from(blocks) * 32768).to_le_bytes());
        prefix[file + 16..file + 18].copy_from_slice(b"b\0");
        let block_stride = 9 + u64::from(reserve_bytes);
        let physical_bytes = data_offset + u64::from(blocks) * block_stride;
        prefix[8..12].copy_from_slice(&(physical_bytes as u32).to_le_bytes());
        Self {
            prefix,
            position: 0,
            physical_bytes,
            data_offset,
            block_stride,
            maximum_request: 0,
        }
    }
}

impl Read for VirtualCabinet {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.maximum_request = self.maximum_request.max(bytes.len());
        let count = bytes
            .len()
            .min(self.physical_bytes.saturating_sub(self.position) as usize);
        for byte in &mut bytes[..count] {
            *byte = if self.position < self.prefix.len() as u64 {
                self.prefix[self.position as usize]
            } else {
                let offset = (self.position - self.data_offset) % self.block_stride;
                [0, 0, 0, 0, 1, 0, 0, 128]
                    .get(offset as usize)
                    .copied()
                    .unwrap_or(0)
            };
            self.position += 1;
        }
        Ok(count)
    }
}

impl Seek for VirtualCabinet {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let next = match from {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => i128::from(self.position) + i128::from(offset),
            SeekFrom::End(offset) => i128::from(self.physical_bytes) + i128::from(offset),
        };
        self.position = u64::try_from(next).map_err(|_| io::ErrorKind::InvalidInput)?;
        Ok(self.position)
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(2 * 1024 * 1024, 0, 1_000_000)
}

#[test]
fn single_folder_capacity_is_exact_and_walks_all_headers_without_a_full_image() {
    assert_eq!(MAX_DATABASE_BYTES, 0x7fff_8000);
    let mut source = VirtualCabinet::new(65535, 0);
    let layout = read(&mut source, &mut budget(), &|| false).unwrap();
    assert_eq!(layout.database_bytes, 0x7fff_8000);
    assert_eq!(source.prefix.len(), 62);
    assert!(source.maximum_request <= 36);
    assert!(layout.cabinet_bytes < 1024 * 1024);
}

#[test]
fn payload_cabinet_and_overlay_ceiling_refusals_do_not_allocate_large_inputs() {
    let mut payload = VirtualCabinet::new(65535, 0);
    payload.prefix[44..48].copy_from_slice(&0x7fff_8001u32.to_le_bytes());
    assert!(matches!(
        read(&mut payload, &mut budget(), &|| false),
        Err(DeploymentError::Read(ReadError::StructureLimit))
    ));

    let mut cabinet = VirtualCabinet::new(1, 0);
    cabinet.prefix[8..12].copy_from_slice(&0x8000_0000u32.to_le_bytes());
    cabinet.physical_bytes = 0x8000_0000;
    assert!(matches!(
        read(&mut cabinet, &mut budget(), &|| false),
        Err(DeploymentError::Read(ReadError::StructureLimit))
    ));

    let mut overlay = VirtualCabinet::new(1, 0);
    overlay.physical_bytes += 64 * 1024 * 1024 + 1;
    assert!(matches!(
        read(&mut overlay, &mut budget(), &|| false),
        Err(DeploymentError::Read(ReadError::StructureLimit))
    ));
    for source in [payload, cabinet, overlay] {
        assert!(source.prefix.len() <= 66);
        assert!(source.maximum_request <= 36);
    }
}

#[test]
fn per_block_reserve_storage_is_included_in_decoder_metadata_precharge() {
    let plain = read(&mut VirtualCabinet::new(3, 0), &mut budget(), &|| false).unwrap();
    let reserved = read(&mut VirtualCabinet::new(3, 7), &mut budget(), &|| false).unwrap();
    assert_eq!(reserved.database_bytes, plain.database_bytes);
    assert_eq!(reserved.allocation_bytes - plain.allocation_bytes, 21);
    assert_eq!(reserved.cabinet_bytes - plain.cabinet_bytes, 25);
}

#[test]
fn hybrid_large_geometry_selects_factory_without_allocating_a_large_image() {
    let large_payload = VirtualCabinet::new(2049, 0);
    let mut large_container = VirtualCabinet::new(1, 0);
    large_container.physical_bytes = 64 * 1024 * 1024 + 1;
    for mut source in [large_payload, large_container] {
        let called = Cell::new(0);
        let result = hybrid::read(
            &mut source,
            || {
                called.set(called.get() + 1);
                Ok(Cursor::new(Vec::new()))
            },
            &mut ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000),
            &|| false,
        );
        // Virtual compressed bytes are deliberately invalid. This proves strategy
        // selection after geometry, not successful native extraction at scale.
        assert!(matches!(result, Err(DeploymentError::MalformedPackage)));
        assert_eq!(called.get(), 1);
        assert!(source.prefix.len() <= 66);
    }
}
