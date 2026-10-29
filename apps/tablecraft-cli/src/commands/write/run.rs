//! Host-only source authorization and fresh-copy publication around typed services.
use super::{
    args::{Command, Delete, Insert, Paths, Update},
    error::{Error, input as invalid},
    input, publication, tick, updates,
};
use crate::native;
use std::fmt::Write;
use tablecraft_access::engineering::{
    self, DeleteMode, EngineeringCopy, EngineeringRequest, UpdateMode,
};
use tablecraft_ace::{
    AutoNumberRowInsert, FixedColumnUpdates, FixedScalarUpdate, IncrementingAutoNumber,
    InspectError, NativeImage, RowInsert, SlotScalarUpdate, inspect_header,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const SUMMARY_HEADER_BYTES: usize = 512;
const GENERATED_SUMMARY_BYTES: usize = 64;

pub(super) fn run(command: Command, password: Option<&str>) -> Result<String, Error> {
    match command {
        Command::Update(args) => update(args, password),
        Command::Insert(args) => insert(args, password),
        Command::Delete(args) => delete(args, password),
    }
}

fn update(args: Update, password: Option<&str>) -> Result<String, Error> {
    let mut budget = args.paths.limits.budget();
    if args.variable || args.long_value {
        let groups = updates::parse(
            &args.cells,
            args.value_bytes,
            SlotScalarUpdate::new,
            &mut budget,
            &|| false,
        )?;
        if groups.len() != 1 {
            return Err(invalid(
                "Variable/Memo/OLE mode requires one table and column; mixed groups are not chained",
            ));
        }
        let (fields, _) = args.cells.as_chunks::<{ updates::SET_FIELDS }>();
        if fields
            .iter()
            .any(|fields| !matches!(fields[3].as_str(), "text" | "binary"))
        {
            return Err(invalid(
                "Variable/Memo/OLE mode accepts text or binary KIND only",
            ));
        }
        let group = &groups[0];
        let request = if args.long_value {
            EngineeringRequest::LongValueUpdate {
                table: group.table,
                column: group.column,
                rows: &group.rows,
            }
        } else {
            EngineeringRequest::VariableUpdate {
                table: group.table,
                column: group.column,
                rows: &group.rows,
                index_growth: args.index_growth,
            }
        };
        return publish(&args.paths, request, password, &mut budget);
    }

    let owned = updates::parse(
        &args.cells,
        args.value_bytes,
        FixedScalarUpdate::new,
        &mut budget,
        &|| false,
    )?;
    let mut groups = Vec::new();
    input::reserve(&mut groups, owned.len(), &mut budget)?;
    for group in &owned {
        tick(&mut budget, &|| false)?;
        groups.push(FixedColumnUpdates::new(
            group.table,
            group.column,
            &group.rows,
        ));
    }
    let mode = if args.index_growth {
        UpdateMode::IndexGrowth
    } else if args.cascade_updates {
        UpdateMode::Cascading
    } else {
        UpdateMode::Fitted
    };
    publish(
        &args.paths,
        EngineeringRequest::Update {
            groups: &groups,
            mode,
        },
        password,
        &mut budget,
    )
}

fn insert(args: Insert, password: Option<&str>) -> Result<String, Error> {
    let mut budget = args.paths.limits.budget();
    let bytes = input::capture(&args.requests, &mut budget, &|| false)?;
    if let Some(last) = args.incrementing_last {
        let rows = input::inserts(
            &bytes,
            args.value_bytes,
            AutoNumberRowInsert::new,
            &mut budget,
            &|| false,
        )?;
        publish(
            &args.paths,
            EngineeringRequest::IncrementingInsert {
                table: &args.table,
                rows: &rows,
                policy: IncrementingAutoNumber::new(last),
            },
            password,
            &mut budget,
        )
    } else {
        let rows = input::inserts(
            &bytes,
            args.value_bytes,
            RowInsert::new,
            &mut budget,
            &|| false,
        )?;
        publish(
            &args.paths,
            EngineeringRequest::Insert {
                table: &args.table,
                rows: &rows,
            },
            password,
            &mut budget,
        )
    }
}

fn delete(args: Delete, password: Option<&str>) -> Result<String, Error> {
    let mut budget = args.paths.limits.budget();
    let mode = if args.cascade_deletes {
        DeleteMode::Cascading
    } else {
        DeleteMode::Restrict
    };
    if !args.requests.ordinals.is_empty() {
        if args.requests.ordinals.len() > input::MAX_REQUESTS {
            return Err(invalid("Too many delete ordinals"));
        }
        budget
            .charge_allocation(
                args.requests.ordinals.len() as u64,
                size_of::<usize>() as u64,
            )
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Rows, args.requests.ordinals.len() as u64)
            .map_err(InspectError::from)?;
        return publish(
            &args.paths,
            EngineeringRequest::DeleteOrdinals {
                table: &args.table,
                ordinals: &args.requests.ordinals,
                mode,
            },
            password,
            &mut budget,
        );
    }
    let rows = if let Some(path) = args.requests.delete_file {
        let bytes = input::capture(&path, &mut budget, &|| false)?;
        input::deletes(&bytes, &mut budget, &|| false)?
    } else {
        if args.requests.inline.len() > input::MAX_REQUESTS {
            return Err(invalid("Too many inline delete requests"));
        }
        let mut rows = Vec::new();
        input::reserve(&mut rows, args.requests.inline.len(), &mut budget)?;
        for text in &args.requests.inline {
            tick(&mut budget, &|| false)?;
            rows.push(input::delete_inline(text, &mut budget)?);
        }
        rows
    };
    publish(
        &args.paths,
        EngineeringRequest::Delete {
            table: &args.table,
            rows: &rows,
            mode,
        },
        password,
        &mut budget,
    )
}

