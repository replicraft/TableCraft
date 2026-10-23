//! Date return declarations remain independent of the DATE carrier.
use tablecraft_expressions::{
    functions::dates::{
        DateConstructor, DateFunction, clock::ClockFunction, parsing::DateConversion,
    },
    numeric::OperandDeclaration,
};

pub(super) fn conversion(function: DateConversion) -> OperandDeclaration {
    match function {
        DateConversion::CDate => OperandDeclaration::Scalar,
        DateConversion::CVDate | DateConversion::DateValue | DateConversion::TimeValue => {
            OperandDeclaration::Variant
        }
    }
}

pub(super) fn constructor(function: DateConstructor) -> OperandDeclaration {
    match function {
        DateConstructor::DateSerial | DateConstructor::TimeSerial => OperandDeclaration::Variant,
    }
}

pub(super) fn component(function: DateFunction) -> OperandDeclaration {
    match function {
        DateFunction::Year
        | DateFunction::Month
        | DateFunction::Day
        | DateFunction::Hour
        | DateFunction::Minute
        | DateFunction::Second => OperandDeclaration::Variant,
    }
}

pub(super) fn clock(function: ClockFunction) -> OperandDeclaration {
    match function {
        ClockFunction::Date | ClockFunction::Time | ClockFunction::Now => {
            OperandDeclaration::Variant
        }
        ClockFunction::Timer => OperandDeclaration::Scalar,
    }
}
