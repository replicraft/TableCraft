//! Working ordinary/index consumers share one typed cursor and byte ledger.
mod maps;
mod state;
#[cfg(test)]
mod tests;

pub(in crate::table) use maps::{Edit, audit_bytes, claim, clear_hint, current, merge};
pub(in crate::table) use state::State;
