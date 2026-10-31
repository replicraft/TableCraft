//! Explicit bounded roles and native system declarations; no imported template.
use super::{CreateDatabaseError, column::Declaration};
use crate::format::data_type::{
    BOOLEAN, BOOLEAN_BYTES, BYTE, BYTE_BYTES, CURRENCY, CURRENCY_BYTES, DATE_TIME, DATE_TIME_BYTES,
    DOUBLE, DOUBLE_BYTES, GUID, GUID_BYTES, INT16, INT16_BYTES, INT32, INT32_BYTES, INT64,
    INT64_BYTES, NUMERIC, NUMERIC_BYTES, OLE, SHORT_BINARY, SHORT_TEXT, SINGLE, SINGLE_BYTES,
};
use crate::format::{allocation::INLINE_BITMAP_OFFSET, page::PAGE_BYTES};
use tablecraft_schema::{DecimalConstraints, ScalarType};

pub(super) const PAGE_COUNT: usize = 9;
pub(super) const IMAGE_BYTES: usize = PAGE_COUNT * PAGE_BYTES;
pub(super) const MAX_COLUMNS: usize = 16;
pub(super) const MAX_ROWS: usize = 64;
pub(super) const MAX_NAME_UNITS: usize = 64;
pub(super) const ROOT_ID: u32 = 0x0f00_0000;
pub(super) const TABLES_ID: u32 = ROOT_ID + 1;
pub(super) const DATABASES_ID: u32 = ROOT_ID + 2;
pub(super) const RELATIONSHIPS_ID: u32 = ROOT_ID + 3;
pub(super) const SYSTEM_FLAGS: u32 = 0x8000_0002;
pub(super) const GLOBAL_PAGE: usize = 1;
pub(super) const CATALOG_DEFINITION: usize = 2;
pub(super) const MAP_PAGE: usize = 3;
pub(super) const CATALOG_DATA: usize = 4;
pub(super) const RELATIONSHIPS_DEFINITION: usize = 5;
pub(super) const ACES_DEFINITION: usize = 6;
pub(super) const USER_DEFINITION: usize = 7;
pub(super) const USER_DATA: usize = 8;
pub(super) const CATALOG_MAP_SLOT: usize = 0;
pub(super) const RELATIONSHIPS_MAP_SLOT: usize = 1;
pub(super) const ACES_MAP_SLOT: usize = 2;
pub(super) const USER_MAP_SLOT: usize = 3;
pub(super) const TABLE_COUNT: usize = 4;
pub(super) const CATALOG_OBJECT_COUNT: usize = 8;
pub(super) const LONG_VALUE_COLUMNS: [usize; 4] = [5, 6, 7, 8];
pub(super) const FIRST_LONG_VALUE_MAP_SLOT: usize = 2 * TABLE_COUNT;
pub(super) const MAP_ROWS: usize = 2 * (TABLE_COUNT + LONG_VALUE_COLUMNS.len());
pub(super) const USER_CANDIDATE_SLOT: usize = USER_MAP_SLOT + TABLE_COUNT;
pub(super) const CATALOG_CANDIDATE_SLOT: usize = CATALOG_MAP_SLOT + TABLE_COUNT;
pub(super) const CATALOG_ID: usize = 0;
pub(super) const CATALOG_PARENT: usize = 1;
pub(super) const CATALOG_NAME: usize = 2;
pub(super) const CATALOG_TYPE: usize = 3;
pub(super) const CATALOG_FLAGS: usize = 4;
pub(super) const CATALOG_CREATED: usize = 13;
pub(super) const CATALOG_UPDATED: usize = 14;
pub(super) const MAP_BITMAP_BYTES: usize = 8;
pub(super) const MAP_RECORD_BYTES: usize = INLINE_BITMAP_OFFSET + MAP_BITMAP_BYTES;
pub(super) const GENERAL_SORT: [u8; 4] = [9, 4, 0, 1];
pub(super) const SYSTEM_NAMES: [&str; 3] = ["MSysObjects", "MSysRelationships", "MSysACEs"];

#[derive(Clone, Copy)]
pub(super) struct Field<'a> {
    pub name: &'a str,
    pub kind: u8,
    pub offset: Option<usize>,
    pub width: usize,
    pub decimal: Option<DecimalConstraints>,
}

const fn field(name: &str, kind: u8, offset: Option<usize>, width: usize) -> Field<'_> {
    Field {
        name,
        kind,
        offset,
        width,
        decimal: None,
    }
}

