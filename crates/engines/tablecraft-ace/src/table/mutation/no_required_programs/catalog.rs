//! Physical catalog completeness and exact seventeen-field native row shape.
use super::refused;
use crate::{
    EditError, InspectError, NativeImage, NativeTableStorage, TableSchema,
    format::{
        allocation,
        bytes::{u16_at, u32_at},
        data_type::{DATE_TIME, INT16, INT32, LONG_TEXT, OLE, SHORT_BINARY, SHORT_TEXT},
        layout::{self, Field, Layout},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES, PAGE_NUMBER_MASK},
        reader::{Reader, row_count, row_range},
        row::{
            COLUMN_COUNT_BYTES, DELETED_MASK, MASK_BITS, OFFSET_MASK, RELOCATED_MASK, reference,
            slot_offset,
        },
        text,
    },
    header::check,
    inspect_table_schema,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::LegacyDateTime,
};
use tablecraft_model::ObjectKind;

const CATALOG: &str = "MSysObjects";
const CATALOG_DEFINITION: u32 = 2;
const FIELD_COUNT: usize = 17;
const VARIABLE_COUNT: usize = 11;
const FIXED_END: usize = 32;
const LOCAL_TABLE: i16 = 1;
const CONTAINER: i16 = 3;
const DATABASE_OBJECT: i16 = 2;
const PROPERTY_OBJECT: i16 = -32757;
const ID: usize = 0;
const PARENT: usize = 1;
const NAME: usize = 2;
const KIND: usize = 3;
const CREATED: usize = 4;
const UPDATED: usize = 5;
const FLAGS: usize = 7;
const REMOTE_FIRST: usize = 8;
const REMOTE_LAST: usize = 12;
const BODY: usize = 13;
const PROPERTIES: usize = 14;
const MODULE: usize = 15;
const EXTRA: usize = 16;

// Native-number order, not physical descriptor order. The independently
// observed system descriptors use native flags13/12/32 hex, not user flags3/2.
const FIELDS: [(&str, u8, u8, usize, usize, usize); FIELD_COUNT] = [
    ("Id", INT32, 0x13, 0, 4, 0),
    ("ParentId", INT32, 0x13, 4, 4, 0),
    ("Name", SHORT_TEXT, 0x12, 0, 510, 0),
    ("Type", INT16, 0x13, 8, 2, 1),
    ("DateCreate", DATE_TIME, 0x13, 10, 8, 1),
    ("DateUpdate", DATE_TIME, 0x13, 18, 8, 1),
    ("Owner", SHORT_BINARY, 0x32, 0, 510, 1),
    ("Flags", INT32, 0x13, 26, 4, 2),
    ("Database", LONG_TEXT, 0x12, 0, 0, 2),
    ("Connect", LONG_TEXT, 0x12, 0, 0, 3),
    ("ForeignName", SHORT_TEXT, 0x12, 0, 510, 4),
    ("RmtInfoShort", SHORT_BINARY, 0x12, 0, 510, 5),
    ("RmtInfoLong", OLE, 0x12, 0, 0, 6),
    ("Lv", OLE, 0x12, 0, 0, 7),
    ("LvProp", OLE, 0x12, 0, 0, 8),
    ("LvModule", OLE, 0x12, 0, 0, 9),
    ("LvExtra", OLE, 0x12, 0, 0, 10),
];

