//! Explicit, non-journalable engineering copies; ordinary native saves stay gated.
mod deleting;
mod fitted;
pub use deleting::AccessEngineeringDeleter;
mod editor;
pub use editor::{AccessEngineeringEditor, ScalarUpdateMode};
pub mod guards;
mod inserter;
pub use inserter::{AccessEngineeringInserter, ScalarInsertMode};
pub mod sql;
pub mod sql_delete;
pub mod sql_index;
pub mod sql_insert;
use tablecraft_ace::{
    Ace16AutoNumberInsertPreview, Ace16DeletePreview, Ace16IndexCreatePreview, Ace16WritePreview,
    AutoNumberRowInsert, EditError, FittedColumnUpdates, FixedColumnUpdates,
    IncrementingAutoNumber, NativeImage, RowDelete, RowInsert, SecondaryIndexCreate,
    SlotScalarUpdate, TableRowDeletes,
};
use tablecraft_ace::{InsertFieldValue, OmittedRowInsert};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::clock::Clock;
use thiserror::Error;

/// Explicit fixed-field maintenance policy. Unsupported requests never fall back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateMode {
    /// Require the existing fitting fixed/batch profile.
    Fitted,
    /// Permit qualified Int32 INDEX growth under one original-image batch.
    IndexGrowth,
    /// Expand the engine's qualified original-association update cascades.
    Cascading,
}

/// Explicit deletion action policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteMode {
    /// Check final relationship membership without expanding delete actions.
    Restrict,
    /// Expand qualified declared CASCADE DELETE and SET NULL actions.
    Cascading,
}

/// Explicit generation choice for named typed rows; no stored mode is inferred.
#[derive(Clone, Copy)]
pub enum NamedInsertMode {
    /// Ordinary fields; missing names retain original omission intent.
    Ordinary,
    /// Explicit increment-by-one using LAST from the same captured image.
    Incrementing,
    /// Explicit increment-by-one with the caller's LAST precondition.
    IncrementingLast(IncrementingAutoNumber),
}

