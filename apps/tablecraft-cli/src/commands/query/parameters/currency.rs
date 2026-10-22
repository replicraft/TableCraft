use super::Error;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_expressions::{
    evaluation::{Evaluator, NoBindings},
    functions::conversions::{Conversion, text::NumberFormat},
};

pub(super) fn parse(text: &str, budget: &mut ResourceBudget) -> Result<Value, Error> {
    budget
        .charge(Resource::Bytes, text.len() as u64)
        .map_err(|_| Error::Parameter)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| Error::Parameter)?;
    owned.push_str(text);
    Evaluator::new(&NoBindings, budget, Default::default())
        .with_number_format(NumberFormat::Invariant)
        .apply_conversion(Conversion::Currency, &Value::Text(owned))
        .map_err(|_| Error::Parameter)
}
