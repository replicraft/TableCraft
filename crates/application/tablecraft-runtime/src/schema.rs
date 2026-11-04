//! Owned native field descriptors for an explicit application inspector request.
use super::{
    Error, Response,
    controls::{SCOPE, budget, check},
    document::Document,
};
use tablecraft_access::NativeBrowser;
use tablecraft_core::limits::{Resource, ResourceBudget};

const DESCRIPTOR_BYTES: u64 = 8 * 1024 * 1024;
const DESCRIPTOR_STEPS: u64 = 1_000_000;
const CARRIER_NAME_BYTES: u64 = 32;

/// Original native declarations and separately qualified scalar-reader facts.
/// These fields do not authorize editing or establish complete table readability.
pub struct Field {
    /// Original validated name, preserving its stored spelling.
    pub name: String,
    /// Logical native column number, distinct from display/vector position.
    pub number: usize,
    /// Original native format type code, including unsupported declarations.
    pub native_type: u8,
    /// Declared native storage bytes; not a character count or retained payload size.
    pub storage_bytes: usize,
    /// Whether this field uses the native fixed storage region.
    pub fixed: bool,
    /// Original native flags; not interpreted as Required/default permissions.
    pub flags: u8,
    /// Original extended flags, retained even when scalar reads are unsupported.
    pub extended_flags: u8,
    /// Qualified scalar-reader carrier name; None preserves an unavailable field.
    pub scalar_type: Option<String>,
    /// Reader representation policy, NOT declared Required or a constraint check.
    /// Required/default/validation property maps are not loaded by describe().
    pub reader_nullable: Option<bool>,
    /// Original Decimal precision/scale; not inferred from materialized values.
    pub decimal_parameters: Option<(u8, u8)>,
    /// Native complex-type ID, without opening child tables or decoding collections.
    pub complex_type_id: Option<u32>,
}

impl Document {
    pub(super) fn describe(
        &mut self,
        table: &str,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Response, Error> {
        let mut browser = NativeBrowser::new(&mut self.source, SCOPE, budget(), cancelled);
        let description = browser.describe(table)?;
        let schema = description.schema();
        let mut work = ResourceBudget::new(DESCRIPTOR_BYTES, 0, DESCRIPTOR_STEPS);
        check(cancelled)?;
        work.charge_allocation(schema.columns().len() as u64, size_of::<Field>() as u64)?;
        work.charge(Resource::Bytes, schema.name().len() as u64)?;
        let table = schema.name().to_owned();
        let mut fields = Vec::new();
        fields.try_reserve_exact(schema.columns().len())?;

        for native in schema.columns() {
            check(cancelled)?;
            work.charge(Resource::Steps, 1)?;
            work.charge(Resource::Bytes, native.name().len() as u64)?;
            work.charge(Resource::Steps, native.name().len() as u64)?;
            let name = native.name().to_owned();
            let (scalar_type, reader_nullable) = if native.scalar_type().is_some() {
                // The existing shared-column API owns exact nullable/carrier
                // classification. Pay its name copy and our short carrier label.
                work.charge(Resource::Bytes, native.name().len() as u64)?;
                work.charge(Resource::Bytes, CARRIER_NAME_BYTES)?;
                work.charge(Resource::Steps, native.name().len() as u64)?;
                work.charge(Resource::Steps, CARRIER_NAME_BYTES)?;
                let column = native.column()?;
                (
                    Some(format!("{:?}", column.data_type())),
                    Some(column.nullable()),
                )
            } else {
                (None, None)
            };
            fields.push(Field {
                name,
                number: native.number(),
                native_type: native.native_type(),
                storage_bytes: native.storage_length(),
                fixed: native.fixed(),
                flags: native.flags(),
                extended_flags: native.extended_flags(),
                scalar_type,
                reader_nullable,
                decimal_parameters: native.decimal_parameters(),
                complex_type_id: native.complex_type_id(),
            });
        }
        check(cancelled)?;
        Ok(Response::Details {
            table,
            declared_rows: schema.declared_rows(),
            fields,
        })
    }
}
