//! Inert source-bound table and child coverage; no execution or save admission.
use super::{Error, extra, line};
use crate::{native, output::TableOutput};
use std::io::{Read, Seek};
use tablecraft_ace::{InspectError, NativeImage, PayloadPresence, TableExtraCoverage};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::ObjectKind;

pub(super) fn run<R: Read + Seek>(
    source: &mut R,
    name: &str,
    output: &mut TableOutput,
    mut budget: ResourceBudget,
) -> Result<(), Error> {
    let image = NativeImage::capture(source, native::CATALOG_SCOPE, &mut budget, &|| false)
        .map_err(Error::Read)?;
    for descriptor in image.catalog().catalog().objects() {
        let work = descriptor
            .name()
            .as_str()
            .len()
            .checked_add(name.len())
            .and_then(|length| length.checked_add(1))
            .ok_or(Error::Read(InspectError::StructureLimit))?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)
            .map_err(Error::Read)?;
    }
    let object = image
        .catalog()
        .catalog()
        .lookup(name, Some(ObjectKind::Table), None)
        .map_err(InspectError::from)
        .map_err(Error::Read)?
        .ok_or(Error::Read(InspectError::UnknownTable))?;
    let coverage = image
        .inspect_table_macro_coverage(object.id(), &mut budget, &|| false)
        .map_err(Error::Read)?;
    let origin = coverage.table_origin();
    let table_extra = coverage.table_extra();

    line(output, &["Table", object.name().as_str()])?;
    line(output, &["Native ID", &origin.native_id().to_string()])?;
    line(
        output,
        &["Native parent ID", &origin.parent_id().to_string()],
    )?;
    line(output, &["Catalog page", &origin.page().to_string()])?;
    line(output, &["Catalog slot", &origin.slot().to_string()])?;
    line(
        output,
        &["Coverage scope", "Table and direct catalog children"],
    )?;
    line(
        output,
        &["Direct children", &coverage.children().len().to_string()],
    )?;
    line(output, &["Required macro coverage", "Not certified"])?;
    line(output, &["Data-macro absence", "Not certified"])?;
    line(output, &["Save permission", "Not granted"])?;

    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &["Owner", "Native ID", "Carrier", "Presence", "Bytes"],
    )?;
    for (carrier, payload) in [("Lv", coverage.body()), ("LvModule", coverage.module())] {
        payload_row(
            output,
            "Table",
            origin.native_id(),
            carrier,
            payload.presence(),
            payload.as_bytes(),
        )?;
    }
    payload_row(
        output,
        "Table",
        origin.native_id(),
        "LvExtra",
        table_extra.presence(),
        table_extra.raw(),
    )?;
    for child in coverage.children() {
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)
            .map_err(Error::Read)?;
        let payloads = child.payloads();
        for (carrier, payload) in [
            ("Lv", payloads.body()),
            ("LvModule", payloads.module()),
            ("LvExtra", payloads.extra()),
        ] {
            payload_row(
                output,
                "Direct child",
                child.origin().native_id(),
                carrier,
                payload.presence(),
                payload.as_bytes(),
            )?;
        }
    }

    output.table_section().map_err(Error::Output)?;
    line(
        output,
        &[
            "Child",
            "Native ID",
            "Native parent ID",
            "Native type",
            "Catalog page",
            "Catalog slot",
        ],
    )?;
    for child in coverage.children() {
        let origin = child.origin();
        line(
            output,
            &[
                child.descriptor().name().as_str(),
                &origin.native_id().to_string(),
                &origin.parent_id().to_string(),
                &origin.type_code().to_string(),
                &origin.page().to_string(),
                &origin.slot().to_string(),
            ],
        )?;
    }

    output.table_section().map_err(Error::Output)?;
    let framing = match table_extra.coverage() {
        TableExtraCoverage::NotPresent => "No payload inspected",
        TableExtraCoverage::Empty => "Present empty payload",
        TableExtraCoverage::WideMr2 => "Observed wide-MR2 framing",
        TableExtraCoverage::Opaque => "Opaque content retained",
    };
    line(output, &["Table Extra framing", framing])?;
    line(
        output,
        &["Child payload interpretation", "Opaque; not interpreted"],
    )?;
    line(
        output,
        &["Indirect/native storage coverage", "Not certified"],
    )?;
    extra::inventory::write(table_extra, output, &mut budget)
}

fn payload_row(
    output: &mut TableOutput,
    owner: &str,
    native_id: i32,
    carrier: &str,
    presence: PayloadPresence,
    bytes: Option<&[u8]>,
) -> Result<(), Error> {
    let presence = match presence {
        PayloadPresence::Uninspected => "Column missing",
        PayloadPresence::Absent => "NULL",
        PayloadPresence::Present => "Present",
    };
    let length = bytes
        .map(|bytes| bytes.len().to_string())
        .unwrap_or_default();
    line(
        output,
        &[owner, &native_id.to_string(), carrier, presence, &length],
    )
}
