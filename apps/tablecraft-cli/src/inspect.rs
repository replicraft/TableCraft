use std::{fs::File, path::Path};
use tablecraft_ace::{Header, InspectError, inspect_header};
use tablecraft_core::limits::ResourceBudget;

pub(crate) fn run(path: &Path) -> Result<String, InspectError> {
    let mut source = File::open(path).map_err(InspectError::from)?;
    let header = inspect_header(&mut source, &mut ResourceBudget::new(66, 0, 10), &|| false)?;
    Ok(format_header(header))
}
fn format_header(header: Header) -> String {
    format!(
        "Format\t{:?}\nSubversion\t{}\nPage bytes\t{}\nPages\t{}\nEncoded pages\t{}\n",
        header.format(),
        header.subversion(),
        header.page_size(),
        header.page_count(),
        header.encoded_pages()
    )
}
