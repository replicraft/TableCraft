mod identity;
mod table;

pub use identity::RowHandle;
pub use table::Snapshot;

#[cfg(test)]
mod tests;
