//! Explicit declaration arithmetic probe; no SQL, native files or active content.
use clap::{Parser, ValueEnum};
use std::process::ExitCode;
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    EvalError,
    numeric::{
        Arithmetic, ArithmeticDeclarations, OperandDeclaration, declared_integral_arithmetic,
    },
};

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
    Divide,
    IntegerDivide,
    Modulo,
    Power,
}
impl From<Operation> for Arithmetic {
    fn from(value: Operation) -> Self {
        match value {
            Operation::Add => Self::Add,
            Operation::Subtract => Self::Subtract,
            Operation::Multiply => Self::Multiply,
            Operation::Divide => Self::Divide,
            Operation::IntegerDivide => Self::IntegerDivide,
            Operation::Modulo => Self::Modulo,
            Operation::Power => Self::Power,
        }
    }
}
#[derive(Parser)]
struct Arguments {
    #[arg(long, value_enum)]
    operator: Operation,
    #[arg(long, value_parser = operand)]
    left: Value,
    #[arg(long, value_parser = operand)]
    right: Value,
    #[arg(long, value_enum)]
    left_declaration: Declaration,
    #[arg(long, value_enum)]
    right_declaration: Declaration,
}
fn operand(text: &str) -> Result<Value, &'static str> {
    if text.len() > MAX_OPERAND_BYTES {
        return Err("operand limit exceeded");
    }
    if text == "null" {
        return Ok(Value::Null);
    }
    let (kind, text) = text
        .split_once(':')
        .ok_or("expected typed integral operand")?;
    match kind {
        "bool" => text
            .parse()
            .map(Value::Boolean)
            .map_err(|_| "invalid Boolean"),
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
    }
}
fn run(args: Arguments) -> Result<(), EvalError> {
    let declarations = ArithmeticDeclarations {
        left: args.left_declaration.into(),
        right: args.right_declaration.into(),
    };
    let result =
        declared_integral_arithmetic(args.operator.into(), &args.left, &args.right, declarations)?;
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
