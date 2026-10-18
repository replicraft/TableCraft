use super::*;
pub(super) fn parse(p: &mut Parser<'_>, inline: bool) -> Result<Constraint, ParseError> {
    let constraint_name = if p.parse_keyword(W::CONSTRAINT) {
        Some(identifier(p)?)
    } else if inline {
        None
    } else {
        return Err(ParseError::at(
            p,
            K::Syntax,
            "expected named table constraint",
        ));
    };
    let kind = if p.parse_keyword(W::PRIMARY) {
        expect(p, W::KEY)?;
        ConstraintKind::PrimaryKey(if inline { Vec::new() } else { names(p)? })
    } else if p.parse_keyword(W::UNIQUE) {
        ConstraintKind::Unique(if inline { Vec::new() } else { names(p)? })
    } else if p.parse_keyword(W::NOT) {
        expect(p, W::NULL)?;
        ConstraintKind::NotNull(if inline { Vec::new() } else { names(p)? })
    } else {
        let (fields, no_index) = if inline {
            (Vec::new(), false)
        } else {
            expect(p, W::FOREIGN)?;
            expect(p, W::KEY)?;
            let no_index = if p.parse_keyword(W::NO) {
                expect(p, W::INDEX)?;
                true
            } else {
                false
            };
            (names(p)?, no_index)
        };
        expect(p, W::REFERENCES)?;
        let table = identifier(p)?;
        let foreign_fields = if p.peek_token().token == Token::LParen {
            names(p)?
        } else {
            Vec::new()
        };
        let mut on_update = None;
        let mut on_delete = None;
        while p.parse_keyword(W::ON) {
            let slot = if p.parse_keyword(W::UPDATE) {
                &mut on_update
            } else if p.parse_keyword(W::DELETE) {
                &mut on_delete
            } else {
                return Err(ParseError::at(p, K::Syntax, "expected relationship event"));
            };
            if slot.is_some() {
                return Err(ParseError::at(p, K::Syntax, "duplicate relationship event"));
            }
            *slot = Some(if p.parse_keyword(W::CASCADE) {
                ReferentialAction::Cascade
            } else {
                expect(p, W::SET)?;
                expect(p, W::NULL)?;
                ReferentialAction::SetNull
            });
        }
        ConstraintKind::ForeignKey(Reference {
            fields,
            no_index,
            table,
            foreign_fields,
            on_update,
            on_delete,
        })
    };
    Ok(Constraint {
        name: constraint_name,
        kind,
    })
}
