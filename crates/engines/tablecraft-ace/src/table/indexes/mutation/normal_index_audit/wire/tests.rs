//! Generated numeric-order laws, independent of the staging key encoder.
use super::{Row, keys};
use proptest::prelude::*;
use tablecraft_core::limits::ResourceBudget;

proptest! {
    #[test]
    fn wire_order_matches_signed_tuple_order_and_original_row_suffix(
        values in any::<[[i32; 2]; 3]>(),
        directions in any::<[bool; 2]>(),
    ) {
        let population = values
            .iter()
            .enumerate()
            .map(|(ordinal, values)| Row {
                reference: 0x800 + ordinal as u32,
                before: values.to_vec(),
                after: values.to_vec(),
            })
            .collect::<Vec<_>>();
        let columns = [(0, directions[0]), (1, directions[1])];
        let mut budget = ResourceBudget::new(1_000_000, 100, 1_000_000);
        let encoded = keys(&population, &columns, true, &mut budget, &|| false).unwrap();

        let mut expected = [0, 1, 2];
        expected.sort_by(|&left, &right| {
            for (column, descending) in directions.into_iter().enumerate() {
                let order = values[left][column].cmp(&values[right][column]);
                let order = if descending {
                    order.reverse()
                } else {
                    order
                };
                if !order.is_eq() {
                    return order;
                }
            }
            left.cmp(&right)
        });

        let actual = encoded
            .iter()
            .map(|key| {
                u32::from_be_bytes(key[key.len() - size_of::<u32>()..].try_into().unwrap())
            })
            .collect::<Vec<_>>();
        prop_assert_eq!(actual, expected.map(|ordinal| 0x800 + ordinal as u32));
    }
}
