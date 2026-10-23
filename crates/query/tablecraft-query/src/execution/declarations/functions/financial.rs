//! Reviewed Double returns; argument declarations do not change the signature.
use tablecraft_expressions::{
    functions::financial::{FinancialFunction, cashflows::CashFlowFunction},
    numeric::OperandDeclaration,
};

pub(super) fn scalar(function: FinancialFunction) -> OperandDeclaration {
    match function {
        FinancialFunction::Pmt
        | FinancialFunction::PV
        | FinancialFunction::FV
        | FinancialFunction::NPer
        | FinancialFunction::IPmt
        | FinancialFunction::PPmt
        | FinancialFunction::SLN
        | FinancialFunction::SYD
        | FinancialFunction::DDB
        | FinancialFunction::Rate
        | FinancialFunction::EuroConvert => OperandDeclaration::Scalar,
    }
}

pub(super) fn cashflow(function: CashFlowFunction) -> OperandDeclaration {
    match function {
        CashFlowFunction::NPV | CashFlowFunction::IRR | CashFlowFunction::MIRR => {
            OperandDeclaration::Scalar
        }
    }
}
