use super::*;

struct Literal {
    offset: usize,
    before: [u8; 4],
    after: [u8; 4],
    length: usize,
    mask: bool,
    structural: bool,
}

impl BytePatch for Literal {
    fn offset(&self) -> usize {
        self.offset
    }

    fn before(&self) -> &[u8] {
        &self.before[..self.length]
    }

    fn after(&self) -> &[u8] {
        &self.after[..self.length]
    }

    fn is_mask(&self) -> bool {
        self.mask
    }

    fn requires_structural_check(&self) -> bool {
        self.structural
    }
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(1_000_000, 0, 100_000)
}

fn bit(offset: usize, before: u8, after: u8) -> Literal {
    Literal {
        offset,
        before: [before, 0, 0, 0],
        after: [after, 0, 0, 0],
        length: 1,
        mask: true,
        structural: true,
    }
}

#[test]
fn tombstone_structural_authority_survives_owned_span_conversion() {
    let source = Literal {
        offset: 28,
        before: [0x24, 0x01, 0, 0],
        after: [0x24, 0xc1, 0, 0],
        length: 2,
        mask: false,
        structural: true,
    };
    let mut spans = Vec::new();
    push(&mut spans, &source, &mut budget(), &|| false).unwrap();
    coalesce(&mut spans, &mut budget(), &|| false).unwrap();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].offset(), 28);
    assert_eq!(spans[0].before(), &[0x24, 0x01]);
    assert_eq!(spans[0].after(), &[0x24, 0xc1]);
    assert!(!spans[0].is_mask());
    assert!(spans[0].requires_structural_check());
}

#[test]
fn disjoint_null_bits_merge_against_the_same_original_byte_in_either_order() {
    for reverse in [false, true] {
        let mut sources = [bit(25, 0x0f, 0x0d), bit(25, 0x0f, 0x0b)];
        if reverse {
            sources.reverse();
        }
        let mut spans = Vec::new();
        for source in &sources {
            push(&mut spans, source, &mut budget(), &|| false).unwrap();
        }
        coalesce(&mut spans, &mut budget(), &|| false).unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].before(), &[0x0f]);
        assert_eq!(spans[0].after(), &[0x09]);
        assert!(spans[0].is_mask());
        assert!(spans[0].requires_structural_check());
    }
}

#[test]
fn same_bit_conflicting_originals_and_overlapping_payloads_refuse() {
    let payload = |offset| Literal {
        offset,
        before: [1, 2, 0, 0],
        after: [3, 4, 0, 0],
        length: 2,
        mask: false,
        structural: false,
    };
    for sources in [
        vec![bit(25, 0x0f, 0x0d), bit(25, 0x0f, 0x0d)],
        vec![bit(25, 0x0f, 0x0d), bit(25, 0x07, 0x03)],
        vec![payload(25), payload(26)],
        vec![payload(25), payload(25)],
    ] {
        let mut spans = Vec::new();
        for source in sources {
            push(&mut spans, &source, &mut budget(), &|| false).unwrap();
        }
        assert_eq!(
            coalesce(&mut spans, &mut budget(), &|| false),
            Err(EditError::Preservation)
        );
    }
}

#[test]
fn allocation_and_cancellation_fail_before_exposing_partial_spans() {
    let mut spans = Vec::new();
    assert!(matches!(
        push(
            &mut spans,
            &bit(25, 3, 1),
            &mut ResourceBudget::new(0, 0, 100),
            &|| false
        ),
        Err(EditError::Inspect(InspectError::Limit(_)))
    ));
    assert!(spans.is_empty());
    assert!(matches!(
        push(&mut spans, &bit(25, 3, 1), &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert!(spans.is_empty());
    push(&mut spans, &bit(25, 3, 1), &mut budget(), &|| false).unwrap();
    assert!(matches!(
        coalesce(&mut spans, &mut budget(), &|| true),
        Err(EditError::Inspect(InspectError::Cancelled))
    ));
    assert_eq!(spans[0].before(), &[3]);
    assert_eq!(spans[0].after(), &[1]);
}
