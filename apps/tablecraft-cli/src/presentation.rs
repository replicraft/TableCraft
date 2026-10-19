//! Terminal presentation of already escaped, bounded scalar tables.
mod tsv;

use crate::output::{OutputError, TableOutput};
use clap::ValueEnum;
use comfy_table::{
    ContentArrangement, Table, TableStyle,
    presets::{UTF8_FULL, UTF8_FULL_CONDENSED},
};
use std::io::{self, IsTerminal};

const TABLE_WIDTH: u16 = 120;
// Bound eager dependency layout/border expansion before entering its formatter.
const MAX_LAYOUT_BYTES: u128 = 32 * 1024 * 1024;
const CELL_PADDING: u128 = 2;
const BORDER_UTF8_BYTES: u128 = 3;

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
        self.render_style(text, UTF8_FULL)
    }

    fn render_style(self, text: String, style: TableStyle) -> Result<String, OutputError> {
        if !self.bordered() {
            return tsv::encode(&text);
        }
        check_layout(&text)?;
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
        // Keep the same final output ceiling and publish only complete output.
        // comfy-table owns its internal allocations; these are not fallible APIs.
        let mut output = TableOutput::new();
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

fn check_layout(text: &str) -> Result<(), OutputError> {
    // Escaped input has no embedded cell newlines/control sequences. Byte length
    // bounds display width and wrapping height, even for wide Unicode scalars.
    let mut widths = Vec::<u128>::new();
    let mut lines = 2u128; // Outer borders.
    for row in text.lines() {
        let mut height = 1;
        for (column, cell) in row.split('\t').enumerate() {
            if column == widths.len() {
                widths.push(0);
            }
            let bytes = cell.len() as u128;
            widths[column] = widths[column].max(bytes + CELL_PADDING);
            height = height.max(bytes);
        }
        lines += height + 1; // Wrapped content plus separator.
    }
    let width = widths.iter().sum::<u128>() + widths.len() as u128 + 1;
    if width * lines * BORDER_UTF8_BYTES > MAX_LAYOUT_BYTES {
        return Err(OutputError::TableLayout);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expanded_borders_remain_subject_to_the_output_ceiling() {
        let text = "H".repeat(512) + "\n" + &"a\n".repeat(4096);
        assert!(matches!(
            Format::Table.render(text),
            Err(OutputError::Limit)
        ));
    }

    #[test]
    fn large_padding_expansion_is_refused_before_dependency_layout() {
        let text = "H".repeat(16_384) + "\n" + &"a\n".repeat(4096);
        assert!(matches!(
            Format::Table.render(text),
            Err(OutputError::TableLayout)
        ));
    }

    #[test]
    fn report_output_limit_applies_across_all_sections() {
        let section = "H".repeat(512) + "\n" + &"a\n".repeat(5000);
        let second = section.len();
        assert!(matches!(
            Format::Table.render_report(section.repeat(2), &[0, second]),
            Err(OutputError::Limit)
        ));
    }
}
