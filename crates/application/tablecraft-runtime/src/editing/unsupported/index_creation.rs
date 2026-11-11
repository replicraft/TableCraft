//! Unsupported hosts retain public read witnesses without minting save authority.
use crate::editing::IndexCreatedReopen;
use std::sync::Arc;

pub(in crate::editing) struct State {
    pub(in crate::editing) id: u64,
    pub(in crate::editing) reopen: Arc<IndexCreatedReopen>,
}
