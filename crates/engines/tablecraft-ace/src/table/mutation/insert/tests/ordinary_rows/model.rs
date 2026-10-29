//! Literal field spans, masks and records; independent of production row codecs.
use tablecraft_core::values::{Currency, Double, Guid, LegacyDateTime, Single, Value};

pub(super) const FIXED_END: usize = 70;

#[derive(Clone)]
pub(in crate::table::mutation::insert) struct Row {
    pub earlier: Option<i32>,
    pub flag: bool,
    pub id: i32,
    pub byte: Option<u8>,
    pub small: Option<i16>,
    pub large: Option<i64>,
    pub money: Option<i64>,
    pub single: Option<u32>,
    pub double: Option<u64>,
    pub date: Option<u64>,
    pub guid: Option<[u8; 16]>,
    pub later_text: Option<String>,
    pub first_bytes: Option<Vec<u8>>,
    pub score: Option<String>,
    pub last_bytes: Option<Vec<u8>>,
    pub spare: Option<i32>,
    pub last_flag: bool,
}

impl Row {
    pub fn rich(id: i32) -> Self {
        Self {
            earlier: Some(1000 + id),
            flag: true,
            id,
            byte: Some(255),
            small: Some(i16::MIN),
            large: Some(i64::MIN + 7),
            money: Some(i64::MAX - 9),
            single: Some(0x7fc00123),
            double: Some(0x8000000000000000),
            date: Some(0xbff4000000000000),
            guid: Some([
                0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ]),
            later_text: Some("ABC".into()),
            first_bytes: Some(vec![0, 0x80, 0xff]),
            score: Some("AB".into()),
            last_bytes: Some(Vec::new()),
            spare: Some(-77),
            last_flag: false,
        }
    }

    pub fn nulls(id: i32) -> Self {
        Self {
            earlier: None,
            flag: false,
            id,
            byte: None,
            small: None,
            large: None,
            money: None,
            single: None,
            double: None,
            date: None,
            guid: None,
            later_text: None,
            first_bytes: None,
            score: None,
            last_bytes: None,
            spare: None,
            last_flag: false,
        }
    }

    pub fn values(&self, variables: bool) -> Vec<Value> {
        let mut values = vec![
            self.earlier.map_or(Value::Null, Value::Int32),
            Value::Boolean(self.flag),
            Value::Int32(self.id),
            self.byte.map_or(Value::Null, Value::Byte),
            self.small.map_or(Value::Null, Value::Int16),
            self.large.map_or(Value::Null, Value::Int64),
            self.money.map_or(Value::Null, |value| {
                Value::Currency(Currency::from_scaled(value))
            }),
            self.single
                .map_or(Value::Null, |bits| Value::Single(Single::from_bits(bits))),
            self.double
                .map_or(Value::Null, |bits| Value::Double(Double::from_bits(bits))),
            self.date.map_or(Value::Null, |bits| {
                Value::DateTime(LegacyDateTime::from_bits(bits).unwrap())
            }),
            self.guid
                .map_or(Value::Null, |bytes| Value::Guid(Guid::from_bytes(bytes))),
        ];
        if variables {
            values.extend([
                self.later_text.clone().map_or(Value::Null, Value::Text),
                self.first_bytes.clone().map_or(Value::Null, Value::Binary),
                self.score.clone().map_or(Value::Null, Value::Text),
                self.last_bytes.clone().map_or(Value::Null, Value::Binary),
            ]);
        }
        values.extend([
            self.spare.map_or(Value::Null, Value::Int32),
            Value::Boolean(self.last_flag),
        ]);
        values
    }

    pub fn record(&self, variables: bool) -> Vec<u8> {
        let columns = if variables { 17_u16 } else { 13 };
        let mut row = vec![0; FIXED_END];
        row[..2].copy_from_slice(&columns.to_le_bytes());
        if let Some(value) = self.earlier {
            row[2..6].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = self.byte {
            row[6] = value;
        }
        // Byte7 is literal padding between the Byte and Int16 spans.
        if let Some(value) = self.small {
            row[8..10].copy_from_slice(&value.to_le_bytes());
        }
        row[10..14].copy_from_slice(&self.id.to_le_bytes());
        if let Some(value) = self.large {
            row[14..22].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = self.money {
            row[22..30].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = self.single {
            row[30..34].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = self.double {
            row[34..42].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = self.date {
            row[42..50].copy_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = self.guid {
            row[50..66].copy_from_slice(&[
                value[3], value[2], value[1], value[0], value[5], value[4], value[7], value[6],
                value[8], value[9], value[10], value[11], value[12], value[13], value[14],
                value[15],
            ]);
        }
        if let Some(value) = self.spare {
            row[66..70].copy_from_slice(&value.to_le_bytes());
        }
        if variables {
            let payloads = [
                self.first_bytes.clone().unwrap_or_default(),
                text(self.score.as_deref(), false),
                text(self.later_text.as_deref(), true),
                self.last_bytes.clone().unwrap_or_default(),
            ];
            let mut offsets = vec![70_u16];
            for payload in payloads {
                row.extend(payload);
                offsets.push(row.len() as u16);
            }
            for offset in offsets.into_iter().rev() {
                row.extend(offset.to_le_bytes());
            }
            row.extend([4, 0]);
        }
        let mut present = vec![
            self.earlier.is_some(),
            self.flag,
            true,
            self.byte.is_some(),
            self.small.is_some(),
            self.large.is_some(),
            self.money.is_some(),
            self.single.is_some(),
            self.double.is_some(),
            self.date.is_some(),
            self.guid.is_some(),
        ];
        if variables {
            present.extend([
                self.later_text.is_some(),
                self.first_bytes.is_some(),
                self.score.is_some(),
                self.last_bytes.is_some(),
            ]);
        }
        present.extend([self.spare.is_some(), self.last_flag]);
        let mut mask = vec![0_u8; usize::from(columns).div_ceil(8)];
        for (number, bit) in present.into_iter().enumerate() {
            if bit {
                mask[number / 8] |= 1 << (number % 8);
            }
        }
        row.extend(mask);
        row
    }
}

fn text(value: Option<&str>, compressed: bool) -> Vec<u8> {
    let Some(value) = value else {
        return Vec::new();
    };
    let units = value.encode_utf16().collect::<Vec<_>>();
    if compressed && units.len() > 2 && units.iter().all(|unit| (1..=255).contains(unit)) {
        let mut result = vec![0xff, 0xfe];
        result.extend(units.into_iter().map(|unit| unit as u8));
        result
    } else {
        units.into_iter().flat_map(u16::to_le_bytes).collect()
    }
}
