use super::{Error, output::TableOutput};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::conversions::text::NumberFormat;
use tablecraft_expressions::functions::text::code_page::CharacterCodePage;
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
    functions::dates::TwoDigitYearWindow,
};
use tablecraft_sql::{
    query::{Query, ReadBody, Statement},
    select::{Projection, Select, SelectPredicate},
};

const MAX_EVAL_STEPS: u64 = 1024;

pub(super) fn run(
    query: &Query,
    year_window: Option<TwoDigitYearWindow>,
    character_code_page: Option<CharacterCodePage>,
    number_format: Option<NumberFormat>,
) -> Result<String, Error> {
    let select = scalar_select(query)?;
    // One shared allowance across all output columns, including intermediates.
    let mut budget = ResourceBudget::new(EvalLimits::MAX_TEXT_BYTES as u64, 0, MAX_EVAL_STEPS);
    let mut evaluator = Evaluator::new(&NoBindings, &mut budget, EvalLimits::default());
    if let Some(format) = number_format {
        evaluator = evaluator.with_number_format(format);
    }
    if let Some(page) = character_code_page {
        evaluator = evaluator.with_character_code_page(page);
    }
    if let Some(window) = year_window {
        evaluator = evaluator.with_year_window(window);
    }
    let mut output = TableOutput::new();
    for (index, projection) in select.projection.iter().enumerate() {
        if index > 0 {
            output.separator().map_err(Error::Output)?;
        }
        let alias = match projection {
            Projection::Expression { alias, .. } | Projection::Field { alias, .. } => alias,
            Projection::Wildcard(_) => return Err(Error::Unsupported),
        };
        output
            .header(alias.as_ref().map(|name| name.as_str()), index + 1)
            .map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    for (index, projection) in select.projection.iter().enumerate() {
        let value = match projection {
            Projection::Expression { expression, .. } => evaluator.evaluate(expression),
            Projection::Field { .. } => Err(EvalError::UnboundField),
            Projection::Wildcard(_) => return Err(Error::Unsupported),
        }
        .map_err(|error| Error::Evaluation {
            column: index + 1,
            error,
        })?;
        if index > 0 {
            output.separator().map_err(Error::Output)?;
        }
        output.value(&value).map_err(Error::Output)?;
    }
    output.newline().map_err(Error::Output)?;
    Ok(output.finish())
}
fn scalar_select(query: &Query) -> Result<&Select, Error> {
    let Statement::Read(read) = &query.statement else {
        return Err(Error::Unsupported);
    };
    let ReadBody::Select(select) = &read.body else {
        return Err(Error::Unsupported);
    };
    if !query.parameters.is_empty()
        || !read.order_by.is_empty()
        || read.owner_access
        || select.predicate != SelectPredicate::All
        || select.projection.is_empty()
        || !select.from.is_empty()
        || select.selection.is_some()
        || !select.group_by.is_empty()
        || select.having.is_some()
        || select.into.is_some()
        || select.into_external.is_some()
        || select.external.is_some()
        || select.owner_access
        || !select.order_by.is_empty()
    {
        return Err(Error::Unsupported);
    }
    Ok(select)
}
