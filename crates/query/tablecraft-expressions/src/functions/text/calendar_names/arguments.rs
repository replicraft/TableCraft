//! Ordered control admission, with caller-owned contextual conversion.
use super::CalendarName;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        dates::FirstDayOfWeek,
        text::{TextContext, TextFunction},
    },
};
use jiff::civil::Date;
use tablecraft_core::values::Value;

const MONTHS_PER_YEAR: i32 = 12;
const DAYS_PER_WEEK: i32 = 7;
const FIRST_ORDINAL: i32 = 1;
const SYSTEM_FIRST_DAY: i32 = 0;
const NAME_ANCHOR_YEAR: i16 = 2000;
const SUNDAY_ANCHOR: Date = Date::constant(2000, 1, 2);

pub(crate) fn prepare(
    function: TextFunction,
    arguments: &[Value],
    context: TextContext<'_>,
) -> Result<CalendarName, EvalError> {
    prepare_with_inputs(
        function,
        arguments.len(),
        context,
        |index| Ok(&arguments[index]),
        |conversion, value| conversion.apply(value),
    )
}

pub(crate) fn prepare_with_inputs<'a>(
    function: TextFunction,
    count: usize,
    context: TextContext<'_>,
    mut argument: impl FnMut(usize) -> Result<&'a Value, EvalError>,
    mut convert: impl FnMut(Conversion, &Value) -> Result<Value, EvalError>,
) -> Result<CalendarName, EvalError> {
    let ordinal = long(argument(0)?, &mut convert)?;
    let abbreviated = match (count > 1).then(|| argument(1)).transpose()? {
        None => false,
        Some(value) => {
            let Value::Boolean(value) = convert(Conversion::Boolean, value)? else {
                return Err(EvalError::TypeMismatch);
            };
            value
        }
    };
    let month = function == TextFunction::MonthName;
    let date = if month {
        if !(FIRST_ORDINAL..=MONTHS_PER_YEAR).contains(&ordinal) {
            return Err(EvalError::Domain);
        }
        Date::new(NAME_ANCHOR_YEAR, ordinal as i8, FIRST_ORDINAL as i8)
            .map_err(|_| EvalError::Domain)?
    } else {
        if !(FIRST_ORDINAL..=DAYS_PER_WEEK).contains(&ordinal) {
            return Err(EvalError::Domain);
        }
        let first = (count > 2)
            .then(|| argument(2))
            .transpose()?
            .map(|value| long(value, &mut convert))
            .transpose()?
            .unwrap_or(SYSTEM_FIRST_DAY);
        let first = if first == SYSTEM_FIRST_DAY {
            context
                .first_day_of_week
                .ok_or(EvalError::CalendarPolicyRequired)?
        } else {
            FirstDayOfWeek::new(first.try_into().map_err(|_| EvalError::Domain)?)?
        };
        let offset = ((ordinal - FIRST_ORDINAL) + (i32::from(first.number()) - FIRST_ORDINAL))
            % DAYS_PER_WEEK;
        // The fixed Sunday anchor plus at most six days stays in January.
        Date::new(
            SUNDAY_ANCHOR.year(),
            SUNDAY_ANCHOR.month(),
            SUNDAY_ANCHOR.day() + offset as i8,
        )
        .map_err(|_| EvalError::Domain)?
    };

    Ok(CalendarName {
        date,
        month,
        abbreviated,
        profile: context.date_format.ok_or(EvalError::DateFormatRequired)?,
    })
}

fn long(
    value: &Value,
    convert: &mut impl FnMut(Conversion, &Value) -> Result<Value, EvalError>,
) -> Result<i32, EvalError> {
    let Value::Int32(value) = convert(Conversion::Long, value)? else {
        return Err(EvalError::TypeMismatch);
    };

    Ok(value)
}
