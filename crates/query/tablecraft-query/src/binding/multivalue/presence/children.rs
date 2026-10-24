//! Scalar children only; nested read plans own independent collection frames.
use crate::plan::Expr;

const MAX_FIXED_CHILDREN: usize = 5;

pub(super) fn children(expr: &Expr) -> impl Iterator<Item = &Expr> {
    let (fixed, many): ([Option<&Expr>; MAX_FIXED_CHILDREN], &[Expr]) = match expr {
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::CollectionEmpty(value, _)
        | Expr::DateCall(_, value)
        | Expr::Convert(_, value)
        | Expr::CVar(value)
        | Expr::CvErr(value)
        | Expr::Eval(value)
        | Expr::GuidFromString(value)
        | Expr::StringFromGuid(value)
        | Expr::ErrorMessage(value)
        | Expr::ErrorMessageString(value)
        | Expr::Inspect(_, value)
        | Expr::ValueInspect(_, value)
        | Expr::DateConvert(_, value)
        | Expr::InQuery(value, _, _)
        | Expr::Quantified(value, _, _, _) => ([Some(value), None, None, None, None], &[]),
        Expr::Binary(_, left, right)
        | Expr::ArrayBound(_, left, right)
        | Expr::Like(left, right, _) => ([Some(left), Some(right), None, None, None], &[]),
        Expr::IIf(first, second, third)
        | Expr::DateAdd(first, second, third)
        | Expr::Between(first, second, third, _) => {
            ([Some(first), Some(second), Some(third), None, None], &[])
        }
        Expr::DateDiff(first, second, third, [fourth, fifth]) => (
            [
                Some(first),
                Some(second),
                Some(third),
                fourth.as_deref(),
                fifth.as_deref(),
            ],
            &[],
        ),
        Expr::DatePart(first, second, [third, fourth]) => (
            [
                Some(first),
                Some(second),
                third.as_deref(),
                fourth.as_deref(),
                None,
            ],
            &[],
        ),
        Expr::Nz(value, other) | Expr::Weekday(value, other) => {
            ([Some(value), other.as_deref(), None, None, None], &[])
        }
        Expr::InList(value, values, _) => {
            ([Some(value), None, None, None, None], values.as_slice())
        }
        Expr::Aggregate(_, value) => ([value.as_deref(), None, None, None, None], &[]),
        Expr::DomainCall(_, values)
        | Expr::TextCall(_, values)
        | Expr::MathCall(_, values)
        | Expr::FinancialCall(_, values)
        | Expr::CashFlowCall(_, values, _)
        | Expr::ConstructorCall(_, values)
        | Expr::ConditionalCall(_, values)
        | Expr::Array(values) => ([None, None, None, None, None], values.as_slice()),
        Expr::Literal(_)
        | Expr::DateLiteral(..)
        | Expr::Field { .. }
        | Expr::Output(..)
        | Expr::CollectionItem { .. }
        | Expr::ParentPresence { .. }
        | Expr::Parameter(_)
        | Expr::Clock(_)
        | Expr::CurrentUser
        | Expr::Subquery(_)
        | Expr::Exists(_) => ([None, None, None, None, None], &[]),
    };
    fixed.into_iter().flatten().chain(many.iter())
}

pub(super) fn children_mut(expr: &mut Expr) -> impl Iterator<Item = &mut Expr> {
    let (fixed, many): ([Option<&mut Expr>; MAX_FIXED_CHILDREN], &mut [Expr]) = match expr {
        Expr::Unary(_, value)
        | Expr::IsNull(value, _)
        | Expr::CollectionEmpty(value, _)
        | Expr::DateCall(_, value)
        | Expr::Convert(_, value)
        | Expr::CVar(value)
        | Expr::CvErr(value)
        | Expr::Eval(value)
        | Expr::GuidFromString(value)
        | Expr::StringFromGuid(value)
        | Expr::ErrorMessage(value)
        | Expr::ErrorMessageString(value)
        | Expr::Inspect(_, value)
        | Expr::ValueInspect(_, value)
        | Expr::DateConvert(_, value)
        | Expr::InQuery(value, _, _)
        | Expr::Quantified(value, _, _, _) => ([Some(value), None, None, None, None], &mut []),
        Expr::Binary(_, left, right)
        | Expr::ArrayBound(_, left, right)
        | Expr::Like(left, right, _) => ([Some(left), Some(right), None, None, None], &mut []),
        Expr::IIf(first, second, third)
        | Expr::DateAdd(first, second, third)
        | Expr::Between(first, second, third, _) => (
            [Some(first), Some(second), Some(third), None, None],
            &mut [],
        ),
        Expr::DateDiff(first, second, third, [fourth, fifth]) => (
            [
                Some(first),
                Some(second),
                Some(third),
                fourth.as_deref_mut(),
                fifth.as_deref_mut(),
            ],
            &mut [],
        ),
        Expr::DatePart(first, second, [third, fourth]) => (
            [
                Some(first),
                Some(second),
                third.as_deref_mut(),
                fourth.as_deref_mut(),
                None,
            ],
            &mut [],
        ),
        Expr::Nz(value, other) | Expr::Weekday(value, other) => (
            [Some(value), other.as_deref_mut(), None, None, None],
            &mut [],
        ),
        Expr::InList(value, values, _) => {
            ([Some(value), None, None, None, None], values.as_mut_slice())
        }
        Expr::Aggregate(_, value) => ([value.as_deref_mut(), None, None, None, None], &mut []),
        Expr::DomainCall(_, values)
        | Expr::TextCall(_, values)
        | Expr::MathCall(_, values)
        | Expr::FinancialCall(_, values)
        | Expr::CashFlowCall(_, values, _)
        | Expr::ConstructorCall(_, values)
        | Expr::ConditionalCall(_, values)
        | Expr::Array(values) => ([None, None, None, None, None], values.as_mut_slice()),
        Expr::Literal(_)
        | Expr::DateLiteral(..)
        | Expr::Field { .. }
        | Expr::Output(..)
        | Expr::CollectionItem { .. }
        | Expr::ParentPresence { .. }
        | Expr::Parameter(_)
        | Expr::Clock(_)
        | Expr::CurrentUser
        | Expr::Subquery(_)
        | Expr::Exists(_) => ([None, None, None, None, None], &mut []),
    };
    fixed.into_iter().flatten().chain(many.iter_mut())
}