pub(super) fn validate(
    image: &NativeImage,
    target: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<u32, EditError> {
    let scope = image.catalog().catalog().scope();
    let schema = inspect_table_schema(&mut image.reader(), scope, CATALOG, budget, cancelled)?;
    if schema.definition_page() != CATALOG_DEFINITION {
        return Err(refused());
    }
    if image.catalog().origins().len() != image.catalog().catalog().objects().len() {
        return Err(refused());
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let raw_layout = layout::read(&mut reader, CATALOG_DEFINITION)?;
    let fields = declaration(&raw_layout, reader.budget, cancelled)?;
    let mut count = 0_usize;
    let mut selected = None;
    let mut selected_parent = None;
    for number in allocation::pages(&mut reader, raw_layout.usage)? {
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != CATALOG_DEFINITION
        {
            return Err(refused());
        }
        let slots = row_count(&page)?;
        for slot in 0..slots {
            reader.tick()?;
            reader
                .budget
                .charge(Resource::Rows, 1)
                .map_err(InspectError::from)?;
            let word = u16_at(&page, slot_offset(slot))?;
            let flags = word & !OFFSET_MASK;
            if flags & DELETED_MASK != 0 && flags & !(DELETED_MASK | RELOCATED_MASK) == 0 {
                let start = usize::from(word & OFFSET_MASK);
                let end = if slot == 0 {
                    PAGE_BYTES
                } else {
                    usize::from(u16_at(&page, slot_offset(slot - 1))? & OFFSET_MASK)
                };
                // Native catalogs can retain an empty deleted directory slot.
                // It has no record bytes or payload references to omit. Retain
                // its exact word and require every nonempty row to be live.
                if start == end && start >= slot_offset(slots) && end <= PAGE_BYTES {
                    continue;
                }
                return Err(refused());
            }
            if flags != 0 {
                return Err(refused());
            }
            let (_, start, end) = row_range(&page, slot)?;
            let raw = &page[start..end];
            let layout = record_layout::parse(raw, &schema, reader.budget)?;
            if layout.count != FIELD_COUNT
                || layout.offsets.len() != VARIABLE_COUNT + 1
                || layout.offsets.first() != Some(&FIXED_END)
                || layout.offsets.last() != Some(&layout.data_end)
                || raw.len() - layout.mask_start != FIELD_COUNT.div_ceil(MASK_BITS)
                || raw[raw.len() - 1] & !1 != 0
            {
                return Err(refused());
            }
            for field in [ID, PARENT, NAME, KIND, FLAGS] {
                if !present(raw, layout.mask_start, field)? {
                    return Err(refused());
                }
            }
            let id = integer(raw, fields[ID])?;
            let parent = integer(raw, fields[PARENT])?;
            let flags = integer(raw, fields[FLAGS])? as u32;
            let kind = u16_at(raw, COLUMN_COUNT_BYTES + fields[KIND].offset)? as i16;
            let native_slot = u8::try_from(slot).map_err(|_| InspectError::StructureLimit)?;
            let mut native = None;
            for origin in image.catalog().origins() {
                reader.tick()?;
                if origin.page() == number
                    && origin.slot() == native_slot
                    && native.replace(origin).is_some()
                {
                    return Err(refused());
                }
            }
            let native = native.ok_or_else(refused)?;
            if native.native_id() != id
                || native.parent_id() != parent
                || native.type_code() != kind
                || native.flags() != flags
            {
                return Err(refused());
            }
            let mut descriptor = None;
            for object in image.catalog().catalog().objects() {
                reader.tick()?;
                if object.id() == native.object_id() && descriptor.replace(object).is_some() {
                    return Err(refused());
                }
            }
            let descriptor = descriptor.ok_or_else(refused)?;
            // Catalog LVAL tracing preserves recognized application objects,
            // database records and property records without executing them.
            // Unknown storage kinds and linked TD ownership need their own
            // complete physical contract before this profile can admit them.
            if (descriptor.kind() == ObjectKind::Unknown
                && !matches!(kind, DATABASE_OBJECT | PROPERTY_OBJECT))
                || native
                    .table_storage()
                    .is_some_and(|storage| storage != NativeTableStorage::Local)
            {
                return Err(refused());
            }
            let name = variable(raw, &layout.offsets, fields[NAME])?;
            let name = text::decode(name, true, reader.budget)?;
            reader.work(name.len().max(descriptor.name().as_str().len()))?;
            if name != descriptor.name().as_str() {
                return Err(refused());
            }
            if parent != 0
                && native.object_id() != target.object_id()
                && parent as u32 & PAGE_NUMBER_MASK == target.definition_page()
            {
                return Err(refused());
            }
            if id as u32 & PAGE_NUMBER_MASK == target.definition_page()
                && native.object_id() != target.object_id()
            {
                return Err(refused());
            }
            if native.object_id() == target.object_id() {
                if id <= 0
                    || id as u32 != target.definition_page()
                    || kind != LOCAL_TABLE
                    || native.table_storage() != Some(NativeTableStorage::Local)
                    || flags != 0
                    || name != target.name()
                {
                    return Err(refused());
                }
                // Four actual OLE payload fields, including LvProp: public
                // ObjectPayloads/coverage intentionally expose only the other3.
                for field in &fields[REMOTE_FIRST..=REMOTE_LAST] {
                    absent(raw, layout.mask_start, &layout.offsets, field)?;
                }
                for field in [BODY, PROPERTIES, MODULE, EXTRA] {
                    absent(raw, layout.mask_start, &layout.offsets, fields[field])?;
                }
                for field in [CREATED, UPDATED] {
                    if present(raw, layout.mask_start, field)? {
                        let at = COLUMN_COUNT_BYTES + fields[field].offset;
                        let bits = raw.get(at..at + size_of::<u64>()).ok_or_else(refused)?;
                        LegacyDateTime::from_bits(u64::from_le_bytes(bits.try_into().unwrap()))
                            .map_err(|_| refused())?;
                    }
                }
                if selected.replace(reference(number, native_slot)).is_some() {
                    return Err(refused());
                }
                selected_parent = Some(parent);
            }
            count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
        }
    }
    if count != schema.declared_rows() as usize || count != image.catalog().origins().len() {
        return Err(refused());
    }
    let parent = selected_parent.ok_or_else(refused)?;
    let mut tables = 0;
    for (object, origin) in image
        .catalog()
        .catalog()
        .objects()
        .iter()
        .zip(image.catalog().origins())
    {
        reader.tick()?;
        if object.id() != origin.object_id() {
            return Err(refused());
        }
        if origin.native_id() == parent
            && origin.type_code() == CONTAINER
            && object.name().as_str() == "Tables"
        {
            tables += 1;
        }
    }
    if tables != 1 {
        return Err(refused());
    }
    reader.tick()?;
    selected.ok_or_else(refused)
}

fn declaration<'layout>(
    layout: &'layout Layout,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[&'layout Field; FIELD_COUNT], EditError> {
    if layout.max_columns != FIELD_COUNT
        || layout.fields.len() != FIELD_COUNT
        || layout.variable_slots != VARIABLE_COUNT
        || layout.fixed_end != FIXED_END
    {
        return Err(refused());
    }
    // Raw TD descriptor/name order is not native-number order in these genuine
    // catalogs. Bind by the declared native number, never by vector position.
    let mut fields = [&layout.fields[0]; FIELD_COUNT];
    let mut seen = [false; FIELD_COUNT];
    for field in &layout.fields {
        check(budget, cancelled)?;
        let number = field.number;
        if number >= FIELD_COUNT || seen[number] {
            return Err(refused());
        }
        let (name, kind, flags, offset, width, variable) = FIELDS[number];
        if field.name != name
            || field.kind != kind
            || field.flags != flags
            || field.extended_flags != 0
            || field.complex_type_id.is_some()
            || field.fixed != (flags & 1 != 0)
            || field.offset != offset
            || field.length != width
            || field.variable != variable
        {
            return Err(refused());
        }
        seen[number] = true;
        fields[number] = field;
    }
    if seen.iter().any(|present| !present) {
        return Err(refused());
    }
    Ok(fields)
}

fn present(raw: &[u8], mask: usize, number: usize) -> Result<bool, EditError> {
    Ok(raw.get(mask + number / MASK_BITS).ok_or_else(refused)? & (1 << (number % MASK_BITS)) != 0)
}

fn integer(raw: &[u8], field: &Field) -> Result<i32, EditError> {
    Ok(u32_at(raw, COLUMN_COUNT_BYTES + field.offset)? as i32)
}

fn variable<'a>(raw: &'a [u8], offsets: &[usize], field: &Field) -> Result<&'a [u8], EditError> {
    let start = *offsets.get(field.variable).ok_or_else(refused)?;
    let end = *offsets.get(field.variable + 1).ok_or_else(refused)?;
    raw.get(start..end).ok_or_else(refused)
}

fn absent(raw: &[u8], mask: usize, offsets: &[usize], field: &Field) -> Result<(), EditError> {
    if present(raw, mask, field.number)? || !variable(raw, offsets, field)?.is_empty() {
        return Err(refused());
    }
    Ok(())
}
