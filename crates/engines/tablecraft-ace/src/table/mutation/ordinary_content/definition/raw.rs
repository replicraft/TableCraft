//! Shared exhaustive raw TD header, column descriptor and name classification.
use super::Role;
use crate::{
    EditError, FileFormat, InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        data_type::{
            BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, LONG_TEXT,
            NUMERIC, OLE, SHORT_BINARY, SHORT_TEXT, SINGLE,
        },
        definition as native_definition,
        layout::{
            self, AUTO_NUMBER_FLAG, AUTO_NUMBER_GUID_FLAG, FIXED_FLAG, Field, Layout,
            UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG,
        },
        page::{PAGE_BYTES, TABLE_DEFINITION_SIGNATURE},
        reader::Reader,
    },
    header::check,
    table::mutation::ordinary_content::refused,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const HEADER_BYTES: usize = layout::COLUMN_BLOCK_OFFSET;
const COLUMN_BYTES: usize = 25;
const MAGIC: u32 = 1625;
const GENERAL_SORT: [u8; 4] = [9, 4, 0, 1];
const GENERAL_LEGACY_SORT: [u8; 4] = [9, 4, 0, 0];
const FREE_OFFSET: usize = TABLE_DEFINITION_SIGNATURE.len();
const MAGIC_OFFSET: usize = native_definition::LENGTH_OFFSET + size_of::<u32>();
const LAST_ISSUED_OFFSET: usize = 20;
const AUTO_SUPPORT_OFFSET: usize = LAST_ISSUED_OFFSET + size_of::<i32>();
const TABLE_ROLE_OFFSET: usize = 40;
const MAX_COLUMNS_OFFSET: usize = 41;
const VARIABLE_COLUMNS_OFFSET: usize = MAX_COLUMNS_OFFSET + size_of::<u16>();
const COLUMN_COUNT_OFFSET: usize = VARIABLE_COLUMNS_OFFSET + size_of::<u16>();
const OWNED_MAP_OFFSET: usize = 55;
const CANDIDATE_MAP_OFFSET: usize = OWNED_MAP_OFFSET + size_of::<u32>();
const USER_TABLE_ROLE: u8 = 0x4e;
const SYSTEM_TABLE_ROLE: u8 = 0x53;
const AUTO_SUPPORT: u8 = 1;
const COLUMN_KIND_OFFSET: usize = 0;
const COLUMN_MAGIC_OFFSET: usize = COLUMN_KIND_OFFSET + size_of::<u8>();
const COLUMN_NUMBER_OFFSET: usize = COLUMN_MAGIC_OFFSET + size_of::<u32>();
const COLUMN_VARIABLE_OFFSET: usize = COLUMN_NUMBER_OFFSET + size_of::<u16>();
const COLUMN_ID_OFFSET: usize = COLUMN_VARIABLE_OFFSET + size_of::<u16>();
const COLUMN_SORT_OFFSET: usize = COLUMN_ID_OFFSET + size_of::<u16>();
const COLUMN_PRECISION_OFFSET: usize = COLUMN_SORT_OFFSET;
const COLUMN_SCALE_OFFSET: usize = COLUMN_PRECISION_OFFSET + size_of::<u8>();
const COLUMN_FLAGS_OFFSET: usize = 15;
const COLUMN_EXTENDED_OFFSET: usize = COLUMN_FLAGS_OFFSET + size_of::<u8>();
const COLUMN_PAYLOAD_OFFSET: usize = 21;
const COLUMN_WIDTH_OFFSET: usize = COLUMN_PAYLOAD_OFFSET + size_of::<u16>();
const NAME_LENGTH_BYTES: usize = size_of::<u16>();

enum TailAdmission {
    Zeroed,
    PreserveRows,
}

pub(super) struct Declaration {
    pub layout: Layout,
    pub length: usize,
    pub candidate: u32,
    pub position: usize,
    pub retained_tail: bool,
}

