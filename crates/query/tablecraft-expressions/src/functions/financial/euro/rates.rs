//! Irrevocable national units per euro; exact integer/scale facts, no live FX data.
use super::DECIMAL_RADIX;
use crate::EvalError;
use tablecraft_core::values::Value;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Currency {
    pub code: &'static str,
    coefficient: u32,
    scale: i32,
    pub places: i32,
}

const CURRENCIES: [Currency; 12] = [
    Currency::new("ATS", 137603, 4, 2),
    Currency::new("BEF", 403399, 4, 0),
    Currency::new("DEM", 195583, 5, 2),
    Currency::new("ESP", 166386, 3, 0),
    Currency::new("FIM", 594573, 5, 2),
    Currency::new("FRF", 655957, 5, 2),
    Currency::new("IEP", 787564, 6, 2),
    Currency::new("ITL", 193627, 2, 0),
    Currency::new("LUF", 403399, 4, 0),
    Currency::new("NLG", 220371, 5, 2),
    Currency::new("PTE", 200482, 3, 0),
    Currency::new("EUR", 1, 0, 2),
];

impl Currency {
    const fn new(code: &'static str, coefficient: u32, scale: i32, places: i32) -> Self {
        Self {
            code,
            coefficient,
            scale,
            places,
        }
    }

    pub fn parse(
        value: &Value,
        check: &mut impl FnMut() -> Result<(), EvalError>,
    ) -> Result<Self, EvalError> {
        let code = match value {
            Value::Text(code) => code,
            Value::Null => return Err(EvalError::InvalidNullConversion),
            _ => return Err(EvalError::TypeMismatch),
        };
        if code.len() != "EUR".len() || !code.is_ascii() {
            return Err(EvalError::Domain);
        }
        for currency in CURRENCIES {
            check()?;
            if code.eq_ignore_ascii_case(currency.code) {
                return Ok(currency);
            }
        }
        Err(EvalError::Domain)
    }

    pub fn rate(self) -> f64 {
        f64::from(self.coefficient) / DECIMAL_RADIX.powi(self.scale)
    }

    pub fn is_euro(self) -> bool {
        self.code == "EUR"
    }
}
