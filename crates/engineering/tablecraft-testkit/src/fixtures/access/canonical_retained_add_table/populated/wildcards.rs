//! Literal manual ID/Score output after selecting a generated source ID.
use super::{Column, append};

/// Two ordinary nullable LONG fields, no primary, generation or property body.
/// IDs are selected literal values rather than native generated requests.
pub fn id_scores(before: &[u8], table: &str, values: &[(i32, Option<i32>)]) -> Vec<u8> {
    let columns = [
        Column {
            name: "ID",
            native: 4,
            width: 4,
            variable: None,
        },
        Column {
            name: "Score",
            native: 4,
            width: 4,
            variable: None,
        },
    ];

    let records = values
        .iter()
        .map(|(id, score)| {
            let mut row = vec![2, 0];
            row.extend(id.to_le_bytes());
            row.extend(score.unwrap_or(0).to_le_bytes());
            row.push(1 | (u8::from(score.is_some()) << 1));
            row
        })
        .collect::<Vec<_>>();

    append(before, table, &columns, &records, 11)
}
