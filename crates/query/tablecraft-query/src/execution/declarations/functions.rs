//! Reviewed function declarations, distinct from successful result carriers.
mod dates;
mod financial;
mod inspection;
use crate::plan::Expr;
use tablecraft_expressions::{
    functions::{math::MathFunction, text::TextFunction},
    numeric::OperandDeclaration,
};

pub(crate) fn result(expression: &Expr) -> Option<OperandDeclaration> {
    match expression {
        Expr::MathCall(function, _) => Some(math(*function)),
        Expr::FinancialCall(function, _) => Some(financial::scalar(*function)),
        Expr::CashFlowCall(function, ..) => Some(financial::cashflow(*function)),
        Expr::DateConvert(function, _) => Some(dates::conversion(*function)),
        Expr::ConstructorCall(function, _) => Some(dates::constructor(*function)),
        Expr::Clock(function) => Some(dates::clock(*function)),
        Expr::DateCall(function, _) => Some(dates::component(*function)),
        Expr::Inspect(function, _) => Some(inspection::recognition(*function)),
        Expr::ValueInspect(function, _) => Some(inspection::value(*function)),
        Expr::IsNull(..) | Expr::Exists(_) => Some(OperandDeclaration::Scalar),
        Expr::TextCall(
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
        | Expr::DateAdd(..)
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
