//! SOURCE-only gates over the shared independent growing-record model.
pub(in crate::table::mutation) mod model;

use self::model::{Destination, case, record_after, record_before};
use super::{model as original, qualify};
use original::{Family, PAGE};

#[test]
fn qualify_growing_mixed_sources_and_complete_expected_before_preview() {
    use crate::NativeImage;
    use crate::table::mutation::{
        gates, page_update::roots, rows::RowAssignment, variable_split::metadata::Metadata,
    };
    use std::io::Cursor;
    use tablecraft_model::CatalogScope;

    struct Selected;
    impl RowAssignment for Selected {
        fn row(&self) -> usize {
            3
        }
    }

    for code in [5, 6] {
        for family in [Family::Legacy, Family::Modern, Family::Binary] {
            for descending in [false, true] {
                for destination in [
                    Destination::OriginalPage,
                    Destination::OtherOwned,
                    Destination::EndOfFile,
                ] {
                    let case = case(code, family, descending, destination);
                    qualify(&case.before, &case.rows, case.family);
                    qualify(&case.expected, &case.final_rows, case.family);

                    for (bytes, final_image) in [(&case.before, false), (&case.expected, true)] {
                        for (definition, tail) in [(2, 398), (7, 449), (10, 663)] {
                            let at = definition * PAGE + tail;
                            assert_eq!(&bytes[at..at + 2], &[0xff, 0xff]);
                        }
                        let mut budget = super::budget();
                        let image = NativeImage::capture(
                            &mut Cursor::new(bytes),
                            CatalogScope::new(1),
                            &mut budget,
                            &|| false,
                        )
                        .unwrap();
                        let checked =
                            gates::before_slot(&image, "Items", &mut budget, &|| false).unwrap();
                        Metadata::capture_for_insert(
                            &image,
                            checked.schema(),
                            checked.indexes(),
                            &mut budget,
                            &|| false,
                        )
                        .unwrap();
                        let selected = [Selected];
                        let captured = roots::capture(
                            &image,
                            checked.schema(),
                            &selected,
                            &mut budget,
                            &|| false,
                        )
                        .unwrap();
                        assert_eq!(captured.selected.len(), 1);
                        assert_eq!(captured.selected[0].reference, (12 << 8) | 3);
                        if final_image {
                            assert_eq!(
                                captured.selected[0].final_record.data,
                                record_after(family)
                            );
                            assert_eq!(
                                captured.selected[0].final_record.offset / PAGE,
                                (case.target >> 8) as usize
                            );
                        } else {
                            assert_eq!(captured.selected[0].final_record.data, record_before());
                        }
                    }
                    assert_eq!(
                        case.target >> 8,
                        match destination {
                            Destination::OriginalPage => 12,
                            Destination::OtherOwned | Destination::EndOfFile => 17,
                        }
                    );
                }
            }
        }
    }
}
