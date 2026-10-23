//! Pure declaration categories shared by binding and execution; no evaluation.
pub(crate) mod functions;
use tablecraft_expressions::numeric::OperandDeclaration;

pub(crate) fn combined(
    left: Option<OperandDeclaration>,
    right: Option<OperandDeclaration>,
) -> Option<OperandDeclaration> {
    match (left, right) {
        (Some(OperandDeclaration::Variant), _) | (_, Some(OperandDeclaration::Variant)) => {
            Some(OperandDeclaration::Variant)
        }
        (Some(OperandDeclaration::Scalar), Some(OperandDeclaration::Scalar)) => {
            Some(OperandDeclaration::Scalar)
        }
        _ => None,
    }
}
