//! Presentation names and reader carriers are independent host-supplied facts.
use super::Field;
use crate::Localizer;

pub(super) struct TypeLabel {
    pub text: String,
    pub reader_fallback: bool,
}

pub(super) fn display(localizer: &Localizer, field: &Field) -> TypeLabel {
    if let Some(name) = &field.display_type {
        return TypeLabel {
            text: declared(localizer, name),
            reader_fallback: false,
        };
    }
    TypeLabel {
        text: field.scalar_type.as_deref().map_or_else(
            || localizer.text("design-unsupported"),
            |name| carrier(localizer, name),
        ),
        reader_fallback: field.scalar_type.is_some(),
    }
}

fn declared(localizer: &Localizer, name: &str) -> String {
    let key = match name {
        "Short Text" => "design-type-short-text",
        "Long Text" => "design-type-long-text",
        "Number" => "design-type-number",
        "Large Number" => "design-type-large-number",
        "Date/Time" => "design-type-date-time",
        "Date/Time Extended" => "design-type-date-time-extended",
        "Currency" => "design-type-currency",
        "AutoNumber" => "design-type-autonumber",
        "Yes/No" => "design-type-yes-no",
        "OLE Object" => "design-type-ole-object",
        "Hyperlink" => "design-type-hyperlink",
        "Attachment" => "design-type-attachment",
        "Calculated" => "design-type-calculated",
        _ => return name.to_owned(),
    };
    localizer.text(key)
}

/// These labels describe the qualified reader value, never an inferred native
/// declaration such as AutoNumber, Number, OLE Object or a lookup control.
fn carrier(localizer: &Localizer, name: &str) -> String {
    let key = match name {
        "Boolean" => "design-carrier-boolean",
        "Byte" => "design-carrier-byte",
        "Int16" => "design-carrier-int16",
        "Int32" => "design-carrier-int32",
        "Int64" => "design-carrier-int64",
        "Single" => "design-carrier-single",
        "Double" => "design-carrier-double",
        "Currency" => "design-carrier-currency",
        "Decimal" => "design-carrier-decimal",
        "Text" => "design-carrier-text",
        "Binary" => "design-carrier-binary",
        "Guid" => "design-carrier-guid",
        "DateTime" => "design-carrier-date-time",
        "DateTimeExtended" => "design-carrier-date-time-extended",
        _ => return name.to_owned(),
    };
    localizer.text(key)
}

pub(super) fn field_size(localizer: &Localizer, name: &str) -> String {
    let key = match name {
        "Byte" => "design-size-byte",
        "Integer" => "design-size-integer",
        "Long Integer" => "design-size-long-integer",
        "Single" => "design-size-single",
        "Double" => "design-size-double",
        "Decimal" => "design-size-decimal",
        "Replication ID" => "design-size-replication-id",
        "Large Number" => "design-size-large-number",
        "Long Text" => "design-type-long-text",
        "8 bytes" => "design-size-eight-bytes",
        "1 bit" => "design-size-one-bit",
        _ => return name.to_owned(),
    };
    localizer.text(key)
}

#[cfg(test)]
mod tests;
