//! Validated named numeric output controls; no host preference lookup.
use super::Kind;
use crate::{EvalError, functions::conversions::text::NumberFormat};

const DEFAULT_PLACES: i32 = 2;
const REGIONAL_PLACES: i32 = -1;
const TRI_DEFAULT: i32 = -2;
const TRI_TRUE: i32 = -1;
const TRI_FALSE: i32 = 0;
/// Explicit per-call output ceiling, independent of stored Decimal scale.
pub(super) const MAX_PLACES: i32 = u8::MAX as i32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Options {
    pub places: u8,
    pub leading: bool,
    pub parentheses: bool,
    pub grouping: bool,
}
impl Options {
    pub fn parse(
        kind: Kind,
        count: usize,
        format: NumberFormat,
        convert: &mut impl FnMut(usize) -> Result<i32, EvalError>,
    ) -> Result<Self, EvalError> {
        let places = (count > 1)
            .then(|| convert(1))
            .transpose()?
            .unwrap_or(REGIONAL_PLACES);
        let places = if places == REGIONAL_PLACES {
            DEFAULT_PLACES
        } else {
            places
        };
        if !(0..=MAX_PLACES).contains(&places) {
            return Err(EvalError::Domain);
        }
        let leading = tristate(count, 2, true, convert)?;
        let parentheses = tristate(
            count,
            3,
            kind == Kind::Currency && format == NumberFormat::EnUs,
            convert,
        )?;
        let grouping = tristate(count, 4, format != NumberFormat::Invariant, convert)?;
        if grouping && format.group().is_none()
            || kind == Kind::Currency && format.currency().is_none()
        {
            return Err(EvalError::UnsupportedExpression);
        }
        Ok(Self {
            places: places as u8,
            leading,
            parentheses,
            grouping,
        })
    }
}
fn tristate(
    count: usize,
    index: usize,
    default: bool,
    convert: &mut impl FnMut(usize) -> Result<i32, EvalError>,
) -> Result<bool, EvalError> {
    match (count > index)
        .then(|| convert(index))
        .transpose()?
        .unwrap_or(TRI_DEFAULT)
    {
        TRI_DEFAULT => Ok(default),
        TRI_TRUE => Ok(true),
        TRI_FALSE => Ok(false),
        _ => Err(EvalError::Domain),
    }
}
