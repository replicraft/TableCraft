//! Reviewed function declarations, distinct from successful result carriers.
use crate::plan::Expr;
use tablecraft_expressions::{
    functions::{
        dates::DateFunction, inspection::types::ValueInspectionFunction, math::MathFunction,
        text::TextFunction,
    },
    numeric::OperandDeclaration,
};

pub(crate) fn result(expression: &Expr) -> Option<OperandDeclaration> {
    match expression {
        Expr::ValueInspect(ValueInspectionFunction::VarType, _)
        | Expr::TextCall(
            TextFunction::Len
            | TextFunction::LenB
            | TextFunction::Asc
            | TextFunction::AscB
            | TextFunction::AscW,
            _,
        ) => Some(OperandDeclaration::Scalar),
        Expr::TextCall(TextFunction::StrComp | TextFunction::InStr | TextFunction::InStrB, _)
        | Expr::MathCall(MathFunction::Sgn, _)
        | Expr::DateCall(DateFunction::Year, _) => Some(OperandDeclaration::Variant),
        _ => None,
    }
}
