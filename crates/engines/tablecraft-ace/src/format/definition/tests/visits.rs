use super::{budget, image, put32};
use crate::{
    Header, InspectError,
    format::{definition::read_with_pages, reader::Reader},
};
use std::{cell::Cell, io::Cursor};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn physical_visits_cover_first_and_continuation_pages_without_header_bytes_in_payload() {
    for length in [12, 4096, 4097, 8184, 8185, 261640] {
        let (file, expected) = image(length);
        let header = Header::parse(&file, file.len() as u64).unwrap();
        let mut source = Cursor::new(file);
        let mut allowance = budget();
        let mut visited = Vec::new();
        let actual = read_with_pages(
            &mut Reader {
                source: &mut source,
                header,
                budget: &mut allowance,
                cancelled: &|| false,
                reads: 0,
            },
            1,
            |number, reader| {
                reader.budget.charge_allocation(1, 4)?;
                visited.push(number);
                Ok(())
            },
        )
        .unwrap();
        let count = 1 + length.saturating_sub(4096).div_ceil(4088);
        assert_eq!(visited, (1..=count as u32).collect::<Vec<_>>());
        assert_eq!(&*actual, expected);
    }
}

#[test]
fn visitor_failures_bad_chains_limits_and_cancellation_return_no_definition() {
    let (healthy, _) = image(9000);
    let header = Header::parse(&healthy, healthy.len() as u64).unwrap();
    let mut source = Cursor::new(healthy.clone());
    let result = read_with_pages(
        &mut Reader {
            source: &mut source,
            header,
            budget: &mut budget(),
            cancelled: &|| false,
            reads: 0,
        },
        1,
        |number, _| {
            if number == 2 {
                return Err(InspectError::Unsupported);
            }
            Ok(())
        },
    );
    assert!(matches!(result, Err(InspectError::Unsupported)));

    let mut malformed = healthy.clone();
    put32(&mut malformed, 2 * 4096 + 4, 1);
    let mut source = Cursor::new(malformed);
    let result = read_with_pages(
        &mut Reader {
            source: &mut source,
            header,
            budget: &mut budget(),
            cancelled: &|| false,
            reads: 0,
        },
        1,
        |_, _| Ok(()),
    );
    assert!(matches!(result, Err(InspectError::Malformed)));

    let visited = Cell::new(0);
    let cancelled = || visited.get() != 0;
    let mut source = Cursor::new(healthy.clone());
    let result = read_with_pages(
        &mut Reader {
            source: &mut source,
            header,
            budget: &mut budget(),
            cancelled: &cancelled,
            reads: 0,
        },
        1,
        |_, _| {
            visited.set(visited.get() + 1);
            Ok(())
        },
    );
    assert!(matches!(result, Err(InspectError::Cancelled)));
    assert_eq!(visited.get(), 1);

    let mut source = Cursor::new(healthy);
    let result = read_with_pages(
        &mut Reader {
            source: &mut source,
            header,
            budget: &mut ResourceBudget::new(0, 0, 1000),
            cancelled: &|| false,
            reads: 0,
        },
        1,
        |_, _| Ok(()),
    );
    assert!(matches!(result, Err(InspectError::Limit(_))));
}
