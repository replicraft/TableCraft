//! Boolean predicates and concrete type metadata have Scalar return signatures.
use tablecraft_expressions::{
    functions::inspection::{InspectionFunction, types::ValueInspectionFunction},
    numeric::OperandDeclaration,
};

pub(super) fn recognition(function: InspectionFunction) -> OperandDeclaration {
    match function {
        InspectionFunction::IsDate | InspectionFunction::IsNumeric => OperandDeclaration::Scalar,
    }
}

pub(super) fn value(function: ValueInspectionFunction) -> OperandDeclaration {
    match function {
        ValueInspectionFunction::VarType
        | ValueInspectionFunction::TypeName
        | ValueInspectionFunction::IsArray
        | ValueInspectionFunction::IsEmpty
        | ValueInspectionFunction::IsError => OperandDeclaration::Scalar,
    }
}
