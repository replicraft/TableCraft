//! Bounded, quoted TSV rows captured before any database is opened.
use csv::{ByteRecord, ReaderBuilder};
use std::{error::Error, fs::File, path::Path};
use tablecraft_ace::RowInsert;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const MAX_INPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_REQUESTS: usize = 4096;
const READ_BUFFER_BYTES: usize = 8192;
const VALUE_PAIR_FIELDS: usize = 2;
const CSV_RETAINED_RECORDS: usize = 3;
const HEX_DIGITS_PER_BYTE: usize = 2;

pub(super) fn load(
    path: &Path,
    budget: &mut ResourceBudget,
    max_value_bytes: usize,
) -> Result<Vec<RowInsert>, Box<dyn Error>> {
    let mut file = File::open(path)?;
    let length = tablecraft_io::seek_to_end(&mut file, budget, &|| false)?;
    if length > MAX_INPUT_BYTES {
        return Err("Insert-request file exceeds its 16 MiB input ceiling".into());
    }
    budget.charge_allocation(length, 1)?;
    let length = usize::try_from(length)?;
    let mut encoded = Vec::new();
    encoded.try_reserve_exact(length)?;
    encoded.resize(length, 0);
    tablecraft_io::read_exact_at(&mut file, 0, &mut encoded, budget, &|| false)?;
    if tablecraft_io::seek_to_end(&mut file, budget, &|| false)? != length as u64 {
        return Err("Insert-request file length changed during capture".into());
    }

    parse(&encoded, budget, max_value_bytes)
}

fn parse(
    encoded: &[u8],
    budget: &mut ResourceBudget,
    max_value_bytes: usize,
) -> Result<Vec<RowInsert>, Box<dyn Error>> {
    if encoded.is_empty() {
        return Err("Insert-request file is empty".into());
    }
    if encoded.len() as u64 > MAX_INPUT_BYTES {
        return Err("Insert-request file exceeds its 16 MiB input ceiling".into());
    }
    // csv retains both byte and string copies of the first record even without
    // headers. Prepay those caches and the reused record, including a hostile
    // row containing only delimiters. Capacities bound all byte/field growth.
    budget.charge(Resource::Steps, encoded.len() as u64)?;
    budget.charge_allocation(READ_BUFFER_BYTES as u64, 1)?;
    let field_capacity = encoded.len().checked_add(1).ok_or("Input size overflow")?;
    for _ in 0..CSV_RETAINED_RECORDS {
        budget.charge_allocation(encoded.len() as u64, 1)?;
        budget.charge_allocation(field_capacity as u64, size_of::<usize>() as u64)?;
    }
    budget.charge_allocation(MAX_REQUESTS as u64, size_of::<RowInsert>() as u64)?;
    let mut requests = Vec::new();
    requests.try_reserve_exact(MAX_REQUESTS)?;
    let mut reader = ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false)
        .flexible(true)
        .buffer_capacity(READ_BUFFER_BYTES)
        .from_reader(encoded);
    let mut record = ByteRecord::with_capacity(encoded.len(), field_capacity);
    let mut field_count = None;
    while reader.read_byte_record(&mut record)? {
        let number = requests.len() + 1;
        if number > MAX_REQUESTS {
            return Err("Too many insert requests".into());
        }
        if record.is_empty() || !record.len().is_multiple_of(VALUE_PAIR_FIELDS) {
            return Err(
                format!("Invalid insert request {number}: expected kind/data pairs").into(),
            );
        }
        if field_count.is_some_and(|count| count != record.len()) {
            return Err(format!("Invalid insert request {number}: inconsistent row arity").into());
        }
        field_count = Some(record.len());
        budget.charge(Resource::Rows, 1)?;
        budget.charge(Resource::Steps, record.as_slice().len() as u64)?;
        let count = record.len() / VALUE_PAIR_FIELDS;
        budget.charge_allocation(count as u64, size_of::<Value>() as u64)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count)?;
        for position in (0..record.len()).step_by(VALUE_PAIR_FIELDS) {
            values.push(value(
                &record[position],
                &record[position + 1],
                number,
                budget,
                max_value_bytes,
            )?);
        }
        requests.push(RowInsert::new(values));
    }
    if requests.is_empty() {
        return Err("Insert-request file is empty".into());
    }
    Ok(requests)
}

fn value(
    kind: &[u8],
    data: &[u8],
    number: usize,
    budget: &mut ResourceBudget,
    max_value_bytes: usize,
) -> Result<Value, Box<dyn Error>> {
    if data.len() > max_value_bytes {
        return Err(format!("Invalid insert request {number}: value exceeds input ceiling").into());
    }
    match kind {
        b"null" if data.is_empty() => Ok(Value::Null),
        b"int32" => {
            let text = std::str::from_utf8(data)
                .map_err(|_| format!("Invalid insert request {number}: Int32 is not UTF-8"))?;
            let integer = text
                .parse::<i32>()
                .map_err(|_| format!("Invalid insert request {number}: invalid Int32"))?;
            Ok(Value::Int32(integer))
        }
        b"text" => {
            let text = std::str::from_utf8(data)
                .map_err(|_| format!("Invalid insert request {number}: Text is not UTF-8"))?;
            budget.charge_allocation(data.len() as u64, 1)?;
            let mut owned = String::new();
            owned.try_reserve_exact(text.len())?;
            owned.push_str(text);
            Ok(Value::Text(owned))
        }
        b"binary" if data.len().is_multiple_of(HEX_DIGITS_PER_BYTE) => {
            budget.charge_allocation((data.len() / HEX_DIGITS_PER_BYTE) as u64, 1)?;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(data.len() / HEX_DIGITS_PER_BYTE)?;
            for pair in data.as_chunks::<HEX_DIGITS_PER_BYTE>().0 {
                let high = hex_digit(pair[0]);
                let low = hex_digit(pair[1]);
                let (Some(high), Some(low)) = (high, low) else {
                    return Err(
                        format!("Invalid insert request {number}: invalid Binary hex").into(),
                    );
                };
                bytes.push((high << 4) | low);
            }
            Ok(Value::Binary(bytes))
        }
        _ => Err(format!("Invalid insert request {number}: invalid kind or payload").into()),
    }
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
#[path = "input/tests.rs"]
mod tests;
