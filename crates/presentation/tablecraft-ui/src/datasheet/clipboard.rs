//! Displayed-record TSV; no native record, NULL or attachment transport.
use crate::{Cell, Data, Localizer};
use std::io::{self, Write};

const WRITER_BUFFER_BYTES: usize = 8_192;

#[derive(Clone, Copy)]
pub(super) enum CopyError {
    IncompleteRow,
    Limit,
    Allocation,
    Encoding,
}

impl CopyError {
    pub(super) fn message(self, localizer: &Localizer) -> String {
        let key = match self {
            Self::IncompleteRow => "datasheet-copy-incomplete",
            Self::Limit => "datasheet-copy-limit",
            Self::Allocation => "datasheet-copy-allocation",
            Self::Encoding => "datasheet-copy-encoding",
        };
        localizer.text(key)
    }
}

#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    failure: Option<CopyError>,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().checked_add(bytes.len()).is_none() {
            self.failure = Some(CopyError::Limit);
            return Err(io::Error::other("clipboard size overflow"));
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
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
                    row_ordinals: None,
                    source_revision: None,
                    table: "Result".into(),
                    offset: 0,
                    columns: (0..values.len())
                        .map(|index| crate::Column {
                            name: format!("Raw{index}"),
                            caption: Some(format!("Caption{index}")),
                            native_number: None,
                        })
                        .collect(),
                    rows: vec![values],
                    has_more: false,
                },
                0
            )
            .unwrap_or_else(|error| panic!("{}", error.message(&Localizer::default()))),
            "\tNULL\t2 attachments\t\"tab\there\"\t\"line\r\nnext\"\t\"a\"\"b\"\n"
        );
    }

    #[test]
    fn large_quoted_unicode_record_copies_every_value_without_a_byte_quota() {
        let text = "日本語🦀\t\"value\"\r\n".repeat(90_000);
        assert!(text.len() > 1024 * 1024);
        let values = [
            Cell::scalar(text.clone()),
            Cell::scalar(""),
            Cell::scalar("NULL"),
        ];
        let encoded = encode(&values)
            .unwrap_or_else(|error| panic!("{}", error.message(&Localizer::default())));
        assert_eq!(
            encoded,
            format!("\"{}\"\t\tNULL\n", text.replace('"', "\"\""))
        );
        assert_eq!(values[0].text, text);
    }
}
