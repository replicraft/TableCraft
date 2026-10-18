//! Native scalar type tags and storage widths used by the qualified reader.
//! These are file-format facts, separate from shared ScalarType discriminants.
pub(crate) const BOOLEAN: u8 = 0x01;
pub(crate) const BYTE: u8 = 0x02;
pub(crate) const INT16: u8 = 0x03;
pub(crate) const INT32: u8 = 0x04;
pub(crate) const CURRENCY: u8 = 0x05;
pub(crate) const SINGLE: u8 = 0x06;
pub(crate) const DOUBLE: u8 = 0x07;
pub(crate) const DATE_TIME: u8 = 0x08;
pub(crate) const DATE_TIME_BYTES: usize = size_of::<f64>();
pub(crate) const SHORT_BINARY: u8 = 0x09;
pub(crate) const SHORT_TEXT: u8 = 0x0a;
pub(crate) const LONG_TEXT: u8 = 0x0c;
pub(crate) const BOOLEAN_BYTES: usize = 0;
pub(crate) const BYTE_BYTES: usize = size_of::<u8>();
pub(crate) const INT16_BYTES: usize = size_of::<i16>();
pub(crate) const INT32_BYTES: usize = size_of::<i32>();
pub(crate) const CURRENCY_BYTES: usize = size_of::<i64>();
pub(crate) const SINGLE_BYTES: usize = size_of::<f32>();
pub(crate) const DOUBLE_BYTES: usize = size_of::<f64>();
pub(crate) const MAX_SHORT_VALUE_BYTES: usize = 510;