fn publish(
    paths: &Paths,
    request: EngineeringRequest<'_>,
    _password: Option<&str>,
    budget: &mut ResourceBudget,
) -> Result<String, Error> {
    tick(budget, &|| false)?;
    // Reuse the normal native host's read-only locked/container boundary. An
    // encrypted write has no qualified rewrite path: refuse before soliciting a
    // password or constructing a decoded view. A password cannot widen admission.
    let mut source = native::open_raw(&paths.source, budget).map_err(Error::Source)?;
    if source.is_deployment() {
        return Err(Error::Deployment);
    }
    let header = inspect_header(&mut source, budget, &|| false)?;
    if header.encoded_pages() {
        return Err(Error::Encrypted);
    }
    let image = NativeImage::capture(&mut source, native::CATALOG_SCOPE, budget, &|| false)?;
    let copy = engineering::preview(image, request, budget, &|| false).map_err(Error::Preview)?;
    // Budget and build the complete success report before publishing a path.
    let summary = summary(&copy, budget)?;
    publication::publish(&paths.output, copy.candidate_bytes(), budget, &|| false)
        .map_err(Error::Publish)?;
    // The source guard remains held through final publication. NativeFile Drop
    // uses the same lock-release convention as the existing read commands.
    drop(source);
    Ok(summary)
}

fn summary(copy: &EngineeringCopy, budget: &mut ResourceBudget) -> Result<String, Error> {
    let generated = copy.generated_values();
    let count = generated.map_or(0, |(_, values)| values.len());
    let capacity = count
        .checked_mul(GENERATED_SUMMARY_BYTES)
        .and_then(|bytes| bytes.checked_add(SUMMARY_HEADER_BYTES))
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, capacity as u64)
        .map_err(InspectError::from)?;
    let mut output = String::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    writeln!(
        output,
        "Engineering copy\tExperimental; data macros and native Access saves unqualified"
    )
    .map_err(|_| invalid("Could not format engineering result"))?;
    writeln!(output, "Candidate rows\t{}", copy.affected_rows())
        .map_err(|_| invalid("Could not format engineering result"))?;
    if let Some((column, values)) = generated {
        writeln!(output, "Generated column\t{column}")
            .map_err(|_| invalid("Could not format generated result"))?;
        for (position, value) in values.iter().enumerate() {
            writeln!(output, "Generated value[{position}]\t{value}")
                .map_err(|_| invalid("Could not format generated result"))?;
        }
    }
    tick(budget, &|| false)?;
    Ok(output)
}
