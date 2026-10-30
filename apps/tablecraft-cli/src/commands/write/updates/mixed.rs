//! Typed protocol grouping, preserving caller ordinals and expected carriers.
use super::super::{
    error::{Error, input},
    input::{MAX_REQUESTS, reserve},
    tick, values,
};
use super::{Group, SET_FIELDS, parse_with_kind, same_name};
use tablecraft_ace::{
    FittedColumnUpdates, FixedScalarUpdate, InspectError, NativeImage, SlotScalarUpdate,
};
use tablecraft_core::limits::ResourceBudget;

struct Scope<'a> {
    table: &'a str,
    column: &'a str,
    variable: bool,
}

enum Cell {
    Fixed(FixedScalarUpdate),
    Variable(SlotScalarUpdate),
}

enum Rows {
    Fixed(Vec<FixedScalarUpdate>),
    Variable(Vec<SlotScalarUpdate>),
}

struct Column<'a> {
    name: &'a str,
    rows: Rows,
}

pub(in crate::commands::write) struct Prepared<'a> {
    pub table: &'a str,
    columns: Vec<Column<'a>>,
}

pub(in crate::commands::write) fn required(
    cells: &[String],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, Error> {
    tick(budget, cancelled)?;
    let (assignments, remainder) = cells.as_chunks::<SET_FIELDS>();
    if assignments.is_empty() || !remainder.is_empty() || assignments.len() > MAX_REQUESTS {
        return Err(input("Expected 1..4096 complete --set assignments"));
    }
    for fields in assignments {
        tick(budget, cancelled)?;
        if !variable_kind(&fields[3]) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn variable_kind(kind: &str) -> bool {
    matches!(kind, "text" | "binary")
}

pub(in crate::commands::write) fn preflight(
    cells: &[String],
    index_growth: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    tick(budget, cancelled)?;
    let (assignments, remainder) = cells.as_chunks::<SET_FIELDS>();
    if assignments.is_empty() || !remainder.is_empty() || assignments.len() > MAX_REQUESTS {
        return Err(input("Expected 1..4096 complete --set assignments"));
    }
    let mut scopes: Vec<Scope<'_>> = Vec::new();
    for fields in assignments {
        tick(budget, cancelled)?;
        let variable = variable_kind(&fields[3]);
        if !variable && !values::fixed_kind(&fields[3]) {
            // Preserve the normal typed parser's unknown-KIND refusal rather
            // than misclassifying unknown NULLs as a supported fixed field.
            let _ = values::update(&fields[3], &fields[4], budget)?;
            return Err(input("Unknown update KIND"));
        }
        if let Some(first) = scopes.first()
            && !same_name(first.table, &fields[0], budget, cancelled)?
        {
            return Err(input("Mixed variable updates require one table"));
        }
        let mut found = None;
        for (position, scope) in scopes.iter().enumerate() {
            if same_name(scope.table, &fields[0], budget, cancelled)?
                && same_name(scope.column, &fields[1], budget, cancelled)?
            {
                found = Some(position);
                break;
            }
        }
        if let Some(position) = found {
            if scopes[position].variable != variable {
                return Err(input(
                    "One column cannot mix fixed and variable KIND categories",
                ));
            }
        } else {
            if scopes.len() == NativeImage::MAX_FIXED_BATCH_GROUPS {
                return Err(InspectError::StructureLimit.into());
            }
            reserve(&mut scopes, 1, budget)?;
            scopes.push(Scope {
                table: &fields[0],
                column: &fields[1],
                variable,
            });
        }
    }
    let mut variables = 0;
    for scope in &scopes {
        tick(budget, cancelled)?;
        variables += usize::from(scope.variable);
    }
    if variables != 1 || scopes.len() < 2 {
        return Err(input(
            "Mixed variable updates require fixed siblings and exactly one text/binary column",
        ));
    }
    if index_growth {
        return Err(input("Mixed variable INDEX growth is not qualified"));
    }
    Ok(())
}

pub(in crate::commands::write) fn prepare<'a>(
    cells: &'a [String],
    max_value_bytes: usize,
    index_growth: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Prepared<'a>, Error> {
    preflight(cells, index_growth, budget, cancelled)?;
    let groups = parse_with_kind(
        cells,
        max_value_bytes,
        |kind, ordinal, expected, value| {
            if variable_kind(kind) {
                Cell::Variable(SlotScalarUpdate::new(ordinal, expected, value))
            } else {
                Cell::Fixed(FixedScalarUpdate::new(ordinal, expected, value))
            }
        },
        budget,
        cancelled,
    )?;
    let table = groups
        .first()
        .ok_or_else(|| input("Expected mixed groups"))?
        .table;
    let mut columns = Vec::new();
    reserve(&mut columns, groups.len(), budget)?;
    for Group { column, rows, .. } in groups {
        tick(budget, cancelled)?;
        let variable = matches!(rows.first(), Some(Cell::Variable(_)));
        let rows = if variable {
            let mut values = Vec::new();
            reserve(&mut values, rows.len(), budget)?;
            for cell in rows {
                tick(budget, cancelled)?;
                let Cell::Variable(value) = cell else {
                    return Err(input("Conflicting variable KIND category"));
                };
                values.push(value);
            }
            Rows::Variable(values)
        } else {
            let mut values = Vec::new();
            reserve(&mut values, rows.len(), budget)?;
            for cell in rows {
                tick(budget, cancelled)?;
                let Cell::Fixed(value) = cell else {
                    return Err(input("Conflicting fixed KIND category"));
                };
                values.push(value);
            }
            Rows::Fixed(values)
        };
        columns.push(Column { name: column, rows });
    }
    tick(budget, cancelled)?;
    Ok(Prepared { table, columns })
}

impl Prepared<'_> {
    pub(in crate::commands::write) fn groups(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<FittedColumnUpdates<'_>>, Error> {
        let mut groups = Vec::new();
        reserve(&mut groups, self.columns.len(), budget)?;
        for column in &self.columns {
            tick(budget, cancelled)?;
            groups.push(match &column.rows {
                Rows::Fixed(updates) => FittedColumnUpdates::Fixed {
                    column: column.name,
                    updates,
                },
                Rows::Variable(updates) => FittedColumnUpdates::Variable {
                    column: column.name,
                    updates,
                },
            });
        }
        tick(budget, cancelled)?;
        Ok(groups)
    }
}
