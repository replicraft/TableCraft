//! Displayed-record TSV; no native record, NULL or attachment transport.
use crate::{Cell, Data};
use std::io::{self, Write};

const MAX_CLIPBOARD_BYTES: usize = 1_048_576;
const WRITER_BUFFER_BYTES: usize = 8_192;

#[derive(Clone, Copy)]
pub(super) enum CopyError {
    IncompleteRow,
    Limit,
    Allocation,
    Encoding,
}

impl CopyError {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::IncompleteRow => "The selected record does not contain every displayed field.",
            Self::Limit => "The displayed record exceeds the 1 MiB clipboard limit.",
            Self::Allocation => "The clipboard buffer could not be allocated.",
            Self::Encoding => "The displayed record could not be encoded for the clipboard.",
        }
    }
}

#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    failure: Option<CopyError>,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let Some(length) = self.bytes.len().checked_add(bytes.len()) else {
            self.failure = Some(CopyError::Limit);
            return Err(io::Error::other("clipboard limit"));
        };
        if length > MAX_CLIPBOARD_BYTES {
            self.failure = Some(CopyError::Limit);
            return Err(io::Error::other("clipboard limit"));
        }
        if self.bytes.try_reserve_exact(bytes.len()).is_err() {
            self.failure = Some(CopyError::Allocation);
            return Err(io::Error::other("clipboard allocation"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn row_text(data: &Data, row: usize) -> Result<String, CopyError> {
    let values = data.rows.get(row).ok_or(CopyError::IncompleteRow)?;
    if values.len() != data.columns.len() || values.is_empty() {
        return Err(CopyError::IncompleteRow);
    }
    encode(values)
}

fn encode(values: &[Cell]) -> Result<String, CopyError> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b'\t')
        .terminator(csv::Terminator::Any(b'\n'))
        .buffer_capacity(WRITER_BUFFER_BYTES)
        .from_writer(Output::default());
    if writer
        .write_record(values.iter().map(|cell| cell.text.as_str()))
        .is_err()
        || writer.flush().is_err()
    {
        return Err(writer.get_ref().failure.unwrap_or(CopyError::Encoding));
    }
    let output = writer.into_inner().map_err(|_| CopyError::Encoding)?;
    String::from_utf8(output.bytes).map_err(|_| CopyError::Encoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_record_uses_csv_tsv_quoting_and_preserves_empty_null_and_summary_text() {
        let values: Vec<Cell> = [
            "",
            "NULL",
            "2 attachments",
            "tab\there",
            "line\r\nnext",
            "a\"b",
        ]
        .into_iter()
        .map(Cell::scalar)
        .collect();
        assert_eq!(
            row_text(
                &Data {
                    source_revision: None,
                    table: "Result".into(),
                    offset: 0,
                    columns: (0..values.len())
                        .map(|index| crate::Column {
                            name: format!("Raw{index}"),
                            caption: Some(format!("Caption{index}")),
                        })
                        .collect(),
                    rows: vec![values],
                    has_more: false,
                },
                0
            )
            .unwrap_or_else(|error| panic!("{}", error.message())),
            "\tNULL\t2 attachments\t\"tab\there\"\t\"line\r\nnext\"\t\"a\"\"b\"\n"
        );
    }

    #[test]
    fn quote_expansion_and_record_terminator_count_toward_the_clipboard_bound() {
        assert_eq!(
            encode(&[Cell::scalar("x".repeat(MAX_CLIPBOARD_BYTES - 1))])
                .unwrap_or_else(|error| panic!("{}", error.message()))
                .len(),
            MAX_CLIPBOARD_BYTES
        );
        assert!(matches!(
            encode(&[Cell::scalar("x".repeat(MAX_CLIPBOARD_BYTES))]),
            Err(CopyError::Limit)
        ));
        assert!(matches!(
            encode(&[Cell::scalar("\"".repeat(MAX_CLIPBOARD_BYTES / 2))]),
            Err(CopyError::Limit)
        ));
    }
}
