//! Explicit database query options under the source's cumulative native allowance.
use super::NativeSource;
use tablecraft_ace::{
    DatabaseQueryMode, InspectError, NativeReadSource, inspect_database_properties,
};

impl<R: NativeReadSource> NativeSource<'_, R> {
    /// Read the qualified stored ANSI query mode without opening user tables or
    /// inferring a default from the format, sort order or host environment.
    /// Missing database metadata remains Absent; actual read/role errors propagate.
    pub fn database_query_mode(&mut self) -> Result<DatabaseQueryMode, InspectError> {
        if (self.cancelled)() {
            return Err(InspectError::Cancelled);
        }
        let mut budget = self
            .budget
            .try_borrow_mut()
            .map_err(|_| InspectError::Unsupported)?;
        let properties = inspect_database_properties(
            self.source.get_mut(),
            self.scope,
            &mut budget,
            self.cancelled,
        )?;
        match properties {
            Some(properties) => properties.query_mode(&mut budget, self.cancelled),
            None => Ok(DatabaseQueryMode::Absent),
        }
    }
}
