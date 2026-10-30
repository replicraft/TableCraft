//! Literal records, complete key/ROW streams and image effects for two owners.
use std::collections::BTreeSet;
use tablecraft_testkit::access::{
    self as raw,
    multiple_indexes::{
        Case, PAGE, Row, framing, install, leaf, map, map_page, record, root, source, wires,
    },
};

pub fn expected(case: &Case, requests: &[Row]) -> Vec<u8> {
    let mut bytes = case.before.clone();
    let mut rows = case.rows.clone();
    let mut owned = case.data.clone();
    let mut hints = case.data.clone();
    for row in requests {
        let encoded = record(row);
        let candidate = *hints.iter().max().unwrap();
        let start = candidate as usize * PAGE;
        let free = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap()) as usize;
        let page = if free < encoded.len() + 2 {
            hints.retain(|number| *number != candidate);
            let next = (bytes.len() / PAGE) as u32;
            bytes.resize(bytes.len() + PAGE, 0);
            owned.push(next);
            hints.push(next);
            install(&mut bytes, next, &raw::data_page(10, &[]));
            next
        } else {
            candidate
        };
        let start = page as usize * PAGE;
        let count = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap()) as usize;
        let end = if count == 0 {
            PAGE
        } else {
            u16::from_le_bytes(
                bytes[start + 12 + count * 2..start + 14 + count * 2]
                    .try_into()
                    .unwrap(),
            ) as usize
        };
        let at = end - encoded.len();
        bytes[start + at..start + end].copy_from_slice(&encoded);
        raw::put16(&mut bytes, start + 14 + count * 2, at as u16);
        raw::put16(&mut bytes, start + 12, (count + 1) as u16);
        raw::put16(&mut bytes, start + 2, (at - 14 - 2 * (count + 1)) as u16);
        rows.push((row.clone(), (page << 8) | count as u32));
    }
    let mut members = [vec![13], vec![14]];
    for (index, members) in members.iter_mut().enumerate() {
        let entries = wires(&rows, index == 1, case.descending);
        let old = &case.before[(13 + index) * PAGE..(14 + index) * PAGE];
        let seed = u16::from_le_bytes(old[24..26].try_into().unwrap()) as usize;
        if framing(&entries, seed).1 <= 3616 {
            install(
                &mut bytes,
                13 + index as u32,
                &leaf(Some(old), &entries, seed, 0, 0),
            );
        } else {
            let right = (bytes.len() / PAGE) as u32;
            let left = right + 1;
            bytes.resize(bytes.len() + 2 * PAGE, 0);
            let middle = entries.len() / 2;
            install(
                &mut bytes,
                right,
                &leaf(None, &entries[middle..], 0, left, 0),
            );
            install(
                &mut bytes,
                left,
                &leaf(None, &entries[..middle], 0, 0, right),
            );
            install(
                &mut bytes,
                13 + index as u32,
                &root(old, left, right, &entries[middle - 1]),
            );
            members.extend([right, left]);
        }
    }
    let mut known = case
        .rows
        .iter()
        .map(|(row, _)| row.group)
        .collect::<BTreeSet<_>>();
    let mut secondary_delta = 0;
    for row in requests {
        secondary_delta += u32::from(known.insert(row.group));
    }
    raw::put32(&mut bytes, 10 * PAGE + 16, rows.len() as u32);
    raw::put32(
        &mut bytes,
        10 * PAGE + 67,
        case.histories[0] + requests.len() as u32,
    );
    raw::put32(
        &mut bytes,
        10 * PAGE + 79,
        case.histories[1] + secondary_delta,
    );
    map_page(&mut bytes, 11, vec![map(owned), map(hints)]);
    map_page(&mut bytes, 15, members.into_iter().map(map).collect());
    let eof = (bytes.len() / PAGE) as u32;
    map_page(&mut bytes, 1, vec![map(eof..512)]);
    bytes
}

pub fn descending(mut case: Case) -> Case {
    case.descending = true;
    case.before[10 * PAGE + 250] = 0;
    let entries = wires(&case.rows, true, true);
    install(&mut case.before, 14, &leaf(None, &entries, 0, 0, 0));
    case
}

pub fn ignored_null_source(code: u8) -> Case {
    let mut case = source(code, false, false);
    case.before[10 * PAGE + 290] = 0x82;
    let entries = wires(&case.rows, true, false)
        .into_iter()
        .filter(|wire| wire[0] != 0)
        .collect::<Vec<_>>();
    install(&mut case.before, 14, &leaf(None, &entries, 0, 0, 0));
    case
}

/// Each physical index is independently readable, but their page ownership is
/// deliberately shared. Distinct usage records do not prove tree exclusivity.
pub fn shared_root_source(code: u8) -> Case {
    let mut case = source(code, false, false);
    for (row, _) in &mut case.rows {
        row.group = Some(row.id);
    }
    let records = case
        .rows
        .iter()
        .map(|(row, _)| (0, record(row)))
        .collect::<Vec<_>>();
    let free =
        PAGE - 14 - 2 * records.len() - records.iter().map(|(_, bytes)| bytes.len()).sum::<usize>();
    let mut data = raw::data_page(10, &records);
    raw::put16(&mut data, 2, free as u16);
    install(&mut case.before, 12, &data);
    // The primary13 key/ROW stream now describes both actual columns. The old
    // secondary14 remains allocated and untouched as an orphan source page.
    raw::put32(&mut case.before, 10 * PAGE + 282, 13);
    map_page(&mut case.before, 15, vec![map([13]), map([13])]);
    case
}
