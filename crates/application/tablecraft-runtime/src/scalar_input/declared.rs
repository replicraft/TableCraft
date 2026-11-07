//! Storage input follows an explicit field declaration without dropping digits.
use super::{exact_numeric, invalid};
use crate::{Error, display::Presentation, scalar_input};
use tablecraft_core::values::Value;
use tablecraft_schema::TypeDescriptor;

pub(crate) fn parse(
    declaration: TypeDescriptor,
    text: &str,
    is_null: bool,
    output: &mut Presentation<'_>,
) -> Result<Value, Error> {
    let Some(constraints) = declaration.decimal_constraints().filter(|_| !is_null) else {
        return scalar_input::parse(declaration.scalar_type(), text, is_null, output);
    };
    let Value::Decimal(value) = exact_numeric(text, output)? else {
        return Err(invalid());
    };
    output.work(1)?;
    let value = value
        .rescale_exact(constraints.scale())
        .map_err(|_| invalid())?;
    constraints.validate(value).map_err(|_| invalid())?;
    Ok(Value::Decimal(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tablecraft_core::values::Decimal;

    #[test]
    fn declared_decimal_input_changes_scale_only_without_losing_numeric_value() {
        let declaration = TypeDescriptor::decimal(6, 2).unwrap();
        for (text, coefficient) in [("1", 100), ("1.2", 120), ("1.2300", 123), ("-0.01", -1)] {
            assert_eq!(
                parse(declaration, text, false, &mut Presentation::new(&|| false)).unwrap(),
                Value::Decimal(Decimal::new(coefficient, 2).unwrap())
            );
        }
        for text in ["1.234", "10000.00", "0.00000000000000000000000000001"] {
            assert!(parse(declaration, text, false, &mut Presentation::new(&|| false)).is_err());
        }
        assert_eq!(
            parse(
                declaration,
                "kept hidden",
                true,
                &mut Presentation::new(&|| false)
            )
            .unwrap(),
            Value::Null
        );
    }
}
