use super::{cases, model};
use crate::{BeforeEventIntents, MacroProgramProfile, NativeImage, RowDelete, read_table};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;

#[test]
fn qualify_before_guard_delete_ordinals_against_original_records_before_preparation() {
    let case = &cases::CASES[4];
    for code in [5, 6] {
        let (bytes, xml, _) = model::source(code, case.event, case.body);
        let scope = CatalogScope::new(7);
        let mut budget = ResourceBudget::new(32 * 1024 * 1024, 16_384, 1_000_000);
        let image =
            NativeImage::capture(&mut Cursor::new(&bytes), scope, &mut budget, &|| false).unwrap();
        let table =
            read_table(&mut image.reader(), scope, "Items", &mut budget, &|| false).unwrap();
        assert_eq!(table.rows().len(), 3);
        let extra = image
            .inspect_table_extra(scope.object_id(10), &mut budget, &|| false)
            .unwrap();
        let property = &extra.groups()[0].properties().unwrap()[0];
        assert_eq!(extra.groups()[0].kind(), 3);
        assert_eq!(property.name(), "StoredGuard");
        assert_eq!(property.text(), Some(xml.as_str()));
        let program = property
            .inspect_macro_program(MacroProgramProfile::DocumentedDesktop, &mut budget, &|| {
                false
            })
            .unwrap();
        assert_eq!(
            program.envelopes().definitions()[0].event(),
            Some("BeforeDelete")
        );

        let ordinals = [1_usize, 0];
        let intent = BeforeEventIntents::DeleteOrdinals(&ordinals);
        let BeforeEventIntents::DeleteOrdinals(selected) = intent else {
            panic!("ordinal intent changed kind");
        };
        assert_eq!(selected, [1, 0]);
        let page = &bytes[12 * 4096..13 * 4096];
        for ordinal in 0..2 {
            let slot = 14 + 2 * ordinal;
            let word = u16::from_le_bytes([page[slot], page[slot + 1]]);
            assert_eq!(word & 0xf000, 0);
            let start = usize::from(word & 0x0fff);
            let exact = model::original_record(ordinal);
            assert_eq!(exact.len(), 11);
            assert_eq!(&page[start..start + 11], exact);
            let inverse = RowDelete::new(ordinal, exact.clone());
            assert_eq!(inverse.row(), ordinal);
            assert_eq!(inverse.expected_bytes(), exact);
        }
        assert!(image.as_bytes() == bytes);
    }
}
