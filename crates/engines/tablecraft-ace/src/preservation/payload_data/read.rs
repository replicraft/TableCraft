use super::super::payloads::PAYLOAD_FIELDS;
use super::super::{NativeImage, PayloadPresence};
use super::{ObjectPayloadData, OpaquePayload};
use crate::{
    InspectError,
    format::{data_type, reader::Reader},
    header::check,
    inspect_table_schema,
    table::decode_projected_record,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const CATALOG_TABLE: &str = "MSysObjects";

impl NativeImage {
    /// Read exact bounded opaque Lv/LvModule/LvExtra bytes for one captured
    /// object, without invoking active content. Qualified native OLE/long-value
    /// storage is required for present fields. Existing long-value caps and
    /// structural/resource/cancellation checks apply; failure returns no partial
    /// payload set. Presence alone never proves that writes are safe.
    pub fn read_object_payloads(
        &self,
        object: ObjectId,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ObjectPayloadData, InspectError> {
        let _span = tracing::debug_span!("native.object.payload.read").entered();
        check(budget, cancelled)?;
        let presence = self.inspect_object_payloads(object, budget, cancelled)?;
        let presences = [presence.body(), presence.module(), presence.extra()];
        let mut payloads = presences.map(|presence| OpaquePayload {
            presence,
            bytes: None,
        });
        if presences.contains(&PayloadPresence::Present) {
            let mut origin = None;
            for candidate in self.catalog().origins() {
                check(budget, cancelled)?;
                if candidate.object_id() == object {
                    origin = Some(candidate);
                    break;
                }
            }
            let origin = origin.ok_or(InspectError::UnknownObject)?;
            let mut source = self.reader();
            let schema = inspect_table_schema(
                &mut source,
                self.catalog().catalog().scope(),
                CATALOG_TABLE,
                budget,
                cancelled,
            )?;
            let mut selected = [(0, 0); PAYLOAD_FIELDS.len()];
            let mut count = 0;
            for (slot, name) in PAYLOAD_FIELDS.iter().enumerate() {
                check(budget, cancelled)?;
                if presences[slot] != PayloadPresence::Present {
                    continue;
                }
                let mut fields = schema
                    .columns()
                    .iter()
                    .enumerate()
                    .filter(|(_, field)| field.name().eq_ignore_ascii_case(name));
                let (position, field) = fields.next().ok_or(InspectError::Malformed)?;
                if fields.next().is_some() {
                    return Err(InspectError::Malformed);
                }
                if field.native_type() != data_type::OLE
                    || field.scalar_type() != Some(ScalarType::Binary)
                {
                    return Err(InspectError::Unsupported);
                }
                selected[count] = (position, slot);
                count += 1;
            }
            // The projected row decoder retains native column order, not the
            // caller's name order. Keep a separate destination slot mapping.
            selected[..count].sort_unstable_by_key(|(position, _)| *position);
            let positions = selected.map(|(position, _)| position);
            let mut reader = Reader {
                source: &mut source,
                header: self.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            };
            reader.budget.charge(Resource::Rows, 1)?;
            let raw = reader.row(origin.page(), origin.slot(), Some(schema.definition_page()))?;
            let values =
                decode_projected_record(&raw, &schema, Some(&positions[..count]), &mut reader)?;
            if values.len() != count {
                return Err(InspectError::Malformed);
            }
            for ((_, slot), value) in selected[..count].iter().zip(values) {
                reader.tick()?;
                payloads[*slot].bytes = Some(match value {
                    Value::Binary(bytes) => bytes,
                    // A non-NULL field with an empty definition is not promoted
                    // into NULL/empty bytes by this exact-preservation API.
                    Value::Null => return Err(InspectError::Unsupported),
                    _ => return Err(InspectError::Malformed),
                });
            }
        }
        check(budget, cancelled)?;
        let [body, module, extra] = payloads;
        Ok(ObjectPayloadData {
            object,
            body,
            module,
            extra,
        })
    }
}
