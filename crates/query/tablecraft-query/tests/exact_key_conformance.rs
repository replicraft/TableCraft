//! Independent encounter-order models for proven exact integral key populations.
use std::cell::Cell;
use std::collections::BTreeMap;
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_query::{
    ExecutionOptions, QueryError, QueryLimits, prepare,
    source::{MemorySource, MemoryTable, Source, TextCollationId},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::parse_query;

#[path = "exact_key_conformance/boundaries.rs"]
mod boundaries;

type ModelKey = (Option<i128>, Option<i128>);
type ModelGroup = (Option<i128>, Option<i128>, i64);
#[path = "exact_key_conformance/values.rs"]
mod values;

fn run(
    sql: &str,
    source: &dyn Source,
    options: ExecutionOptions<'_>,
) -> Result<Vec<Vec<Value>>, QueryError> {
    let syntax = parse_query(sql, Default::default()).unwrap();
    prepare(syntax.query(), source)?
        .execute(source, &[], options)
        .map(|result| result.rows().to_vec())
}

fn encode(kind: ScalarType, value: Option<i128>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    match kind {
        ScalarType::Boolean => Value::Boolean(value != 0),
        ScalarType::Byte => Value::Byte(u8::try_from(value).unwrap()),
        ScalarType::Int16 => Value::Int16(i16::try_from(value).unwrap()),
        ScalarType::Int32 => Value::Int32(i32::try_from(value).unwrap()),
        ScalarType::Int64 => Value::Int64(i64::try_from(value).unwrap()),
        _ => panic!("test profile must be an exact integral carrier"),
    }
}

fn population(kind: ScalarType) -> (MemorySource, Vec<ModelKey>) {
    let mut model = Vec::new();
    let mut rows = Vec::new();
    for index in 0..101 {
        let ordinal = index * 37 % 101;
        let key = if ordinal % 11 == 0 {
            None
        } else {
            let slot = usize::try_from(ordinal % 7).unwrap();
            let value = match kind {
                ScalarType::Boolean => ordinal % 2,
                ScalarType::Byte => [0, 1, 2, 127, 128, 254, 255][slot],
                ScalarType::Int16 => [-32768, 32767, -1, 0, 1, -32767, 32766][slot],
                ScalarType::Int32 => {
                    [-2147483648, 2147483647, -1, 0, 1, -2147483647, 2147483646][slot]
                }
                ScalarType::Int64 => [
                    -9223372036854775808_i128,
                    9223372036854775807,
                    9007199254740992,
                    9007199254740993,
                    -9007199254740992,
                    -9007199254740993,
                    0,
                ][slot],
                _ => panic!("test profile must be an exact integral carrier"),
            };
            Some(value)
        };
        let second = (ordinal % 13 != 0).then_some(ordinal % 3 - 1);
        model.push((key, second));
        rows.push(vec![encode(kind, key), encode(ScalarType::Int16, second)]);
    }
    let table = MemoryTable::new(
        "T".into(),
        vec![
            Column::new("K".into(), kind, true).unwrap(),
            Column::new("G".into(), ScalarType::Int16, true).unwrap(),
        ],
        rows,
    )
    .unwrap();
    (MemorySource::new(vec![table]).unwrap(), model)
}

fn grouped(model: &[ModelKey]) -> Vec<ModelGroup> {
    let mut positions = BTreeMap::new();
    let mut result = Vec::new();
    for &(first, second) in model {
        let index = *positions.entry((first, second)).or_insert_with(|| {
            result.push((first, second, 0_i64));
            result.len() - 1
        });
        result[index].2 += 1;
    }
    result
}

struct Unique {
    columns: Vec<Column>,
    policies: Vec<Option<TextCollationId>>,
    count: usize,
    visits: Cell<usize>,
}

impl Unique {
    fn new(count: usize) -> Self {
        Self {
            columns: vec![
                Column::new("Unused".into(), ScalarType::Text, true).unwrap(),
                Column::new("K".into(), ScalarType::Int32, false).unwrap(),
            ],
            policies: vec![Some(TextCollationId::from_host(999)), None],
            count,
            visits: Cell::new(0),
        }
    }
}

impl Source for Unique {
    fn columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        Ok(&self.columns)
    }

    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&self.policies)
    }

    fn scan(
        &self,
        _table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        for index in (0..self.count).rev() {
            self.visits.set(self.visits.get() + 1);
            let row = [Value::Null, Value::Int32(i32::try_from(index).unwrap())];
            visitor(&row)?;
        }
        Ok(())
    }
}
