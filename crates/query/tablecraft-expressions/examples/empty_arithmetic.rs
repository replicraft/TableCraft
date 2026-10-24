//! Host Empty arithmetic probe; no SQL Empty constructor or native file access.
use clap::{Parser, ValueEnum};
use std::process::ExitCode;
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::{
    EvalError,
    evaluation::arithmetic_operands,
    numeric::{
        Arithmetic, ArithmeticDeclarations, OperandDeclaration, declared_integral_arithmetic,
    },
};
use tablecraft_sql::select::BinaryOperator;

const MAX_OPERAND_BYTES: usize = 64;
const DOUBLE_HEX_DIGITS: usize = size_of::<u64>() * 2;
const INVALID_INPUT_STATUS: u8 = 2;

#[derive(Clone, Copy, ValueEnum)]
enum Declaration {
    Scalar,
    Variant,
}

impl From<Declaration> for OperandDeclaration {
    fn from(value: Declaration) -> Self {
        match value {
            Declaration::Scalar => Self::Scalar,
            Declaration::Variant => Self::Variant,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Operation {
    Add,
    Subtract,
    Multiply,
    IntegerDivide,
    Modulo,
}

impl Operation {
    fn operators(self) -> (BinaryOperator, Arithmetic) {
        match self {
            Self::Add => (BinaryOperator::Add, Arithmetic::Add),
            Self::Subtract => (BinaryOperator::Subtract, Arithmetic::Subtract),
            Self::Multiply => (BinaryOperator::Multiply, Arithmetic::Multiply),
            Self::IntegerDivide => (BinaryOperator::IntegerDivide, Arithmetic::IntegerDivide),
            Self::Modulo => (BinaryOperator::Modulo, Arithmetic::Modulo),
        }
    }
}

#[derive(Parser)]
struct Arguments {
    #[arg(long, value_enum)]
    operator: Operation,
    #[arg(long, value_parser = operand)]
    left: ExpressionValue,
    #[arg(long, value_parser = operand)]
    right: ExpressionValue,
    #[arg(long, value_enum)]
    left_declaration: Declaration,
    #[arg(long, value_enum)]
    right_declaration: Declaration,
}

fn operand(text: &str) -> Result<ExpressionValue, &'static str> {
    if text.len() > MAX_OPERAND_BYTES {
        return Err("operand limit exceeded");
    }
    if text == "empty" {
        return Ok(ExpressionValue::Empty);
    }
    if text == "null" {
        return Ok(ExpressionValue::Value(Value::Null));
    }

    let (kind, text) = text
        .split_once(':')
        .ok_or("expected typed integral operand")?;
    let value = match kind {
        "byte" => text.parse().map(Value::Byte).map_err(|_| "invalid Byte"),
        "i16" => text
            .parse()
            .map(Value::Int16)
            .map_err(|_| "invalid Integer"),
        "i32" => text.parse().map(Value::Int32).map_err(|_| "invalid Long"),
        "i64" => text
            .parse()
            .map(Value::Int64)
            .map_err(|_| "invalid LongLong"),
        _ => Err("unsupported operand type"),
    }?;
    Ok(ExpressionValue::Value(value))
}

fn run(arguments: Arguments) -> Result<(), EvalError> {
    let (operator, arithmetic) = arguments.operator.operators();
    let (left, right) = arithmetic_operands(operator, arguments.left, arguments.right)?;
    let result = declared_integral_arithmetic(
        arithmetic,
        &left,
        &right,
        ArithmeticDeclarations {
            left: arguments.left_declaration.into(),
            right: arguments.right_declaration.into(),
        },
    )?;
    match result {
        Value::Null => println!("NULL"),
        Value::Byte(value) => println!("Byte:{value}"),
        Value::Int16(value) => println!("Integer:{value}"),
        Value::Int32(value) => println!("Long:{value}"),
        Value::Int64(value) => println!("LongLong:{value}"),
        Value::Double(value) => println!(
            "Double:{:0width$x}",
            value.to_bits(),
            width = DOUBLE_HEX_DIGITS
        ),
        _ => return Err(EvalError::UnsupportedNumericCombination),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Arguments::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(INVALID_INPUT_STATUS)
        }
    }
}
