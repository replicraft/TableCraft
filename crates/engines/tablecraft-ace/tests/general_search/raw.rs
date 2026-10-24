use super::*;
use tablecraft_ace::{compare_general_utf16, find_general_utf16};

fn raw_find(
    version: u8,
    text: &[u16],
    needle: &[u16],
    search: GeneralTextSearch,
) -> Option<Range<usize>> {
    find_general_utf16(order(version), text, needle, search, &mut budget(), &|| {
        false
    })
    .unwrap()
}

fn boundaries(text: &[u16]) -> Vec<usize> {
    let mut offsets = vec![0];
    for value in char::decode_utf16(text.iter().copied()) {
        let width = value.map_or(1, char::len_utf16);
        offsets.push(offsets.last().unwrap() + width);
    }
    offsets
}

fn brute_raw(
    version: u8,
    text: &[u16],
    needle: &[u16],
    search: GeneralTextSearch,
) -> Option<Range<usize>> {
    let offsets = boundaries(text);
    let mut starts = offsets.clone();
    if matches!(search, GeneralTextSearch::ReverseThrough(_)) {
        starts.reverse();
    }
    for start in starts {
        let allowed = match search {
            GeneralTextSearch::ForwardFrom(first) => start >= first,
            GeneralTextSearch::ReverseThrough(last) => start <= last.min(text.len()),
        };
        if !allowed {
            continue;
        }
        if needle.is_empty() {
            return Some(start..start);
        }
        for end in offsets.iter().copied().filter(|end| *end > start) {
            if matches!(search, GeneralTextSearch::ReverseThrough(last) if end > last) {
                continue;
            }
            if compare_general_utf16(
                order(version),
                &text[start..end],
                needle,
                &mut budget(),
                &|| false,
            )
            .unwrap()
                == Ordering::Equal
            {
                return Some(start..end);
            }
        }
    }
    None
}

#[test]
fn raw_search_retains_isolated_units_and_decoded_pair_boundaries() {
    for version in [0, 1] {
        assert_eq!(
            raw_find(
                version,
                &[0xd800],
                &[0xd800],
                GeneralTextSearch::ForwardFrom(0)
            ),
            Some(0..1)
        );
        assert_eq!(
            raw_find(
                version,
                &[0xdc00],
                &[0xdc00],
                GeneralTextSearch::ReverseThrough(1)
            ),
            Some(0..1)
        );
        assert_eq!(
            raw_find(
                version,
                &[0xd83d, 0xde00],
                &[0xd83d],
                GeneralTextSearch::ReverseThrough(1)
            ),
            None
        );
        assert_eq!(
            raw_find(
                version,
                &[0xd83d, 0xde00],
                &[],
                GeneralTextSearch::ForwardFrom(1)
            ),
            Some(2..2)
        );
        assert_eq!(
            raw_find(
                version,
                &[0xd800, 65],
                &[97],
                GeneralTextSearch::ForwardFrom(0)
            ),
            Some(if version == 0 { 0..2 } else { 1..2 })
        );
    }
}

#[test]
fn raw_search_refuses_unknown_orders_allocations_work_and_each_cancellation_checkpoint() {
    assert!(matches!(
        find_general_utf16(
            NativeSortOrder::from_native_bytes([9, 4, 1, 1]),
            &[65],
            &[65],
            GeneralTextSearch::ForwardFrom(0),
            &mut budget(),
            &|| false
        ),
        Err(InspectError::Unsupported)
    ));
    for mut limited in [
        ResourceBudget::new(0, 0, 10000),
        ResourceBudget::new(65536, 0, 0),
    ] {
        assert!(matches!(
            find_general_utf16(
                order(1),
                &[0xd800, 65],
                &[65],
                GeneralTextSearch::ForwardFrom(0),
                &mut limited,
                &|| false
            ),
            Err(InspectError::Limit(_))
        ));
    }
    for search in [
        GeneralTextSearch::ForwardFrom(0),
        GeneralTextSearch::ReverseThrough(3),
    ] {
        let visits = Cell::new(0);
        find_general_utf16(
            order(1),
            &[0xd800, 223, 65],
            &[115, 115],
            search,
            &mut budget(),
            &|| {
                visits.set(visits.get() + 1);
                false
            },
        )
        .unwrap();
        let count = visits.get();
        for stop in 1..=count {
            visits.set(0);
            assert!(matches!(
                find_general_utf16(
                    order(1),
                    &[0xd800, 223, 65],
                    &[115, 115],
                    search,
                    &mut budget(),
                    &|| {
                        visits.set(visits.get() + 1);
                        visits.get() >= stop
                    }
                ),
                Err(InspectError::Cancelled)
            ));
        }
    }
}

fn raw_units(maximum: usize) -> impl Strategy<Value = Vec<u16>> {
    prop::collection::vec(
        prop::sample::select(vec![
            0, 32, 65, 97, 98, 115, 223, 233, 769, 0x30ab, 0x30fc, 0xd800, 0xdc00, 0xd83d, 0xde00,
            0xffff,
        ]),
        0..=maximum,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn raw_pruning_agrees_with_unpruned_unit_span_enumeration(
        text in raw_units(7),
        needle in raw_units(3),
    ) {
        for version in [0, 1] {
            for bound in 0..=text.len() + 1 {
                for search in [GeneralTextSearch::ForwardFrom(bound), GeneralTextSearch::ReverseThrough(bound)] {
                    prop_assert_eq!(
                        raw_find(version, &text, &needle, search),
                        brute_raw(version, &text, &needle, search),
                    );
                }
            }
        }
    }

    #[test]
    fn valid_raw_and_decoded_inputs_have_identical_spans(
        text in strings(6),
        needle in strings(3),
    ) {
        let text_units = text.encode_utf16().collect::<Vec<_>>();
        let needle_units = needle.encode_utf16().collect::<Vec<_>>();
        for version in [0, 1] {
            for bound in 0..=text_units.len() + 1 {
                for search in [GeneralTextSearch::ForwardFrom(bound), GeneralTextSearch::ReverseThrough(bound)] {
                    prop_assert_eq!(
                        raw_find(version, &text_units, &needle_units, search),
                        find(version, &text, &needle, search),
                    );
                }
            }
        }
    }
}
