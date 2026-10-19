//! Named, separately rendered table pages under one cumulative browser budget.
use super::{Args, Error, read_page};
use crate::{output::TableOutput, presentation::Format};
use std::io::{Read, Seek};
use tablecraft_access::{NativeBrowser, NativeTableStorage, TablePageRequest};

pub(super) fn run<R: Read + Seek>(
    browser: &mut NativeBrowser<'_, R>,
    args: &Args,
    request: TablePageRequest,
    format: Format,
) -> Result<String, Error> {
    let inventory = browser.tables().map_err(Error::Read)?;
    let mut output = TableOutput::new();
    for (index, entry) in inventory
        .entries()
        .filter(|entry| !entry.is_system())
        .enumerate()
    {
        let name = entry.object().name().as_str();
        let mut section = TableOutput::new();
        section.header(Some("Table"), 1).map_err(Error::Output)?;
        section.separator().map_err(Error::Output)?;
        section.header(Some(name), 2).map_err(Error::Output)?;
        section.newline().map_err(Error::Output)?;
        section.table_section().map_err(Error::Output)?;
        let text = match entry.storage() {
            Some(NativeTableStorage::Local) => read_page(browser, name, request, args)?,
            _ => "Status\nExternal or unknown source not opened\n".to_owned(),
        };
        section.display(&text).map_err(Error::Output)?;
        let (text, sections) = section.finish_sections();
        let rendered = format
            .render_report(text, &sections)
            .map_err(Error::Output)?;
        if index != 0 {
            output.newline().map_err(Error::Output)?;
        }
        output.display(&rendered).map_err(Error::Output)?;
    }
    Ok(output.finish())
}