/// Typed intent bound to one immutable image. Values and expected bytes are exact.
pub enum EngineeringRequest<'a> {
    /// Create one real physical secondary index through the native source audit.
    CreateSecondaryIndex {
        /// Exact local table name; never an implicit external source.
        table: &'a str,
        /// Original borrowed native request; unsupported options are not inferred.
        request: &'a SecondaryIndexCreate<'a>,
    },
    /// Map explicitly named typed fields once, then finish one original batch.
    NamedInsert {
        /// Exact local table, resolved by the existing native metadata service.
        table: &'a str,
        /// Written field order; absent fields retain omission intent.
        fields: &'a [String],
        /// Owned original values/presence, moved without cloning payloads.
        rows: Vec<Vec<InsertFieldValue>>,
        /// Explicit ordinary/source-counter/expected-LAST selection.
        mode: NamedInsertMode,
        /// Borrowed provider; only the native needed-default phase may read it.
        clock: Option<&'a dyn Clock>,
    },
    /// Apply original-row expected-value assignments.
    Update {
        /// Groups may span tables; engine profile checks remain authoritative.
        groups: &'a [FixedColumnUpdates<'a>],
        /// Explicit index/action maintenance policy.
        mode: UpdateMode,
    },
    /// Compose Short Text/Binary fields and optional fixed siblings in original capacity.
    FittedUpdate {
        /// Exact selected table name.
        table: &'a str,
        /// Original-row typed groups; engine storage admission remains required.
        groups: &'a [FittedColumnUpdates<'a>],
    },
    /// Joint Short Text/Binary replacements and optional fixed siblings with DATA placement.
    MixedUpdate {
        /// Exact selected table name.
        table: &'a str,
        /// Original-row groups; INDEX topology must fit without allocation.
        groups: &'a [FittedColumnUpdates<'a>],
    },
    /// One fixed/variable statement with shared DATA and qualified INDEX growth.
    MixedSplitUpdate {
        /// Exact selected table name.
        table: &'a str,
        /// Original typed groups; native source/ownership constraints stay active.
        groups: &'a [FittedColumnUpdates<'a>],
    },
    /// Replace Short Text/Binary values in one actual table and column.
    VariableUpdate {
        /// Exact selected table name.
        table: &'a str,
        /// Exact selected Short Text/Binary column name.
        column: &'a str,
        /// Original-row expected values and complete replacement values.
        rows: &'a [SlotScalarUpdate],
        /// Permit the existing qualified single-key encoded INDEX splits.
        index_growth: bool,
    },
    /// Replace ordinary Memo/OLE values using existing qualified LVAL ownership.
    LongValueUpdate {
        /// Exact selected table name.
        table: &'a str,
        /// Exact selected Memo/OLE column name.
        column: &'a str,
        /// Exact original values and replacements, without executing payloads.
        rows: &'a [SlotScalarUpdate],
    },
    /// Insert complete explicit rows in native column order.
    Insert {
        /// Exact selected table name.
        table: &'a str,
        /// Every field value, with explicit NULLs rather than defaults.
        rows: &'a [RowInsert],
    },
    /// Original ordinary field presence; only the engine resolves needed defaults.
    InsertWithOmissions {
        /// Exact selected table name.
        table: &'a str,
        /// Full native field width, distinguishing supplied NULL from omission.
        rows: &'a [OmittedRowInsert],
    },
    /// Explicit generated-primary policy with original companion presence.
    IncrementingInsertWithOmissions {
        /// Exact selected table name.
        table: &'a str,
        /// Full companion width excluding only the source-bound generated field.
        rows: &'a [OmittedRowInsert],
        /// Caller-selected incrementing policy with original LAST precondition.
        policy: IncrementingAutoNumber,
    },
    /// Select increment-by-one using original LAST, retaining companion presence.
    IncrementingInsertWithOmissionsFromSource {
        /// Exact selected table name.
        table: &'a str,
        /// Original companion presence, excluding only the generated primary.
        rows: &'a [OmittedRowInsert],
    },
    /// Source-bound increment-by-one with explicit lazy default-clock authority.
    IncrementingInsertWithClockFromSource {
        /// Exact selected table name.
        table: &'a str,
        /// Original companion presence; supplied NULL never becomes omission.
        rows: &'a [OmittedRowInsert],
        /// Borrowed provider read only by the native needed-default phase.
        clock: &'a dyn Clock,
    },
    /// Original ordinary presence with explicit needed-default clock authority.
    InsertWithClock {
        /// Exact selected table name.
        table: &'a str,
        /// Full native presence intent, retaining explicit NULL.
        rows: &'a [OmittedRowInsert],
        /// Borrowed provider; only native preparation may request one reading.
        clock: &'a dyn Clock,
    },
    /// Generated companions with source-bound incrementing and clock authority.
    IncrementingInsertWithClock {
        /// Exact selected table name.
        table: &'a str,
        /// Original companion presence, excluding the actual generated primary.
        rows: &'a [OmittedRowInsert],
        /// Explicit incrementing policy bound to original LAST.
        policy: IncrementingAutoNumber,
        /// Borrowed provider, never invoked merely to construct this request.
        clock: &'a dyn Clock,
    },
    /// Select incrementing engineering behavior; stored mode is not inferred.
    IncrementingInsert {
        /// Exact selected table name.
        table: &'a str,
        /// Companion values excluding the one generated primary column.
        rows: &'a [AutoNumberRowInsert],
        /// Caller-selected increment policy and original LAST-counter precondition.
        policy: IncrementingAutoNumber,
    },
    /// Explicitly select incrementing behavior with LAST from this same image.
    /// This does not detect the stored Increment/Random mode.
    IncrementingInsertFromSource {
        /// Exact selected table name.
        table: &'a str,
        /// Companion values excluding the one generated primary column.
        rows: &'a [AutoNumberRowInsert],
    },
    /// Capture exact original records for snapshot-local ordinals, then delete.
    DeleteOrdinals {
        /// Exact selected table name.
        table: &'a str,
        /// Explicit original live-row ordinals from the same captured image.
        ordinals: &'a [usize],
        /// Explicit relationship action policy.
        mode: DeleteMode,
    },
    /// Delete exact original logical rows and record bytes.
    Delete {
        /// Exact selected table name.
        table: &'a str,
        /// Snapshot-local ordinal and complete original record payload.
        rows: &'a [RowDelete],
        /// Explicit relationship action policy.
        mode: DeleteMode,
    },
}

