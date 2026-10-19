//! Inert logical foreign-index references, separate from physical key flags.
use crate::{InspectError, format::bytes::u32_at};
use tablecraft_schema::ReferentialAction;

pub(super) const FOREIGN_INDEX: u8 = 2;
const ROLE_OFFSET: usize = 12;
const INDEX_OFFSET: usize = 13;
const TABLE_OFFSET: usize = 17;
const UPDATE_OFFSET: usize = 21;
const DELETE_OFFSET: usize = 22;
const NO_INDEX: u32 = u32::MAX;
const PRIMARY_TABLE: u8 = 1;
const SECONDARY_TABLE: u8 = 2;
const CASCADE: u8 = 1;
const SET_NULL: u8 = 2;

/// Stored reciprocal-index pointer and raw action declarations. This does not
/// establish that the target exists, agrees with MSysRelationships or has rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ForeignIndexReference {
    role: u8,
    index: u32,
    table: u32,
    updates: u8,
    deletes: u8,
}
impl ForeignIndexReference {
    /// Native table role: 1 is the parent, 2 the dependent; others are unknown.
    pub const fn native_role(self) -> u8 {
        self.role
    }
    /// Referenced table definition page, not a global semantic identity.
    pub const fn table_page(self) -> u32 {
        self.table
    }
    /// Referenced logical index number, not its physical data number.
    pub const fn index_number(self) -> u32 {
        self.index
    }
    /// Raw update-action byte; unknown bits remain visible.
    pub const fn update_flags(self) -> u8 {
        self.updates
    }
    /// Raw delete-action byte; unknown/conflicting bits remain visible.
    pub const fn delete_flags(self) -> u8 {
        self.deletes
    }
    /// Qualified update declaration, without executing a cascade.
    pub const fn on_update(self) -> Option<ReferentialAction> {
        match self.updates {
            0 => Some(ReferentialAction::Restrict),
            CASCADE => Some(ReferentialAction::Cascade),
            _ => None,
        }
    }
    /// Qualified delete declaration, without executing a cascade or SET NULL.
    pub const fn on_delete(self) -> Option<ReferentialAction> {
        match self.deletes {
            0 => Some(ReferentialAction::Restrict),
            CASCADE => Some(ReferentialAction::Cascade),
            SET_NULL => Some(ReferentialAction::SetNull),
            _ => None,
        }
    }
    pub(super) fn qualify(self) -> Result<(), InspectError> {
        if !matches!(self.role, PRIMARY_TABLE | SECONDARY_TABLE)
            || self.on_update().is_none()
            || self.on_delete().is_none()
        {
            return Err(InspectError::Unsupported);
        }
        Ok(())
    }
    pub(super) fn reciprocal(self, other: Self, table: u32, index: u32) -> bool {
        other.table == table
            && other.index == index
            && self.role != other.role
            && self.updates == other.updates
            && self.deletes == other.deletes
    }
}

pub(super) fn read(
    logical: &[u8],
    kind: u8,
) -> Result<Option<ForeignIndexReference>, InspectError> {
    if kind != FOREIGN_INDEX {
        return Ok(None);
    }
    let index = u32_at(logical, INDEX_OFFSET)?;
    if index == NO_INDEX {
        return Ok(None);
    }
    Ok(Some(ForeignIndexReference {
        role: logical[ROLE_OFFSET],
        index,
        table: u32_at(logical, TABLE_OFFSET)?,
        updates: logical[UPDATE_OFFSET],
        deletes: logical[DELETE_OFFSET],
    }))
}
