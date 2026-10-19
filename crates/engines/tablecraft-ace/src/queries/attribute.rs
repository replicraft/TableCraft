const START: u8 = 0;
const TYPE: u8 = 1;
const PARAMETER: u8 = 2;
const FLAGS: u8 = 3;
const REMOTE_DATABASE: u8 = 4;
const TABLE: u8 = 5;
const COLUMN: u8 = 6;
const JOIN: u8 = 7;
const WHERE: u8 = 8;
const GROUP_BY: u8 = 9;
const HAVING: u8 = 10;
const ORDER_BY: u8 = 11;
const END: u8 = 255;

/// Native MSysQueries fragment tag, without interpreting SQL or its flags.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryAttribute {
    /// Beginning metadata row.
    Start,
    /// Stored query type metadata.
    Type,
    /// Parameter declaration fragment.
    Parameter,
    /// Query options/flags fragment.
    Flags,
    /// Remote database metadata; never opens a provider.
    RemoteDatabase,
    /// Table/source fragment.
    Table,
    /// Column/result fragment.
    Column,
    /// Join fragment.
    Join,
    /// WHERE expression fragment.
    Where,
    /// Grouping fragment.
    GroupBy,
    /// HAVING expression fragment.
    Having,
    /// Ordering fragment.
    OrderBy,
    /// End metadata row.
    End,
    /// An unrecognized native tag retained without inventing semantics.
    Unknown(u8),
}
impl QueryAttribute {
    /// Preserve every native byte, including unrecognized values.
    pub const fn from_code(code: u8) -> Self {
        match code {
            START => Self::Start,
            TYPE => Self::Type,
            PARAMETER => Self::Parameter,
            FLAGS => Self::Flags,
            REMOTE_DATABASE => Self::RemoteDatabase,
            TABLE => Self::Table,
            COLUMN => Self::Column,
            JOIN => Self::Join,
            WHERE => Self::Where,
            GROUP_BY => Self::GroupBy,
            HAVING => Self::Having,
            ORDER_BY => Self::OrderBy,
            END => Self::End,
            other => Self::Unknown(other),
        }
    }
    /// Original native attribute code.
    pub const fn code(self) -> u8 {
        match self {
            Self::Start => START,
            Self::Type => TYPE,
            Self::Parameter => PARAMETER,
            Self::Flags => FLAGS,
            Self::RemoteDatabase => REMOTE_DATABASE,
            Self::Table => TABLE,
            Self::Column => COLUMN,
            Self::Join => JOIN,
            Self::Where => WHERE,
            Self::GroupBy => GROUP_BY,
            Self::Having => HAVING,
            Self::OrderBy => ORDER_BY,
            Self::End => END,
            Self::Unknown(code) => code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_attribute_byte_is_retained_and_unrecognized_tags_stay_explicit() {
        for code in u8::MIN..=u8::MAX {
            assert_eq!(QueryAttribute::from_code(code).code(), code);
        }
        assert_eq!(QueryAttribute::from_code(200), QueryAttribute::Unknown(200));
        assert_eq!(QueryAttribute::from_code(255), QueryAttribute::End);
    }
}