/// Engine refusal or bounded preparation failure. No filesystem action occurs here.
#[derive(Debug, Error)]
pub enum EngineeringError {
    /// Actual typed native preparation, constraint or preservation error.
    #[error(transparent)]
    Engine(#[from] EditError),
    /// Actual source metadata, field binding or exact carrier failure.
    #[error(transparent)]
    Binding(#[from] crate::NativeEditError),
}

impl From<EngineeringError> for crate::NativeEditError {
    fn from(error: EngineeringError) -> Self {
        match error {
            EngineeringError::Engine(error) => Self::Edit(error),
            EngineeringError::Binding(error) => error,
        }
    }
}

enum Candidate {
    Rows(Ace16WritePreview),
    Generated(Ace16AutoNumberInsertPreview),
    Delete(Ace16DeletePreview),
    Index(Ace16IndexCreatePreview),
}

/// Audited index-creation presentation facts, separate from changed data rows.
#[derive(Clone, Copy)]
pub struct CreatedIndexMetadata<'a> {
    /// Exact new logical index name.
    pub name: &'a str,
    /// New zero-based logical descriptor number.
    pub logical_index_number: u32,
    /// New zero-based physical index-data number.
    pub physical_index_number: u32,
    /// All original rows indexed, including NULL and duplicate keys.
    pub indexed_rows: usize,
}

/// A complete disposable candidate, with no normal save, journal or commit API.
/// Required data macros are neither qualified nor executed. Hosts may explicitly
/// publish a fresh engineering copy; they must not overwrite a source or describe
/// these bytes as a qualified native save.
///
/// ```compile_fail
/// fn cannot_promote(copy: tablecraft_access::engineering::EngineeringCopy) {
///     let _: tablecraft_ace::NativeEditPlan = copy;
/// }
/// ```
pub struct EngineeringCopy {
    candidate: Candidate,
}

impl EngineeringCopy {
    /// Complete immutable original image.
    pub fn before_bytes(&self) -> &[u8] {
        match &self.candidate {
            Candidate::Rows(value) => value.before_bytes(),
            Candidate::Generated(value) => value.before_bytes(),
            Candidate::Delete(value) => value.before_bytes(),
            Candidate::Index(value) => value.before_bytes(),
        }
    }

    /// Complete independently audited engineering bytes, never a writable handle.
    pub fn candidate_bytes(&self) -> &[u8] {
        match &self.candidate {
            Candidate::Rows(value) => value.candidate_bytes(),
            Candidate::Generated(value) => value.candidate_bytes(),
            Candidate::Delete(value) => value.candidate_bytes(),
            Candidate::Index(value) => value.candidate_bytes(),
        }
    }

    /// Changed, inserted or deleted logical rows under the selected operation.
    pub fn affected_rows(&self) -> usize {
        match &self.candidate {
            Candidate::Rows(value) => value.changed_rows(),
            Candidate::Generated(value) => value.inserted_rows(),
            Candidate::Delete(value) => value.deleted_rows(),
            Candidate::Index(_) => 0,
        }
    }

    /// Index creation reports metadata without claiming any data row changed.
    pub fn created_index(&self) -> Option<CreatedIndexMetadata<'_>> {
        match &self.candidate {
            Candidate::Index(value) => Some(CreatedIndexMetadata {
                name: value.index_name(),
                logical_index_number: value.logical_index_number(),
                physical_index_number: value.physical_index_number(),
                indexed_rows: value.indexed_rows(),
            }),
            _ => None,
        }
    }

    /// Audited generated native column and request-ordered values, if requested.
    pub fn generated_values(&self) -> Option<(usize, &[i32])> {
        match &self.candidate {
            Candidate::Generated(value) => {
                Some((value.generated_column_number(), value.generated_values()))
            }
            _ => None,
        }
    }
}

