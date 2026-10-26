use super::{composite_fixture, support};
use std::io::{self, Read, Seek, SeekFrom};
use tablecraft_ace::NativeReadSource;

const PAGE_BYTES: usize = 4096;
const USAGE_PAGE: usize = 11;
const DATA_PAGE: usize = 12;
const BITMAP_PAGE: usize = 17;
const SECOND_BITMAP: usize = 18;
const PREFIX_PAGES: usize = 19;
const FIRST_EMPTY: usize = 100;
const EMPTY_PAGES: usize = 4999;
const SOURCE_PAGES: usize = 70_000;
const OWNER_OFFSET: usize = 4;
const DEFINITION_PAGE: usize = 10;
const DECLARED_ROWS_OFFSET: usize = 16;

pub struct Sparse {
    prefix: Vec<u8>,
    position: u64,
    pub data_reads: usize,
    pub bytes_read: usize,
}

fn data(rows: &[(i32, i32)]) -> Vec<u8> {
    let rows = rows
        .iter()
        .map(|(id, score)| {
            let mut row = 2_u16.to_le_bytes().to_vec();
            row.extend(id.to_le_bytes());
            row.extend(score.to_le_bytes());
            row.push(3);
            (0, row)
        })
        .collect::<Vec<_>>();
    support::data_page(10, &rows)
}

impl Sparse {
    pub fn new(code: u8, indirect: bool) -> Self {
        let mut prefix = composite_fixture::database(
            code,
            &[4, 4],
            &[(0, false)],
            &[
                vec![Some(3), Some(30)],
                vec![Some(1), Some(10)],
                vec![Some(2), Some(20)],
            ],
            0,
        );
        prefix.resize(PREFIX_PAGES * PAGE_BYTES, 0);
        let mut bits = vec![0; (FIRST_EMPTY + EMPTY_PAGES).div_ceil(8)];
        bits[DATA_PAGE / 8] |= 1 << (DATA_PAGE % 8);
        for number in FIRST_EMPTY..FIRST_EMPTY + EMPTY_PAGES {
            bits[number / 8] |= 1 << (number % 8);
        }
        let map = if indirect {
            prefix[BITMAP_PAGE * PAGE_BYTES..BITMAP_PAGE * PAGE_BYTES + 4]
                .copy_from_slice(&[5, 1, 0, 0]);
            prefix[BITMAP_PAGE * PAGE_BYTES + 4..BITMAP_PAGE * PAGE_BYTES + 4 + bits.len()]
                .copy_from_slice(&bits);
            [vec![1], (BITMAP_PAGE as u32).to_le_bytes().to_vec()].concat()
        } else {
            [vec![0; 5], bits].concat()
        };
        prefix[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
        Self {
            prefix,
            position: 0,
            data_reads: 0,
            bytes_read: 0,
        }
    }

    pub fn bad_second_bitmap(&mut self) {
        let map = [
            vec![1],
            (BITMAP_PAGE as u32).to_le_bytes().to_vec(),
            (SECOND_BITMAP as u32).to_le_bytes().to_vec(),
        ]
        .concat();
        self.prefix[USAGE_PAGE * PAGE_BYTES..(USAGE_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
        self.prefix[SECOND_BITMAP * PAGE_BYTES] = 255;
    }

    pub fn remove_selected_membership(&mut self) {
        self.prefix[BITMAP_PAGE * PAGE_BYTES + 4 + DATA_PAGE / 8] &= !(1 << (DATA_PAGE % 8));
    }

    pub fn declared_rows(&mut self, count: u32) {
        support::put32(
            &mut self.prefix,
            DEFINITION_PAGE * PAGE_BYTES + DECLARED_ROWS_OFFSET,
            count,
        );
    }

    pub fn wrong_owner(&mut self) {
        support::put32(&mut self.prefix, DATA_PAGE * PAGE_BYTES + OWNER_OFFSET, 99);
    }

    pub fn wrong_key(&mut self) {
        self.prefix[DATA_PAGE * PAGE_BYTES..(DATA_PAGE + 1) * PAGE_BYTES]
            .copy_from_slice(&data(&[(3, 30), (99, 10), (2, 20)]));
    }
}

impl NativeReadSource for Sparse {}

impl Read for Sparse {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.len() > PAGE_BYTES {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let page = self.position as usize / PAGE_BYTES;
        let offset = self.position as usize % PAGE_BYTES;
        let available = if page < PREFIX_PAGES {
            if page == DATA_PAGE {
                self.data_reads += 1;
            }
            &self.prefix[self.position as usize..]
        } else if (FIRST_EMPTY..FIRST_EMPTY + EMPTY_PAGES).contains(&page) {
            panic!("Selected read touched an unrelated empty owned data page");
        } else {
            return Err(io::ErrorKind::InvalidInput.into());
        };
        let count = bytes.len().min(available.len()).min(PAGE_BYTES - offset);
        bytes[..count].copy_from_slice(&available[..count]);
        self.position += count as u64;
        self.bytes_read += count;
        Ok(count)
    }
}

impl Seek for Sparse {
    fn seek(&mut self, requested: SeekFrom) -> io::Result<u64> {
        self.position = match requested {
            SeekFrom::Start(value) => value,
            SeekFrom::Current(delta) => self
                .position
                .checked_add_signed(delta)
                .ok_or(io::ErrorKind::InvalidInput)?,
            SeekFrom::End(delta) => (SOURCE_PAGES as u64 * PAGE_BYTES as u64)
                .checked_add_signed(delta)
                .ok_or(io::ErrorKind::InvalidInput)?,
        };
        Ok(self.position)
    }
}
