use super::*;
use std::io::{Cursor, Write};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_formats::{
    ReadError,
    template::{TemplateError, TemplatePackage, TemplateTable},
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

pub(super) fn package(entries: &[(&str, &str)], method: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in entries {
        writer
            .start_file(
                *name,
                SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

pub(super) fn open(bytes: &[u8]) -> Result<TemplatePackage, TemplateError> {
    TemplatePackage::open(
        &mut Cursor::new(bytes),
        &mut ResourceBudget::new(64 * 1024 * 1024, 0, 1_000_000),
        &|| false,
    )
}

mod data;
mod list;