/// Prepare the selected engineering operation without opening or writing files.
/// All native admission, constraints, relationships and preservation stay inside
/// the engine; no unsupported operation is retried under a different policy.
pub fn preview(
    image: NativeImage,
    request: EngineeringRequest<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<EngineeringCopy, EngineeringError> {
    let _span = tracing::debug_span!("access.edit.engineering_preview").entered();
    let candidate = match request {
        EngineeringRequest::CreateSecondaryIndex { table, request } => Candidate::Index(
            image.preview_ace16_create_secondary_index(table, request, budget, cancelled)?,
        ),
        EngineeringRequest::NamedInsert {
            table,
            fields,
            rows,
            mode,
            clock,
        } => {
            return AccessEngineeringInserter::preview_named(
                image,
                table,
                inserter::named::Request {
                    fields,
                    rows,
                    mode,
                    clock,
                },
                budget,
                cancelled,
            )
            .map_err(EngineeringError::Binding);
        }
        EngineeringRequest::Update { groups, mode } => Candidate::Rows(match mode {
            UpdateMode::Fitted => image.preview_ace16_batch_updates(groups, budget, cancelled)?,
            UpdateMode::IndexGrowth => {
                image.preview_ace16_split_batch_updates(groups, budget, cancelled)?
            }
            UpdateMode::Cascading => {
                image.preview_ace16_cascading_updates(groups, budget, cancelled)?
            }
        }),
        EngineeringRequest::FittedUpdate { table, groups } => {
            Candidate::Rows(fitted::preview(image, table, groups, budget, cancelled)?)
        }
        EngineeringRequest::MixedUpdate { table, groups } => {
            Candidate::Rows(image.preview_ace16_mixed_updates(table, groups, budget, cancelled)?)
        }
        EngineeringRequest::MixedSplitUpdate { table, groups } => Candidate::Rows(
            image.preview_ace16_mixed_split_updates(table, groups, budget, cancelled)?,
        ),
        EngineeringRequest::VariableUpdate {
            table,
            column,
            rows,
            index_growth,
        } => Candidate::Rows(if index_growth {
            image.preview_ace16_variable_split_updates(table, column, rows, budget, cancelled)?
        } else {
            image.preview_ace16_variable_updates(table, column, rows, budget, cancelled)?
        }),
        EngineeringRequest::LongValueUpdate {
            table,
            column,
            rows,
        } => Candidate::Rows(
            image.preview_ace16_long_value_updates(table, column, rows, budget, cancelled)?,
        ),
        EngineeringRequest::Insert { table, rows } => {
            Candidate::Rows(image.preview_ace16_insert_rows(table, rows, budget, cancelled)?)
        }
        EngineeringRequest::InsertWithOmissions { table, rows } => Candidate::Rows(
            image.preview_ace16_insert_rows_with_omissions(table, rows, budget, cancelled)?,
        ),
        EngineeringRequest::IncrementingInsertWithOmissions {
            table,
            rows,
            policy,
        } => Candidate::Generated(image.preview_ace16_incrementing_insert_rows_with_omissions(
            table, rows, policy, budget, cancelled,
        )?),
        EngineeringRequest::IncrementingInsertWithOmissionsFromSource { table, rows } => {
            let policy =
                image.prepare_ace16_incrementing_insert_policy(table, budget, cancelled)?;
            Candidate::Generated(image.preview_ace16_incrementing_insert_rows_with_omissions(
                table, rows, policy, budget, cancelled,
            )?)
        }
        EngineeringRequest::IncrementingInsertWithClockFromSource { table, rows, clock } => {
            let policy =
                image.prepare_ace16_incrementing_insert_policy(table, budget, cancelled)?;
            Candidate::Generated(
                image.preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
                    table, rows, policy, clock, budget, cancelled,
                )?,
            )
        }
        EngineeringRequest::InsertWithClock { table, rows, clock } => {
            Candidate::Rows(image.preview_ace16_insert_rows_with_omissions_and_clock(
                table, rows, clock, budget, cancelled,
            )?)
        }
        EngineeringRequest::IncrementingInsertWithClock {
            table,
            rows,
            policy,
            clock,
        } => Candidate::Generated(
            image.preview_ace16_incrementing_insert_rows_with_omissions_and_clock(
                table, rows, policy, clock, budget, cancelled,
            )?,
        ),
        EngineeringRequest::IncrementingInsert {
            table,
            rows,
            policy,
        } => Candidate::Generated(
            image.preview_ace16_incrementing_insert_rows(table, rows, policy, budget, cancelled)?,
        ),
        EngineeringRequest::IncrementingInsertFromSource { table, rows } => {
            let policy =
                image.prepare_ace16_incrementing_insert_policy(table, budget, cancelled)?;
            Candidate::Generated(
                image.preview_ace16_incrementing_insert_rows(
                    table, rows, policy, budget, cancelled,
                )?,
            )
        }
        EngineeringRequest::DeleteOrdinals {
            table,
            ordinals,
            mode,
        } => {
            let rows = image.prepare_ace16_row_deletes(table, ordinals, budget, cancelled)?;
            Candidate::Delete(match mode {
                DeleteMode::Restrict => {
                    image.preview_ace16_deletes(table, &rows, budget, cancelled)?
                }
                DeleteMode::Cascading => image.preview_ace16_cascading_deletes(
                    &[TableRowDeletes::new(table, &rows)],
                    budget,
                    cancelled,
                )?,
            })
        }
        EngineeringRequest::Delete { table, rows, mode } => Candidate::Delete(match mode {
            DeleteMode::Restrict => image.preview_ace16_deletes(table, rows, budget, cancelled)?,
            DeleteMode::Cascading => image.preview_ace16_cascading_deletes(
                &[TableRowDeletes::new(table, rows)],
                budget,
                cancelled,
            )?,
        }),
    };
    Ok(EngineeringCopy { candidate })
}
