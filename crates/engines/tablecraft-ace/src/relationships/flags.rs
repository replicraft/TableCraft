use tablecraft_schema::{ReferentialAction, RelationshipRules};

const ONE_TO_ONE: u32 = 0x1;
const DONT_ENFORCE: u32 = 0x2;
const INHERITED: u32 = 0x4;
const CASCADE_UPDATE: u32 = 0x100;
const CASCADE_DELETE: u32 = 0x1000;
const CASCADE_NULL: u32 = 0x2000;
const LEFT_JOIN: u32 = 0x0100_0000;
const RIGHT_JOIN: u32 = 0x0200_0000;
const KNOWN_MASK: u32 = ONE_TO_ONE
    | DONT_ENFORCE
    | INHERITED
    | CASCADE_UPDATE
    | CASCADE_DELETE
    | CASCADE_NULL
    | LEFT_JOIN
    | RIGHT_JOIN;

pub(super) fn rules(flags: u32) -> Option<RelationshipRules> {
    if flags & !KNOWN_MASK != 0
        || flags & INHERITED != 0
        || flags & (LEFT_JOIN | RIGHT_JOIN) == (LEFT_JOIN | RIGHT_JOIN)
        || flags & (CASCADE_DELETE | CASCADE_NULL) == (CASCADE_DELETE | CASCADE_NULL)
    {
        return None;
    }
    Some(RelationshipRules {
        enforced: flags & DONT_ENFORCE == 0,
        one_to_one: flags & ONE_TO_ONE != 0,
        on_update: if flags & CASCADE_UPDATE != 0 {
            ReferentialAction::Cascade
        } else {
            ReferentialAction::Restrict
        },
        on_delete: if flags & CASCADE_DELETE != 0 {
            ReferentialAction::Cascade
        } else if flags & CASCADE_NULL != 0 {
            ReferentialAction::SetNull
        } else {
            ReferentialAction::Restrict
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declaration_flags_keep_unknown_inherited_and_conflicting_semantics_unqualified() {
        let declared = rules(0x1101).unwrap();
        assert!(declared.enforced && declared.one_to_one);
        assert_eq!(declared.on_update, ReferentialAction::Cascade);
        assert_eq!(declared.on_delete, ReferentialAction::Cascade);
        assert!(!rules(2).unwrap().enforced);
        assert_eq!(rules(0x2000).unwrap().on_delete, ReferentialAction::SetNull);
        for flags in [4, 0x8000_0000, 0x3000, 0x0300_0000] {
            assert!(rules(flags).is_none());
        }
    }
}