pub(super) fn capture(
    image: &NativeImage,
    number: u32,
    role: Role,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Declaration, EditError> {
    capture_with_tail(
        image,
        number,
        role,
        TailAdmission::Zeroed,
        budget,
        cancelled,
    )
}

pub(super) fn capture_rows(
    image: &NativeImage,
    number: u32,
    role: Role,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Declaration, EditError> {
    if !role.selected_user() {
        return Err(refused());
    }
    capture_with_tail(
        image,
        number,
        role,
        TailAdmission::PreserveRows,
        budget,
        cancelled,
    )
}

fn capture_with_tail(
    image: &NativeImage,
    number: u32,
    role: Role,
    tail_admission: TailAdmission,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Declaration, EditError> {
    check(budget, cancelled)?;
    let page = page(image, number)?;
    let layout = {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        layout::read(&mut reader, number)?
    };
    if (!role.user() && (layout.physical_indexes != 0 || layout.logical_indexes != 0))
        || layout.fields.is_empty()
        || (!role.action_ordinals() && layout.fields.len() != layout.max_columns)
        || (!role.current_schema()
            && layout.variable_slots != layout.fields.iter().filter(|field| !field.fixed).count())
    {
        return Err(refused());
    }
    if role.action_ordinals()
        && !role.current_schema()
        && layout.fields.len() != layout.max_columns
        && (layout.variable_slots != 0 || layout.fields.iter().any(|field| !field.fixed))
    {
        return Err(refused());
    }
    if role == Role::SelectedGenerated {
        if layout.fields.len() >= layout.max_columns || layout.variable_slots != 0 {
            return Err(refused());
        }
        for field in &layout.fields {
            check(budget, cancelled)?;
            if !field.fixed || field.kind != INT32 {
                return Err(refused());
            }
        }
    }
    let length = u32_at(page, native_definition::LENGTH_OFFSET)? as usize;
    if !(HEADER_BYTES..=PAGE_BYTES).contains(&length)
        || u32_at(page, native_definition::NEXT_PAGE_OFFSET)? != 0
    {
        return Err(refused());
    }
    let free = PAGE_BYTES
        .checked_sub(length + native_definition::PAGE_HEADER_BYTES)
        .ok_or_else(refused)?;
    let mut expected = [0; HEADER_BYTES];
    expected[..TABLE_DEFINITION_SIGNATURE.len()].copy_from_slice(&TABLE_DEFINITION_SIGNATURE);
    expected[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(free as u16).to_le_bytes());
    expected[native_definition::LENGTH_OFFSET..native_definition::LENGTH_OFFSET + size_of::<u32>()]
        .copy_from_slice(&(length as u32).to_le_bytes());
    expected[MAGIC_OFFSET..MAGIC_OFFSET + size_of::<u32>()].copy_from_slice(&MAGIC.to_le_bytes());
    expected[layout::RECORD_COUNT_OFFSET..layout::RECORD_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&layout.records.to_le_bytes());
    expected[AUTO_SUPPORT_OFFSET] = AUTO_SUPPORT;
    expected[TABLE_ROLE_OFFSET] = if role.user() {
        USER_TABLE_ROLE
    } else {
        SYSTEM_TABLE_ROLE
    };
    expected[MAX_COLUMNS_OFFSET..MAX_COLUMNS_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(layout.max_columns as u16).to_le_bytes());
    expected[VARIABLE_COLUMNS_OFFSET..VARIABLE_COLUMNS_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(layout.variable_slots as u16).to_le_bytes());
    expected[COLUMN_COUNT_OFFSET..COLUMN_COUNT_OFFSET + size_of::<u16>()]
        .copy_from_slice(&(layout.fields.len() as u16).to_le_bytes());
    expected[OWNED_MAP_OFFSET..OWNED_MAP_OFFSET + size_of::<u32>()]
        .copy_from_slice(&layout.usage.to_le_bytes());
    let candidate = u32_at(page, CANDIDATE_MAP_OFFSET)?;
    expected[CANDIDATE_MAP_OFFSET..HEADER_BYTES].copy_from_slice(&candidate.to_le_bytes());
    expected
        [layout::LOGICAL_INDEX_COUNT_OFFSET..layout::LOGICAL_INDEX_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&(layout.logical_indexes as u32).to_le_bytes());
    expected[layout::INDEX_COUNT_OFFSET..layout::INDEX_COUNT_OFFSET + size_of::<u32>()]
        .copy_from_slice(&(layout.physical_indexes as u32).to_le_bytes());
    let column_start = layout
        .physical_indexes
        .checked_mul(layout::INDEX_ENTRY_BYTES)
        .and_then(|bytes| HEADER_BYTES.checked_add(bytes))
        .ok_or(InspectError::StructureLimit)?;
    let mut position = layout
        .fields
        .len()
        .checked_mul(COLUMN_BYTES)
        .and_then(|bytes| column_start.checked_add(bytes))
        .ok_or(InspectError::StructureLimit)?;
    let mut has_counter = false;
    let mut ordinary_variable = 0;
    let mut long_variable = layout
        .fields
        .iter()
        .filter(|field| !field.fixed && !matches!(field.kind, OLE | LONG_TEXT))
        .count();
    for (index, field) in layout.fields.iter().enumerate() {
        check(budget, cancelled)?;
        if (!role.action_ordinals() && field.number != index)
            || (role.action_ordinals()
                && (field.number >= layout.max_columns
                    || index != 0 && layout.fields[index - 1].number >= field.number))
            || field.name.is_empty()
            || field.name.chars().any(char::is_control)
        {
            return Err(refused());
        }
        for prior in &layout.fields[..index] {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    field.name.len().max(prior.name.len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name.eq_ignore_ascii_case(&prior.name) {
                return Err(refused());
            }
        }
        field_profile(field, role)?;
        let start = column_start + index * COLUMN_BYTES;
        let raw = page.get(start..start + COLUMN_BYTES).ok_or_else(refused)?;
        let mut descriptor = [0; COLUMN_BYTES];
        descriptor[COLUMN_KIND_OFFSET] = field.kind;
        descriptor[COLUMN_MAGIC_OFFSET..COLUMN_MAGIC_OFFSET + size_of::<u32>()]
            .copy_from_slice(&MAGIC.to_le_bytes());
        descriptor[COLUMN_NUMBER_OFFSET..COLUMN_NUMBER_OFFSET + size_of::<u16>()]
            .copy_from_slice(&(field.number as u16).to_le_bytes());
        let variable = if role.current_schema() {
            // Native ordinals survive deletion independently of live descriptor
            // positions. The layout reader already checked unique variable slots
            // against the retained table counter.
            field.variable
        } else if !field.fixed && matches!(field.kind, OLE | LONG_TEXT) {
            let variable = long_variable;
            long_variable += 1;
            variable
        } else {
            let variable = ordinary_variable;
            ordinary_variable += usize::from(!field.fixed);
            variable
        };
        descriptor[COLUMN_VARIABLE_OFFSET..COLUMN_VARIABLE_OFFSET + size_of::<u16>()]
            .copy_from_slice(&(variable as u16).to_le_bytes());
        let stored_id = if role.action_ordinals() {
            let id = u16_at(raw, COLUMN_ID_OFFSET)?;
            if id == u16::MAX {
                return Err(refused());
            }
            for prior in 0..index {
                check(budget, cancelled)?;
                let previous = column_start + prior * COLUMN_BYTES + COLUMN_ID_OFFSET;
                // Descriptor order is native order; both sparse profiles
                // also require the observed stored-ID order, allowing gaps.
                if u16_at(page, previous)? >= id {
                    return Err(refused());
                }
            }
            id
        } else {
            index as u16
        };
        descriptor[COLUMN_ID_OFFSET..COLUMN_ID_OFFSET + size_of::<u16>()]
            .copy_from_slice(&stored_id.to_le_bytes());
        if matches!(field.kind, SHORT_TEXT | LONG_TEXT) {
            let sort = &raw[COLUMN_SORT_OFFSET..COLUMN_SORT_OFFSET + GENERAL_SORT.len()];
            let expected_sort = if role.selected_user() && sort == GENERAL_LEGACY_SORT {
                GENERAL_LEGACY_SORT
            } else {
                GENERAL_SORT
            };
            descriptor[COLUMN_SORT_OFFSET..COLUMN_SORT_OFFSET + GENERAL_SORT.len()]
                .copy_from_slice(&expected_sort);
        } else if field.kind == NUMERIC {
            descriptor[COLUMN_PRECISION_OFFSET] = field.precision;
            descriptor[COLUMN_SCALE_OFFSET] = field.scale;
        }
        let generator_flags = field.flags & (AUTO_NUMBER_FLAG | AUTO_NUMBER_GUID_FLAG);
        descriptor[COLUMN_FLAGS_OFFSET] = if generator_flags != 0 {
            if !role.user()
                || has_counter
                || field.kind != INT32
                || !field.fixed
                || field.length != size_of::<i32>()
                || field.extended_flags != 0
                || field.flags != (FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG)
            {
                return Err(refused());
            }
            has_counter = true;
            // LAST is an original stored historical value. Readonly closure
            // neither infers it from rows nor chooses a generation policy.
            expected[LAST_ISSUED_OFFSET..AUTO_SUPPORT_OFFSET]
                .copy_from_slice(&page[LAST_ISSUED_OFFSET..AUTO_SUPPORT_OFFSET]);
            FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG
        } else {
            UPDATABLE_FLAG | if field.fixed { FIXED_FLAG } else { 0 }
        };
        descriptor[COLUMN_EXTENDED_OFFSET] = field.extended_flags;
        descriptor[COLUMN_PAYLOAD_OFFSET..COLUMN_WIDTH_OFFSET]
            .copy_from_slice(&(field.offset as u16).to_le_bytes());
        let declared_width = if field.kind == BOOLEAN {
            let declared = u16_at(raw, COLUMN_WIDTH_OFFSET)?;
            if declared > 1 {
                return Err(refused());
            }
            declared
        } else {
            field.length as u16
        };
        descriptor[COLUMN_WIDTH_OFFSET..COLUMN_BYTES]
            .copy_from_slice(&declared_width.to_le_bytes());
        budget
            .charge(Resource::Steps, COLUMN_BYTES as u64)
            .map_err(InspectError::from)?;
        if raw != descriptor {
            return Err(refused());
        }
        let bytes = usize::from(u16_at(page, position)?);
        position += NAME_LENGTH_BYTES;
        budget
            .charge(Resource::Steps, bytes as u64)
            .map_err(InspectError::from)?;
        let encoded = page.get(position..position + bytes).ok_or_else(refused)?;
        if bytes == 0
            || bytes > crate::format::data_type::MAX_SHORT_TEXT_BYTES
            || !bytes.is_multiple_of(2)
            || !encoded
                .as_chunks::<2>()
                .0
                .iter()
                .map(|word| u16::from_le_bytes(*word))
                .eq(field.name.encode_utf16())
        {
            return Err(refused());
        }
        position += bytes;
    }
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    if role == Role::SelectedGenerated && !has_counter {
        return Err(refused());
    }
    let nonzero_tail = page[length..].iter().any(|byte| *byte != 0);
    let retained_tail = matches!(tail_admission, TailAdmission::PreserveRows)
        && image.catalog().header().format() == FileFormat::Ace2016
        && nonzero_tail;
    if page[..HEADER_BYTES] != expected
        || (nonzero_tail && !retained_tail)
        || (retained_tail
            && (length > PAGE_BYTES - super::tail::RESERVED_BYTES
                || page[PAGE_BYTES - super::tail::RESERVED_BYTES..]
                    .iter()
                    .any(|byte| *byte != 0)))
    {
        return Err(refused());
    }
    if position != layout.index_start {
        return Err(refused());
    }
    system_schema(&layout, role)?;

    Ok(Declaration {
        layout,
        length,
        candidate,
        position,
        retained_tail,
    })
}

fn field_profile(field: &Field, role: Role) -> Result<(), EditError> {
    let valid = match (field.kind, field.length, field.fixed) {
        (BOOLEAN, 0, true)
        | (BYTE, 1, true)
        | (INT16, 2, true)
        | (INT32, 4, true)
        | (INT64 | CURRENCY | DOUBLE | DATE_TIME, 8, true)
        | (SINGLE, 4, true)
        | (GUID, 16, true) => true,
        (SHORT_BINARY, 1..=255, false) => true,
        (SHORT_TEXT, 2..=510, false) => field.length.is_multiple_of(2),
        (NUMERIC, 17, false) => {
            (1..=28).contains(&field.precision) && field.scale <= field.precision
        }
        (OLE, 0, false) => {
            matches!(
                role,
                Role::Catalog | Role::SelectedUser | Role::SelectedActions
            ) || role.current_schema()
        }
        (LONG_TEXT, 0, false) => role.selected_user(),
        _ => false,
    };
    if !valid
        || (field.extended_flags != 0
            && !(matches!(field.kind, SHORT_TEXT | LONG_TEXT)
                && field.extended_flags == UNICODE_COMPRESSION_FLAG))
    {
        return Err(refused());
    }
    Ok(())
}

fn system_schema(layout: &Layout, role: Role) -> Result<(), EditError> {
    let expected: &[(&str, u8, Option<usize>, usize)] = match role {
        Role::Catalog => &[
            ("Id", INT32, Some(0), 4),
            ("ParentId", INT32, Some(4), 4),
            ("Name", SHORT_TEXT, None, 510),
            ("Type", INT16, Some(8), 2),
            ("Flags", INT32, Some(10), 4),
            ("LvProp", OLE, None, 0),
            ("Lv", OLE, None, 0),
            ("LvExtra", OLE, None, 0),
            ("LvModule", OLE, None, 0),
            ("Database", SHORT_TEXT, None, 510),
            ("ForeignName", SHORT_TEXT, None, 510),
            ("Connect", SHORT_TEXT, None, 510),
            ("Owner", SHORT_BINARY, None, 255),
            ("DateCreate", DATE_TIME, Some(14), 8),
            ("DateUpdate", DATE_TIME, Some(22), 8),
        ],
        Role::Relationships => &[
            ("szRelationship", SHORT_TEXT, None, 510),
            ("grbit", INT32, Some(0), 4),
            ("ccolumn", INT32, Some(4), 4),
            ("icolumn", INT32, Some(8), 4),
            ("szReferencedObject", SHORT_TEXT, None, 510),
            ("szObject", SHORT_TEXT, None, 510),
            ("szReferencedColumn", SHORT_TEXT, None, 510),
            ("szColumn", SHORT_TEXT, None, 510),
        ],
        Role::Aces => &[
            ("ACM", INT32, Some(0), 4),
            ("FInheritable", BOOLEAN, Some(0), 0),
            ("ObjectId", INT32, Some(4), 4),
            ("SID", SHORT_BINARY, None, 255),
        ],
        #[cfg(feature = "ace16-create")]
        Role::SelectedSchema => return Ok(()),
        Role::User | Role::SelectedUser | Role::SelectedActions | Role::SelectedGenerated => {
            return Ok(());
        }
    };
    if layout.fields.len() != expected.len() {
        return Err(refused());
    }
    for (field, &(name, kind, offset, length)) in layout.fields.iter().zip(expected) {
        if field.name != name
            || field.kind != kind
            || field.fixed != offset.is_some()
            || field.offset != offset.unwrap_or(0)
            || field.length != length
        {
            return Err(refused());
        }
    }
    Ok(())
}

pub(super) fn page(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let start = usize::try_from(number)
        .ok()
        .and_then(|number| number.checked_mul(PAGE_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    image.as_bytes().get(start..end).ok_or_else(refused)
}
