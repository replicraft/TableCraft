use crate::commands::typed_values;
use std::{fmt, io};
use tablecraft_access::creation::{CreationError, CreationReceipt};
use tablecraft_ace::{CreateDatabaseError, InspectError};

#[derive(Debug)]
pub(crate) enum Error {
    Password,
    Input(&'static str),
    File(io::ErrorKind),
    Native(InspectError),
    Creation(CreationError),
    Output {
        receipt: CreationReceipt,
        cause: io::Error,
    },
}

impl Error {
    pub(crate) fn invalid_input(&self) -> bool {
        matches!(
            self,
            Self::Password
                | Self::Input(_)
                | Self::Creation(CreationError::Create(
                    CreateDatabaseError::InvalidName
                        | CreateDatabaseError::DuplicateColumn
                        | CreateDatabaseError::SystemTableName
                        | CreateDatabaseError::ColumnLimit
                        | CreateDatabaseError::RowLimit
                        | CreateDatabaseError::RowWidth
                        | CreateDatabaseError::PageCapacity
                        | CreateDatabaseError::TextEncoding
                        | CreateDatabaseError::Declaration
                        | CreateDatabaseError::DeclaredValue
                        | CreateDatabaseError::Carrier
                        | CreateDatabaseError::FieldConstraint(_)
                        | CreateDatabaseError::IndexConstraint(_)
                        | CreateDatabaseError::UnsupportedType
                ))
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Password => formatter
                .write_str("create produces unencrypted databases; --password is unsupported"),
            Self::Input(message) => formatter.write_str(message),
            Self::File(kind) => write!(formatter, "could not capture the creation TSV ({kind:?})"),
            Self::Native(error) => error.fmt(formatter),
            Self::Creation(error) => error.fmt(formatter),
            Self::Output { receipt, cause } => write!(
                formatter,
                "database was published with {} rows and {} columns; namespace barrier acknowledged; completion output failed: {cause}",
                receipt.row_count(),
                receipt.column_count(),
            ),
        }
    }
}

impl From<InspectError> for Error {
    fn from(error: InspectError) -> Self {
        Self::Native(error)
    }
}

impl From<typed_values::Error> for Error {
    fn from(cause: typed_values::Error) -> Self {
        match cause {
            typed_values::Error::Input(message) => Self::Input(message),
            typed_values::Error::Native(cause) => Self::Native(cause),
        }
    }
}
