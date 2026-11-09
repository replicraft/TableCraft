//! Explicit named views of selected same-table recomputed expressions.
mod definition;
mod forwarding;

use super::{CalculationError, NativeSource};
use tablecraft_ace::NativeReadSource;
use tablecraft_core::limits::Resource;
use tablecraft_query::QueryError;
use tablecraft_sql::{ParseLimits, query::Query};

const MAX_VIEWS: usize = 256;
const MAX_VIEW_COLUMNS: usize = 256;
const MAX_NAME_BYTES: usize = 255;

/// A borrowed request for a fresh named view over explicitly selected fields.
/// Fields keep their native names; calculated fields use natural recomputed
/// expression carriers rather than their declared stored ResultType.
#[derive(Clone, Copy)]
pub struct CalculationView<'a> {
    alias: &'a str,
    table: &'a str,
    columns: &'a [&'a str],
}

impl<'a> CalculationView<'a> {
    /// Select ordered ordinary/calculated fields without inspecting any content.
    /// The source constructor validates names, declarations and dependencies.
    pub const fn new(alias: &'a str, table: &'a str, columns: &'a [&'a str]) -> Self {
        Self {
            alias,
            table,
            columns,
        }
    }
}

struct View {
    name: String,
    query: Query,
}

/// Explicit immutable named calculation views over an unchanged native source.
/// Original table/query routes and cached-read policies remain unchanged. Views
/// execute as ordinary derived query plans under the caller's cumulative query
/// budget; this facade neither evaluates formulas nor invents stored schemas.
/// Selected view projections are eager before an outer filter or TOP. Current
/// progressive execution does not admit derived views.
pub struct RecomputedNativeSource<'s, 'a, R> {
    source: &'s NativeSource<'a, R>,
    views: Vec<View>,
}

impl<'s, 'a, R: NativeReadSource> RecomputedNativeSource<'s, 'a, R> {
    /// Validate and retain explicit local-table views without reading records.
    /// At most 256 views with 1..=256 selected columns are admitted. Aliases must
    /// not collide with captured table or saved-query names. Every selected
    /// formula/dependency is preflighted, including unused IIf arms; unselected
    /// declarations remain inert. Native metadata/storage/work limits accumulate
    /// on the borrowed source, independently of subsequent query execution.
    pub fn new(
        source: &'s NativeSource<'a, R>,
        requests: &[CalculationView<'_>],
        limits: ParseLimits,
    ) -> Result<Self, CalculationError> {
        let _span = tracing::debug_span!("native.calculation.views.prepare").entered();
        Self::tick(source)?;
        if requests.len() > MAX_VIEWS {
            return Err(QueryError::StructureLimit.into());
        }
        source
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?
            .charge_allocation(requests.len() as u64, size_of::<View>() as u64)
            .map_err(QueryError::from)?;
        let mut views = Vec::new();
        views
            .try_reserve_exact(requests.len())
            .map_err(|_| QueryError::Allocation)?;

        for request in requests {
            Self::tick(source)?;
            for previous in &views {
                Self::tick(source)?;
                let previous: &View = previous;
                if previous.name.eq_ignore_ascii_case(request.alias) {
                    return Err(QueryError::AmbiguousName.into());
                }
            }
            views.push(definition::prepare(source, *request, limits)?);
        }
        Self::tick(source)?;
        Ok(Self { source, views })
    }

    fn tick(source: &NativeSource<'_, R>) -> Result<(), QueryError> {
        if (source.cancelled)() {
            return Err(QueryError::Cancelled);
        }
        source
            .budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?
            .charge(Resource::Steps, 1)?;
        Ok(())
    }
}
