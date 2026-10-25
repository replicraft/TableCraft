//! Candidate lookup for the existing whole-predicate integral equality proof.
mod key;
mod storage;

use super::{Runtime, equality::BorrowedEquality};
use crate::{QueryError, execution::cell::CellValue};
use storage::Directory;
use tablecraft_sql::select::JoinKind;

impl Runtime<'_, '_> {
    pub(super) fn directory_join(
        &mut self,
        equality: &BorrowedEquality,
        left: &[Vec<CellValue>],
        right: &[Vec<CellValue>],
        kind: Option<JoinKind>,
        widths: (usize, usize),
        matched: &mut [bool],
    ) -> Result<Option<Vec<Vec<CellValue>>>, QueryError> {
        // Both sources have already completed their ordinary validation. Empty
        // sides retain the original path, including keys never compared there.
        if left.is_empty() || right.is_empty() {
            return Ok(None);
        }
        if !self.directory_keys(equality, left, right)? {
            // A successful descriptor is not authority to reinterpret a cell.
            // Fall back for the entire operation before any output is produced.
            return Ok(None);
        }
        let directory = self.join_directory(equality, right)?;
        let mut result = Vec::new();

        for row in left {
            self.tick()?;
            let mut hit = false;
            if let Some(fingerprint) = key::fingerprint(equality.carrier(), equality.left_key(row))
            {
                let mut position = directory.first(fingerprint);
                while let Some(index) = position {
                    self.tick()?;
                    let entry = directory.entry(index);
                    position = entry.next;
                    if entry.fingerprint != fingerprint {
                        continue;
                    }
                    let candidate = &right[entry.row];
                    if self.borrowed_join_matches(equality, row, candidate)? {
                        hit = true;
                        matched[entry.row] = true;
                        let combined = self.join_row(row, candidate)?;
                        self.push(&mut result, combined)?;
                    }
                }
            }
            if !hit && kind == Some(JoinKind::Left) {
                let combined = self.join_missing(row, widths.1, false)?;
                self.push(&mut result, combined)?;
            }
        }
        if kind == Some(JoinKind::Right) {
            for (index, row) in right.iter().enumerate() {
                self.tick()?;
                if !matched[index] {
                    let combined = self.join_missing(row, widths.0, true)?;
                    self.push(&mut result, combined)?;
                }
            }
        }
        Ok(Some(result))
    }

    fn directory_keys(
        &mut self,
        equality: &BorrowedEquality,
        left: &[Vec<CellValue>],
        right: &[Vec<CellValue>],
    ) -> Result<bool, QueryError> {
        for row in left {
            self.tick()?;
            if !key::valid(equality.carrier(), equality.left_key(row)) {
                return Ok(false);
            }
        }
        for row in right {
            self.tick()?;
            if !key::valid(equality.carrier(), equality.right_key(row)) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn join_directory(
        &mut self,
        equality: &BorrowedEquality,
        rows: &[Vec<CellValue>],
    ) -> Result<Directory, QueryError> {
        let mut directory = Directory::default();
        for (index, row) in rows.iter().enumerate() {
            self.tick()?;
            // Actual cells were checked in full before construction. Only NULL
            // lacks a fingerprint; SQL predicate equality never matches it.
            if let Some(fingerprint) = key::fingerprint(equality.carrier(), equality.right_key(row))
            {
                directory.insert(fingerprint, index, self.budget, self.options.cancelled)?;
            }
        }
        Ok(directory)
    }
}