pub(super) const CATALOG_FIELDS: [Field<'static>; 15] = [
    field("Id", INT32, Some(0), INT32_BYTES),
    field("ParentId", INT32, Some(4), INT32_BYTES),
    field("Name", SHORT_TEXT, None, 510),
    field("Type", INT16, Some(8), INT16_BYTES),
    field("Flags", INT32, Some(10), INT32_BYTES),
    field("LvProp", OLE, None, 0),
    field("Lv", OLE, None, 0),
    field("LvExtra", OLE, None, 0),
    field("LvModule", OLE, None, 0),
    field("Database", SHORT_TEXT, None, 510),
    field("ForeignName", SHORT_TEXT, None, 510),
    field("Connect", SHORT_TEXT, None, 510),
    field("Owner", SHORT_BINARY, None, 255),
    field("DateCreate", DATE_TIME, Some(14), DATE_TIME_BYTES),
    field("DateUpdate", DATE_TIME, Some(22), DATE_TIME_BYTES),
];

pub(super) const RELATIONSHIP_FIELDS: [Field<'static>; 8] = [
    field("szRelationship", SHORT_TEXT, None, 510),
    field("grbit", INT32, Some(0), INT32_BYTES),
    field("ccolumn", INT32, Some(4), INT32_BYTES),
    field("icolumn", INT32, Some(8), INT32_BYTES),
    field("szReferencedObject", SHORT_TEXT, None, 510),
    field("szObject", SHORT_TEXT, None, 510),
    field("szReferencedColumn", SHORT_TEXT, None, 510),
    field("szColumn", SHORT_TEXT, None, 510),
];

pub(super) const ACE_FIELDS: [Field<'static>; 4] = [
    field("ACM", INT32, Some(0), INT32_BYTES),
    field("FInheritable", BOOLEAN, Some(0), BOOLEAN_BYTES),
    field("ObjectId", INT32, Some(4), INT32_BYTES),
    field("SID", SHORT_BINARY, None, 255),
];

#[derive(Clone, Copy)]
pub(super) struct Fields<'a>(&'a [Field<'a>]);

impl<'a> Fields<'a> {
    pub fn new(fields: &'a [Field<'a>]) -> Self {
        Self(fields)
    }

    pub fn count(self) -> usize {
        self.0.len()
    }

    pub fn get(self, index: usize) -> Field<'a> {
        self.0[index]
    }

    pub fn variable_count(self) -> usize {
        (0..self.count())
            .filter(|&index| self.get(index).offset.is_none())
            .count()
    }

    pub fn ordinary_variable_count(self) -> usize {
        (0..self.count())
            .filter(|&index| {
                let field = self.get(index);
                field.offset.is_none() && field.kind != OLE
            })
            .count()
    }

    pub fn fixed_end(self) -> usize {
        size_of::<u16>()
            + (0..self.count())
                .filter_map(|index| {
                    let field = self.get(index);
                    field.offset.map(|offset| offset + field.width)
                })
                .max()
                .unwrap_or(0)
    }
}

/// One internal catalog row, without an independently allocated value vector.
pub(super) struct CatalogObject<'a> {
    pub id: u32,
    pub parent: u32,
    pub name: super::stored_text::Plan<'a>,
    pub system: bool,
}

/// Closed native fixed-family selection; all variable and extended profiles
/// use separate explicit metadata binding after original SOURCE qualification.
pub(super) fn fixed_kind(scalar: ScalarType) -> Option<(u8, usize)> {
    Some(match scalar {
        ScalarType::Boolean => (BOOLEAN, BOOLEAN_BYTES),
        ScalarType::Byte => (BYTE, BYTE_BYTES),
        ScalarType::Int16 => (INT16, INT16_BYTES),
        ScalarType::Int32 => (INT32, INT32_BYTES),
        ScalarType::Int64 => (INT64, INT64_BYTES),
        ScalarType::Currency => (CURRENCY, CURRENCY_BYTES),
        ScalarType::Single => (SINGLE, SINGLE_BYTES),
        ScalarType::Double => (DOUBLE, DOUBLE_BYTES),
        ScalarType::DateTime => (DATE_TIME, DATE_TIME_BYTES),
        ScalarType::Guid => (GUID, GUID_BYTES),
        _ => return None,
    })
}

/// Bind actual native declaration metadata before any row or image allocation.
pub(super) fn bind<'a>(
    name: &'a str,
    declaration: Declaration,
    fixed_offset: usize,
) -> Result<Field<'a>, CreateDatabaseError> {
    declaration.validate()?;
    let (kind, offset, width, decimal) = match declaration {
        Declaration::Fixed(scalar) => {
            let (kind, width) = fixed_kind(scalar).ok_or(CreateDatabaseError::UnsupportedType)?;
            (
                kind,
                Some(if width == 0 { 0 } else { fixed_offset }),
                width,
                None,
            )
        }
        Declaration::ShortText { units } => (
            SHORT_TEXT,
            None,
            units
                .checked_mul(size_of::<u16>())
                .ok_or(CreateDatabaseError::Declaration)?,
            None,
        ),
        Declaration::ShortBinary { bytes } => (SHORT_BINARY, None, bytes, None),
        Declaration::Decimal(constraints) => (NUMERIC, None, NUMERIC_BYTES, Some(constraints)),
    };
    Ok(Field {
        name,
        kind,
        offset,
        width,
        decimal,
    })
}
