//! Literal variable records and key bytes, independent of producer profiles.
use super::super::multiple_indexes::model as frames;
use std::collections::BTreeSet;
use tablecraft_core::values::Value;
use tablecraft_testkit::access as raw;

pub(super) const PAGE: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub(super) enum Family {
    Binary,
    Legacy,
    Modern,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Settings {
    pub family: Family,
    pub descending: bool,
    pub tuple: bool,
    pub reversed: bool,
    pub unique: bool,
    pub ignored: bool,
}

#[derive(Clone, Debug)]
pub(super) struct Row {
    pub id: i32,
    pub group: Option<i32>,
    pub payload: Option<Vec<u8>>,
}

impl Row {
    pub(super) fn values(&self, family: Family) -> Vec<Value> {
        let payload = match (&self.payload, family) {
            (None, _) => Value::Null,
            (Some(bytes), Family::Binary) => Value::Binary(bytes.clone()),
            (Some(bytes), _) => Value::Text(String::from_utf8(bytes.clone()).unwrap()),
        };
        vec![
            Value::Int32(self.id),
            self.group.map_or(Value::Null, Value::Int32),
            payload,
        ]
    }

    fn record(&self, family: Family) -> Vec<u8> {
        let mut bytes = 3_u16.to_le_bytes().to_vec();
        bytes.extend(self.id.to_le_bytes());
        bytes.extend(self.group.unwrap_or(0).to_le_bytes());
        if let Some(payload) = &self.payload {
            match family {
                Family::Binary => bytes.extend(payload),
                _ => {
                    for byte in payload {
                        bytes.extend([*byte, 0]);
                    }
                }
            }
        }
        bytes.extend((bytes.len() as u16).to_le_bytes());
        bytes.extend([10, 0, 1, 0]);
        bytes.push(
            1 | (u8::from(self.group.is_some()) << 1) | (u8::from(self.payload.is_some()) << 2),
        );
        bytes
    }
}

pub(super) struct Case {
    pub before: Vec<u8>,
    pub rows: Vec<(Row, u32)>,
    pub settings: Settings,
}

fn integral(value: Option<i32>, descending: bool) -> Vec<u8> {
    let mut key = match value {
        Some(value) => {
            let mut key = vec![127];
            key.extend(((value as u32) ^ 0x80000000).to_be_bytes());
            key
        }
        None => vec![0],
    };
    if descending {
        for byte in &mut key {
            *byte = !*byte;
        }
    }
    key
}

fn variable(value: Option<&[u8]>, settings: Settings) -> Vec<u8> {
    let Some(value) = value else {
        return vec![if settings.descending { 255 } else { 0 }];
    };
    let mut key = vec![127];
    match settings.family {
        Family::Binary => {
            key[0] = if settings.descending { 128 } else { 127 };
            for (position, chunk) in value.chunks(8).enumerate() {
                let mut segment = [if settings.descending { 255 } else { 0 }; 9];
                for (target, source) in segment.iter_mut().zip(chunk) {
                    *target = if settings.descending {
                        !*source
                    } else {
                        *source
                    };
                }
                segment[8] = if (position + 1) * 8 < value.len() {
                    9
                } else if settings.descending {
                    !(chunk.len() as u8)
                } else {
                    chunk.len() as u8
                };
                key.extend(segment);
            }
        }
        Family::Legacy | Family::Modern => {
            // Closed literal General goldens from the existing encoded-key corpus.
            assert!(value.iter().all(|byte| *byte == b'a'));
            for _ in value {
                match settings.family {
                    Family::Legacy => key.push(0x4a),
                    Family::Modern => key.extend([0x0e, 2]),
                    Family::Binary => unreachable!(),
                }
            }
            key.extend([1, 0]);
        }
    }
    if settings.descending && !matches!(settings.family, Family::Binary) {
        for byte in &mut key {
            *byte = !*byte;
        }
        key.push(0);
    }
    if matches!(settings.family, Family::Modern) && value.len() == 255 {
        assert!(!settings.tuple);
        key.truncate(508);
        key.extend(if settings.descending {
            [0xf7, 0x5b]
        } else {
            [0x27, 0xa5]
        });
    }
    key
}

fn key(row: &Row, settings: Settings, primary: bool) -> Option<Vec<u8>> {
    if primary {
        return Some(integral(Some(row.id), false));
    }
    let all_null = row.payload.is_none() && (!settings.tuple || row.group.is_none());
    if settings.ignored && all_null {
        return None;
    }
    let mut key = variable(row.payload.as_deref(), settings);
    if settings.tuple {
        let group = integral(row.group, !settings.descending);
        if settings.reversed {
            key.splice(..0, group);
        } else {
            key.extend(group);
        }
    }
    Some(key)
}

fn entries(rows: &[(Row, u32)], settings: Settings, primary: bool) -> Vec<Vec<u8>> {
    let mut entries = rows
        .iter()
        .filter_map(|(row, reference)| {
            let mut key = key(row, settings, primary)?;
            key.extend(reference.to_be_bytes());
            Some(key)
        })
        .collect::<Vec<_>>();
    // A variable key can be a proper prefix of another key. The ROW suffix
    // breaks ties only after complete key comparison, as in native entries.
    entries.sort_by(|left, right| {
        let left_key = left.len() - 4;
        let right_key = right.len() - 4;
        left[..left_key]
            .cmp(&right[..right_key])
            .then_with(|| left[left_key..].cmp(&right[right_key..]))
    });
    entries
}

pub(super) fn source(code: u8, settings: Settings, records: Vec<Row>) -> Case {
    let mut before = frames::source(code, false, false).before;
    let column = 10 * PAGE + 137;
    before[column] = if matches!(settings.family, Family::Binary) {
        9
    } else {
        10
    };
    raw::put16(&mut before, column + 23, 510);
    if !matches!(settings.family, Family::Binary) {
        super::super::properties::allow_empty_text(&mut before);
        before[column + 11..column + 15].copy_from_slice(&[
            9,
            4,
            0,
            u8::from(matches!(settings.family, Family::Modern)),
        ]);
    }
    let physical = 10 * PAGE + 244;
    let components = if !settings.tuple {
        vec![(2, settings.descending)]
    } else if settings.reversed {
        vec![(1, !settings.descending), (2, settings.descending)]
    } else {
        vec![(2, settings.descending), (1, !settings.descending)]
    };
    for (ordinal, (column, descending)) in components.into_iter().enumerate() {
        raw::put16(&mut before, physical + 4 + 3 * ordinal, column);
        before[physical + 6 + 3 * ordinal] = u8::from(!descending);
    }
    before[physical + 46] = 0x80 | u8::from(settings.unique) | (u8::from(settings.ignored) << 1);
    let encoded = records
        .iter()
        .map(|row| (0, row.record(settings.family)))
        .collect::<Vec<_>>();
    let mut page = raw::data_page(10, &encoded);
    let free =
        PAGE - 14 - 2 * encoded.len() - encoded.iter().map(|(_, row)| row.len()).sum::<usize>();
    raw::put16(&mut page, 2, free as u16);
    frames::install(&mut before, 12, &page);
    raw::put32(&mut before, 10 * PAGE + 16, records.len() as u32);
    let rows = records
        .into_iter()
        .enumerate()
        .map(|(slot, row)| (row, (12 << 8) | slot as u32))
        .collect::<Vec<_>>();
    for physical in 0..2 {
        frames::install(
            &mut before,
            13 + physical,
            &frames::leaf(None, &entries(&rows, settings, physical == 0), 0, 0, 0),
        );
    }
    Case {
        before,
        rows,
        settings,
    }
}

pub(super) fn expected(case: &Case, added: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    for row in added {
        let count =
            u16::from_le_bytes(bytes[12 * PAGE + 12..12 * PAGE + 14].try_into().unwrap()) as usize;
        let end = u16::from_le_bytes(
            bytes[12 * PAGE + 12 + count * 2..12 * PAGE + 14 + count * 2]
                .try_into()
                .unwrap(),
        ) as usize;
        let record = row.record(case.settings.family);
        let start = end - record.len();
        assert!(start >= 14 + 2 * (count + 1));
        bytes[12 * PAGE + start..12 * PAGE + end].copy_from_slice(&record);
        raw::put16(&mut bytes, 12 * PAGE + 14 + count * 2, start as u16);
        raw::put16(&mut bytes, 12 * PAGE + 12, (count + 1) as u16);
        raw::put16(
            &mut bytes,
            12 * PAGE + 2,
            (start - 14 - 2 * (count + 1)) as u16,
        );
        rows.push((row.clone(), (12 << 8) | count as u32));
    }
    let mut members = [vec![13], vec![14]];
    for (physical, members) in members.iter_mut().enumerate() {
        let entries = entries(&rows, case.settings, physical == 0);
        let old = &case.before[(13 + physical) * PAGE..(14 + physical) * PAGE];
        if frames::framing(&entries, 0).1 <= 3616 {
            frames::install(
                &mut bytes,
                13 + physical as u32,
                &frames::leaf(Some(old), &entries, 0, 0, 0),
            );
        } else {
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = entries.len() / 2;
            frames::install(
                &mut bytes,
                right,
                &frames::leaf(None, &entries[middle..], 0, left, 0),
            );
            frames::install(
                &mut bytes,
                left,
                &frames::leaf(None, &entries[..middle], 0, 0, right),
            );
            frames::install(
                &mut bytes,
                13 + physical as u32,
                &frames::root(old, left, right, &entries[middle - 1]),
            );
            members.extend([right, left]);
        }
        let mut families = case
            .rows
            .iter()
            .filter_map(|(row, _)| key(row, case.settings, physical == 0))
            .collect::<BTreeSet<_>>();
        let delta = added
            .iter()
            .filter_map(|row| key(row, case.settings, physical == 0))
            .filter(|key| families.insert(key.clone()))
            .count();
        raw::put32(
            &mut bytes,
            10 * PAGE + 67 + 12 * physical,
            [17, 29][physical] + delta as u32,
        );
    }
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    frames::map_page(
        &mut bytes,
        15,
        members.into_iter().map(frames::map).collect(),
    );
    let eof = (bytes.len() / PAGE) as u32;
    frames::map_page(&mut bytes, 1, vec![frames::map(eof..512)]);
    bytes
}
