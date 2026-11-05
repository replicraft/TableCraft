//! Inert field-property observations, separate from reader and write capabilities.
use crate::{Error, controls::check};
use tablecraft_ace::{NativeProperty, PropertyGroupKind, TableProperties};
use tablecraft_core::limits::{Resource, ResourceBudget};

const DECLARATION_FLAGS: u8 = 1;
const NATIVE_BOOLEAN: u8 = 1;
const NATIVE_TEXT: u8 = 10;
const NATIVE_MEMO: u8 = 12;
const KEYS: [(&str, PropertyKind); 7] = [
    ("Required", PropertyKind::Boolean),
    ("AllowZeroLength", PropertyKind::Boolean),
    ("DefaultValue", PropertyKind::Text),
    ("ValidationRule", PropertyKind::Text),
    ("ValidationText", PropertyKind::Text),
    ("Description", PropertyKind::Text),
    ("Caption", PropertyKind::Text),
];

/// A selected native field property. Its key is canonical, never executable text.
pub struct FieldProperty {
    pub name: &'static str,
    pub value: FieldPropertyValue,
}

/// Observed metadata; an absent or unread property is not an Access default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldPropertyValue {
    /// The source did not expose a qualified native property carrier.
    Uninspected,
    /// The completed property map did not contain this field-level key.
    Absent,
    Boolean(bool),
    /// Original text, including empty strings and unevaluated expressions.
    Text(String),
    /// The property exists with an unqualified native carrier or flags.
    Unsupported,
}

#[derive(Clone, Copy)]
enum PropertyKind {
    Boolean,
    Text,
}

pub(super) fn inspect(
    name: &str,
    properties: Option<&TableProperties>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<FieldProperty>, Error> {
    check(cancelled)?;
    work.charge_allocation(KEYS.len() as u64, size_of::<FieldProperty>() as u64)?;
    let mut observations = Vec::new();
    observations.try_reserve_exact(KEYS.len())?;
    for (key, _) in KEYS {
        check(cancelled)?;
        work.charge(Resource::Steps, 1)?;
        observations.push(FieldProperty {
            name: key,
            value: if properties.is_some() {
                FieldPropertyValue::Absent
            } else {
                FieldPropertyValue::Uninspected
            },
        });
    }
    let Some(properties) = properties else {
        return Ok(observations);
    };

    for group in properties.groups() {
        check(cancelled)?;
        work.charge(
            Resource::Steps,
            1 + group.name().len() as u64 + name.len() as u64,
        )?;
        if group.kind() != PropertyGroupKind::Column || !group.name().eq_ignore_ascii_case(name) {
            continue;
        }
        for property in group.properties() {
            for (position, (key, kind)) in KEYS.iter().enumerate() {
                check(cancelled)?;
                work.charge(
                    Resource::Steps,
                    1 + property.name().len() as u64 + key.len() as u64,
                )?;
                if property.name().eq_ignore_ascii_case(key) {
                    observations[position].value = value(property, *kind, work, cancelled)?;
                    break;
                }
            }
        }
        break;
    }
    check(cancelled)?;
    Ok(observations)
}

fn value(
    property: &NativeProperty,
    kind: PropertyKind,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<FieldPropertyValue, Error> {
    check(cancelled)?;
    work.charge(Resource::Steps, 1)?;
    if property.flags() != DECLARATION_FLAGS {
        return Ok(FieldPropertyValue::Unsupported);
    }
    match kind {
        PropertyKind::Boolean if property.native_type() == NATIVE_BOOLEAN => {
            Ok(FieldPropertyValue::Boolean(property.boolean()?))
        }
        PropertyKind::Text if matches!(property.native_type(), NATIVE_TEXT | NATIVE_MEMO) => {
            let text = property.text()?;
            work.charge(Resource::Steps, text.len() as u64)?;
            work.charge(Resource::Bytes, text.len() as u64)?;
            let mut owned = String::new();
            owned.try_reserve_exact(text.len())?;
            owned.push_str(text);
            check(cancelled)?;
            Ok(FieldPropertyValue::Text(owned))
        }
        _ => Ok(FieldPropertyValue::Unsupported),
    }
}

#[cfg(test)]
mod tests;
