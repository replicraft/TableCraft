//! Reviewed function declarations, distinct from successful result carriers.
use crate::plan::Expr;
use tablecraft_expressions::{
    functions::{
        dates::{DateFunction, parsing::DateConversion},
        inspection::types::ValueInspectionFunction,
        math::MathFunction,
        text::TextFunction,
    },
    numeric::OperandDeclaration,
};

pub(crate) fn result(expression: &Expr) -> Option<OperandDeclaration> {
    match expression {
        Expr::MathCall(function, _) => Some(math(*function)),
        Expr::DateConvert(DateConversion::CDate, _)
        | Expr::ValueInspect(ValueInspectionFunction::VarType, _)
        | Expr::TextCall(
            TextFunction::Asc | TextFunction::AscB | TextFunction::AscW | TextFunction::InStrRev,
            _,
        ) => Some(OperandDeclaration::Scalar),
        Expr::TextCall(
            TextFunction::Len
            | TextFunction::LenB
            | TextFunction::StrComp
            | TextFunction::InStr
            | TextFunction::InStrB,
            _,
        )
        | Expr::DomainCall(..)
        | Expr::DateConvert(DateConversion::CVDate, _)
        | Expr::DateCall(
            DateFunction::Year
            | DateFunction::Month
            | DateFunction::Day
            | DateFunction::Hour
            | DateFunction::Minute
            | DateFunction::Second,
            _,
        )
        | Expr::DatePart(..)
        | Expr::DateDiff(..)
        | Expr::Weekday(..) => Some(OperandDeclaration::Variant),
        _ => None,
    }
}

fn math(function: MathFunction) -> OperandDeclaration {
    match function {
        MathFunction::Abs
        | MathFunction::Sgn
        | MathFunction::Int
        | MathFunction::Fix
        | MathFunction::Round => OperandDeclaration::Variant,
        MathFunction::Sqr
        | MathFunction::Exp
        | MathFunction::Log
        | MathFunction::Sin
        | MathFunction::Cos
        | MathFunction::Tan
        | MathFunction::Atn
        | MathFunction::Rnd
        | MathFunction::Rgb
        | MathFunction::QbColor => OperandDeclaration::Scalar,
    }
}
