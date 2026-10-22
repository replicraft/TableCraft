//! Keep runtime domain strings within scalar expression/name scope.
use super::super::Binder;
use crate::{
    QueryError,
    plan::{Expr, Slot},
};
impl Binder<'_> {
    pub(super) fn domain_scalar(
        &mut self,
        expr: &Expr,
        slots: &[Slot],
        domain_calls: bool,
        depth: usize,
    ) -> Result<(), QueryError> {
        self.tick(depth)?;
        match expr {
            Expr::Field {
                depth: 0, index, ..
            } => {
                if slots
                    .iter()
                    .filter(|s| s.available)
                    .nth(*index)
                    .is_none_or(|s| s.member.is_some())
                {
                    return Err(QueryError::Unsupported);
                }
            }
            Expr::Literal(_) | Expr::Clock(_) => {}
            Expr::Unary(_, e)
            | Expr::IsNull(e, _)
            | Expr::Convert(_, e)
            | Expr::CVar(e)
            | Expr::CvErr(e)
            | Expr::Eval(e)
            | Expr::GuidFromString(e)
            | Expr::StringFromGuid(e)
            | Expr::Inspect(_, e)
            | Expr::ValueInspect(_, e)
            | Expr::DateCall(_, e)
            | Expr::DateConvert(_, e) => self.domain_scalar(e, slots, domain_calls, depth + 1)?,
            Expr::Binary(_, a, b) | Expr::ArrayBound(_, a, b) | Expr::Like(a, b, _) => {
                self.domain_scalar(a, slots, domain_calls, depth + 1)?;
                self.domain_scalar(b, slots, domain_calls, depth + 1)?;
            }
            Expr::IIf(a, b, c) | Expr::Between(a, b, c, _) | Expr::DateAdd(a, b, c) => {
                for e in [a, b, c] {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            Expr::DateDiff(a, b, c, options) => {
                for e in [a, b, c].into_iter().chain(options.iter().flatten()) {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            Expr::DatePart(a, b, options) => {
                for e in [a, b].into_iter().chain(options.iter().flatten()) {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            Expr::DomainCall(_, args) if domain_calls => {
                for e in args {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            Expr::TextCall(_, args)
            | Expr::FinancialCall(_, args)
            | Expr::Array(args)
            | Expr::CashFlowCall(_, args, _)
            | Expr::MathCall(_, args)
            | Expr::ConstructorCall(_, args)
            | Expr::ConditionalCall(_, args) => {
                for e in args {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            Expr::Nz(e, other) | Expr::Weekday(e, other) => {
                self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                if let Some(e) = other {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            Expr::InList(e, list, _) => {
                self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                for e in list {
                    self.domain_scalar(e, slots, domain_calls, depth + 1)?;
                }
            }
            _ => return Err(QueryError::Unsupported),
        }
        Ok(())
    }
}
