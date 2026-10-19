use crate::{
    InspectError, NativeImage,
    format::{bytes::u16_at, layout, reader::Reader, row::MASK_BITS},
    header::check,
};
use tablecraft_core::{
    ids::ObjectId,
    limits::{Resource, ResourceBudget},
};

const CATALOG_DEFINITION_PAGE: u32 = 2;
pub(super) const PAYLOAD_FIELDS: [&str; 3] = ["Lv", "LvModule", "LvExtra"];

/// Presence of one native catalog payload, without following its content pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PayloadPresence {
    /// Catalog field is NULL, including before its introduction into an old row.
    Absent,
    /// Catalog field is non-NULL; its contents and semantics remain opaque.
    Present,
    /// Catalog schema does not declare the inspected payload field.
    Uninspected,
}

/// Raw catalog payload coverage; absence does not prove all dependencies absent.
pub struct ObjectPayloads {
    body: PayloadPresence,
    module: PayloadPresence,
    extra: PayloadPresence,
}
impl ObjectPayloads {
    /// Presence of native Lv object data.
    pub const fn body(&self) -> PayloadPresence {
        self.body
    }
    /// Presence of native LvModule data, not proof of executable VBA contents.
    pub const fn module(&self) -> PayloadPresence {
        self.module
    }
    /// Presence of native LvExtra data, whose unknown semantics may affect writes.
    pub const fn extra(&self) -> PayloadPresence {
        self.extra
    }
}

impl NativeImage {
    /// Inspect only null-mask presence for one exact catalog identity. No module,
    /// extra/object stream is decoded or executed. Names/codes remain native; a
    /// missing payload column is Uninspected, never a fabricated NULL.
    pub fn inspect_object_payloads(
        &self,
        object: ObjectId,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<ObjectPayloads, InspectError> {
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
        let mut reader = Reader {
            source: &mut source,
            header: self.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let layout = layout::read(&mut reader, CATALOG_DEFINITION_PAGE)?;
        let row = reader.row(origin.page(), origin.slot(), Some(CATALOG_DEFINITION_PAGE))?;
        let count = usize::from(u16_at(&row, 0)?);
        if count > layout.max_columns {
            return Err(InspectError::Malformed);
        }
        let mask = row
            .len()
            .checked_sub(count.div_ceil(MASK_BITS))
            .ok_or(InspectError::Malformed)?;
        reader.budget.charge(
            Resource::Steps,
            layout.fields.len() as u64 * PAYLOAD_FIELDS.len() as u64,
        )?;
        let presence = |name: &str| -> Result<PayloadPresence, InspectError> {
            let mut fields = layout
                .fields
                .iter()
                .filter(|field| field.name.eq_ignore_ascii_case(name));
            let field = fields.next();
            if fields.next().is_some() {
                return Err(InspectError::Malformed);
            }
            Ok(match field {
                None => PayloadPresence::Uninspected,
                Some(field)
                    if field.number >= count
                        || row[mask + field.number / MASK_BITS]
                            & (1 << (field.number % MASK_BITS))
                            == 0 =>
                {
                    PayloadPresence::Absent
                }
                Some(_) => PayloadPresence::Present,
            })
        };
        reader.tick()?;
        let [body, module, extra] = PAYLOAD_FIELDS.map(presence);
        reader.tick()?;
        Ok(ObjectPayloads {
            body: body?,
            module: module?,
            extra: extra?,
        })
    }
}
