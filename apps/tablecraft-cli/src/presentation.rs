//! Terminal presentation of already escaped, bounded scalar tables.
mod tsv;

use crate::output::{MAX_OUTPUT_BYTES, OutputError, TableOutput};
use clap::ValueEnum;
use comfy_table::{
    ContentArrangement, Table, TableStyle,
    presets::{UTF8_FULL, UTF8_FULL_CONDENSED},
};
use std::io::{self, IsTerminal};

const TABLE_WIDTH: u16 = 120;

#[derive(Clone, Copy, Default, ValueEnum)]
pub(crate) enum Format {
    /// Bordered tables in terminals; TSV when stdout is redirected.
    #[default]
    Auto,
    /// Bordered Unicode tables, including when redirected.
    Table,
    /// CSV-quoted tab-separated escaped display cells for scripts.
    Tsv,
}
impl Format {
    fn bordered(self) -> bool {
        matches!(self, Self::Table) || matches!(self, Self::Auto) && io::stdout().is_terminal()
    }
    pub(crate) fn render(self, text: String) -> Result<String, OutputError> {
        self.render_with_limit(text, MAX_OUTPUT_BYTES)
    }

    pub(crate) fn render_with_limit(
        self,
        text: String,
        max_bytes: usize,
    ) -> Result<String, OutputError> {
        self.render_style_with_limit(text, UTF8_FULL, max_bytes)
    }

    fn render_style(self, text: String, style: TableStyle) -> Result<String, OutputError> {
        self.render_style_with_limit(text, style, MAX_OUTPUT_BYTES)
    }

    fn render_style_with_limit(
        self,
        text: String,
        style: TableStyle,
        max_bytes: usize,
    ) -> Result<String, OutputError> {
        if !self.bordered() {
            return tsv::encode_with_limit(&text, max_bytes);
        }
        if text.len() > max_bytes {
            return Err(OutputError::Limit);
        }

        let mut lines = text.lines();
        let Some(header) = lines.next() else {
            return Ok(text);
        };
        let mut table = Table::new();
        table
            .load_style(style)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_width(TABLE_WIDTH)
            .set_header(header.split('\t'));
        for line in lines {
            table.add_row(line.split('\t'));
        }
        // Enforce the selected final output ceiling and publish only complete output.
        // comfy-table owns its internal allocations; these are not fallible APIs.
        let mut output = TableOutput::with_limit(max_bytes);
        for line in table.lines() {
            output.display(&line)?;
            output.newline()?;
        }
        Ok(output.finish())
    }

    pub(crate) fn render_report(
        self,
        text: String,
        sections: &[usize],
    ) -> Result<String, OutputError> {
        if !self.bordered() {
            return tsv::encode(&text);
        }
        let mut output = TableOutput::new();
        let summary_end = sections.first().copied().unwrap_or(text.len());
        if summary_end != 0 {
            let summary = self.render_style(
                "Property\tValue\n".to_owned() + &text[..summary_end],
                UTF8_FULL_CONDENSED,
            )?;
            output.display(&summary)?;
        }
        for (i, start) in sections.iter().copied().enumerate() {
            if start != 0 {
                output.newline()?;
            }
            let end = sections.get(i + 1).copied().unwrap_or(text.len());
            output
                .display(&self.render_style(text[start..end].to_owned(), UTF8_FULL_CONDENSED)?)?;
        }
        Ok(output.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_borders_remain_subject_to_the_output_ceiling() {
        let text = "H".repeat(512) + "\n" + &"a\n".repeat(4096);
        assert!(matches!(
            Format::Table.render_with_limit(text, 1_048_576),
            Err(OutputError::Limit)
        ));
    }

    #[test]
    fn explicit_output_budget_also_bounds_wrapped_headers() {
        let text = "H".repeat(16_384) + "\n" + &"a\n".repeat(4096);
        assert!(matches!(
            Format::Table.render_with_limit(text, 1_048_576),
            Err(OutputError::Limit)
        ));
    }

    #[test]
    fn default_tsv_publication_has_no_former_sixteen_mib_quota() {
        let payload = "x".repeat(17 * 1024 * 1024);
        let text = format!("Value\n{payload}\n");
        let rendered = Format::Tsv.render_report(text, &[0]).unwrap();
        assert_eq!(rendered.len(), payload.len() + 7);
        assert!(rendered.starts_with("Value\nx"));
        assert!(rendered.ends_with("x\n"));
    }
}
