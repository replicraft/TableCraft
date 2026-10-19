use std::io::{self, Cursor, Read, Seek, SeekFrom};

pub struct PageReads {
    cursor: Cursor<Vec<u8>>,
    starts: Vec<u64>,
}

impl PageReads {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
            starts: Vec::new(),
        }
    }

    pub fn count(&self, page: u64) -> usize {
        self.starts.iter().filter(|&&at| at == page * 4096).count()
    }
}

impl Read for PageReads {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.starts.push(self.cursor.position());
        self.cursor.read(bytes)
    }
}

impl Seek for PageReads {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(position)
    }
}
