use super::OfflineTarget;
use crate::IoError;
use std::{io::ErrorKind, path::Path};
use tablecraft_core::limits::ResourceBudget;

#[test]
fn unsupported_unix_target_refuses_before_path_or_budget_use() {
    let mut budget = ResourceBudget::new(0, 0, 0);
    assert!(matches!(
        OfflineTarget::open(Path::new("must-not-open/target"), &mut budget, &|| panic!(
            "unsupported platform polled the host"
        )),
        Err(IoError::Io(ErrorKind::Unsupported))
    ));
    assert_eq!(budget, ResourceBudget::new(0, 0, 0));
}
