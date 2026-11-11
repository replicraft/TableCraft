//! Literal fixed records, including native GUID byte order and Boolean value bits.
use super::{Column, append};

const COLUMNS: [Column; 6] = [
    Column {
        name: "Token",
        native: 15,
        width: 16,
        variable: None,
    },
    Column {
        name: "OccurredAt",
        native: 8,
        width: 8,
        variable: None,
    },
    Column {
        name: "Ratio",
        native: 6,
        width: 4,
        variable: None,
    },
    Column {
        name: "Reading",
        native: 7,
        width: 8,
        variable: None,
    },
    Column {
        name: "Code",
        native: 2,
        width: 1,
        variable: None,
    },
    Column {
        name: "Enabled",
        native: 1,
        width: 0,
        variable: None,
    },
];

/// Independent original carriers; nullable payload positions remain zero-filled.
#[derive(Clone, Copy)]
pub struct Row {
    /// Canonical textual GUID byte order, not native struct order.
    pub guid: Option<[u8; 16]>,
    /// Exact finite legacy Automation DATE bits.
    pub date: Option<u64>,
    /// Exact finite IEEE Single bits, including signed zero and subnormal values.
    pub single: Option<u32>,
    /// Exact finite IEEE Double bits.
    pub double: Option<u64>,
    /// Unsigned Byte payload.
    pub byte: Option<u8>,
    /// Boolean value, never a nullable carrier.
    pub enabled: bool,
}

/// Repeated literal patterns cross several actual DATA pages and retain row order.
pub fn rows() -> Vec<Row> {
    (0..360)
        .map(|index| Row {
            guid: (index % 3 != 0).then_some([
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff,
            ]),
            date: (index % 4 != 0).then_some(if index % 2 == 0 {
                0x4004_0000_0000_0000 // OLE 2.5.
            } else {
                0xbff4_0000_0000_0000 // OLE -1.25.
            }),
            single: (index % 5 != 0).then_some(match index % 3 {
                0 => 0x8000_0000, // Negative zero.
                1 => 0x0000_0001, // Smallest positive subnormal.
                _ => 0xc020_0000, // -2.5.
            }),
            double: (index % 6 != 0).then_some(match index % 3 {
                0 => 0x8000_0000_0000_0000,
                1 => 0x0000_0000_0000_0001,
                _ => 0x3ff3_c0ca_428c_59dd,
            }),
            byte: (index % 7 != 0).then_some((index % 256) as u8),
            enabled: index % 2 != 0,
        })
        .collect()
}

/// Full independent AFTER over the supplied retained original, without codecs.
pub fn after(before: &[u8], table: &str, rows: &[Row]) -> Vec<u8> {
    let records = rows
        .iter()
        .map(|value| {
            // Count6, fixed payload37, one mask byte. Boolean owns no payload byte.
            let mut record = vec![0; 40];
            record[..2].copy_from_slice(&[6, 0]);
            if let Some(guid) = value.guid {
                record[2..18].copy_from_slice(&[
                    guid[3], guid[2], guid[1], guid[0], guid[5], guid[4], guid[7], guid[6],
                    guid[8], guid[9], guid[10], guid[11], guid[12], guid[13], guid[14], guid[15],
                ]);
                record[39] |= 1;
            }
            if let Some(bits) = value.date {
                record[18..26].copy_from_slice(&bits.to_le_bytes());
                record[39] |= 2;
            }
            if let Some(bits) = value.single {
                record[26..30].copy_from_slice(&bits.to_le_bytes());
                record[39] |= 4;
            }
            if let Some(bits) = value.double {
                record[30..38].copy_from_slice(&bits.to_le_bytes());
                record[39] |= 8;
            }
            if let Some(byte) = value.byte {
                record[38] = byte;
                record[39] |= 16;
            }
            if value.enabled {
                record[39] |= 32;
            }
            record
        })
        .collect::<Vec<_>>();
    append(before, table, &COLUMNS, &records, 40)
}
