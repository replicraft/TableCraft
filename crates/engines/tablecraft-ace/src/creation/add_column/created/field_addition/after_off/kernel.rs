//! Actual append computation shared by the original candidate and source replay.
use super::super::build;
use super::{
    FieldAdditionAfterOffError as Error, FieldAdditionError, FieldAdditionTarget, check_request,
    source::{ProvedImage, Source},
};
use crate::{
    CreateColumn, InspectError, NativeImage,
    creation::{
        FieldHistoryOffSource,
        add_column::{common, property_preservation},
    },
    format::page::PAGE_BYTES,
    header::check,
    table::mutation::catalog_property,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Constructed<P: ProvedImage> {
    pub proof: P,
    pub after: Vec<u8>,
    pub table: String,
    pub column: String,
    pub columns: usize,
}

pub(super) fn append<P: ProvedImage>(
    source: Source<P>,
    column: &str,
    target: FieldAdditionTarget,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Constructed<P>, Error> {
    let mut columns = source
        .definition
        .borrowed_columns(budget, cancelled)
        .map_err(FieldAdditionError::from)?;
    budget.charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)?;
    columns
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    columns.push(target.declaration(column)?);
    let order = build::extend_order(source.order, &columns, target, budget, cancelled)?;
    let image = source.proof.image();
    let rebuilt = build::canonical(
        source.checked.schema().name(),
        &columns,
        order,
        budget,
        cancelled,
    )?;
    let mut after = common::copy_definition(
        image,
        &rebuilt,
        source.checked.schema().definition_page() as usize * PAGE_BYTES,
        budget,
        cancelled,
    )?;
    super::super::verify::definition(
        image,
        &after,
        &source.checked,
        column,
        target,
        budget,
        cancelled,
    )?;
    let body = build::property_body(&columns, order, budget, cancelled)?;
    let change = catalog_property::compose_fitted_at_origin(
        image,
        source.checked.schema(),
        body,
        budget,
        cancelled,
    )?;
    change.apply(&mut after, budget, cancelled)?;
    property_preservation::repeated(
        image,
        &after,
        &source.checked,
        column,
        &change,
        budget,
        cancelled,
    )
    .map_err(FieldAdditionError::from)?;
    super::verify::candidate(&source, &after, &change, budget, cancelled)?;
    let table = super::super::super::super::owned(source.checked.schema().name(), budget)?;
    let column = super::super::super::super::owned(column, budget)?;
    let count = columns.len();
    drop(columns);
    check(budget, cancelled)?;
    Ok(Constructed {
        proof: source.proof,
        after,
        table,
        column,
        columns: count,
    })
}

/// No arbitrary-image constructor: this owner starts only from the real settled
/// proof and advances only from the native output of the shared append above.
pub(super) struct Replay {
    image: NativeImage,
}

impl ProvedImage for Replay {
    fn image(&self) -> &NativeImage {
        &self.image
    }
}

impl Replay {
    pub(super) fn start(
        proof: FieldHistoryOffSource,
        table: &str,
        column: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check_request(table, column, target, budget, cancelled)?;
        let source = Source::from_proof(proof, table, budget, cancelled)?;
        Self::from_constructed(
            append(source, column, target, budget, cancelled)?,
            budget,
            cancelled,
        )
    }

    pub(super) fn advance(
        self,
        table: &str,
        column: &str,
        target: FieldAdditionTarget,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        check_request(table, column, target, budget, cancelled)?;
        let source = Source::from_proof(self, table, budget, cancelled)?;
        Self::from_constructed(
            append(source, column, target, budget, cancelled)?,
            budget,
            cancelled,
        )
    }

    fn from_constructed<P: ProvedImage>(
        built: Constructed<P>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, Error> {
        let scope = built.proof.image().catalog().catalog().scope();
        let image = NativeImage::capture(&mut Cursor::new(built.after), scope, budget, cancelled)?;
        Ok(Self { image })
    }
}
