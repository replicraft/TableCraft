//! TSV transport for the CLI's escaped display cells, not raw stored values.
use crate::output::{MAX_OUTPUT_BYTES, OutputError};
use std::io::{self, Write};

#[derive(Default)]
struct Buffer {
    bytes: Vec<u8>,
    error: Option<OutputError>,
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.error.is_some() {
            return Err(io::Error::other("output buffer failed"));
        }
        if self
            .bytes
            .len()
            .checked_add(bytes.len())
            .is_none_or(|len| len > MAX_OUTPUT_BYTES)
        {
            self.error = Some(OutputError::Limit);
            return Err(io::Error::other("output limit"));
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.error = Some(OutputError::Allocation);
            return Err(io::Error::other("output allocation"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn encode(text: &str) -> Result<String, OutputError> {
    let mut buffer = Buffer::default();
    let result = (|| -> Result<(), csv::Error> {
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            // Inspection reports contain summary pairs and wider detail sections.
            .flexible(true)
            .from_writer(&mut buffer);
        for line in text.lines() {
            writer.write_record(line.split('\t'))?;
        }
        writer.flush()?;
        Ok(())
    })();
    if result.is_err() {
        return Err(buffer.error.unwrap_or(OutputError::Formatting));
    }
    String::from_utf8(buffer.bytes).map_err(|_| OutputError::Formatting)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_display_cells_round_trip_through_standard_tsv() {
        let text = "Heading\tquote\\\"\nNULL\t\"NULL\"\n\"\"\t\"雪\\tline\\n\\\"\"\n";
        let encoded = encode(text).unwrap();
        assert_ne!(encoded, text);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .has_headers(false)
            .from_reader(encoded.as_bytes());
        let decoded: Vec<Vec<String>> = reader
            .records()
            .map(|r| r.unwrap().iter().map(str::to_owned).collect())
            .collect();
        let expected: Vec<Vec<String>> = text
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect())
            .collect();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn report_sections_allow_different_record_widths() {
        assert_eq!(
            encode("Format\tAce2007\nName\tKind\tRows\nItems\tUser\t3\n").unwrap(),
            "Format\tAce2007\nName\tKind\tRows\nItems\tUser\t3\n"
        );
    }

    #[test]
    fn csv_quote_expansion_obeys_final_output_limit() {
        assert!(matches!(
            encode(&"\"".repeat(MAX_OUTPUT_BYTES / 2)),
            Err(OutputError::Limit)
        ));
    }
}
