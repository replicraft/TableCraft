//! Resource recovery consumes exactly one native owner and retains other variants.
use super::{AccessInput, Input};
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom},
    rc::Rc,
};
use tablecraft_core::limits::ResourceBudget;

struct Resource {
    identity: usize,
    bytes: Cursor<Vec<u8>>,
    drops: Rc<Cell<usize>>,
}

impl Drop for Resource {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

impl Read for Resource {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.bytes.read(bytes)
    }
}

impl Seek for Resource {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn native_selection_returns_the_original_resource_once() {
    let drops = Rc::new(Cell::new(0));
    let resource = Resource {
        identity: 71,
        bytes: Cursor::new(b"native input resource".to_vec()),
        drops: Rc::clone(&drops),
    };
    let input =
        AccessInput::open(resource, &mut ResourceBudget::new(1024, 0, 1024), &|| false).unwrap();
    let mut resource = match input.into_native() {
        Ok(resource) => resource,
        Err(_) => panic!("native resource was not recovered"),
    };
    assert_eq!(resource.identity, 71);
    assert_eq!(drops.get(), 0);
    let mut bytes = Vec::new();
    resource.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"native input resource");
    drop(resource);
    assert_eq!(drops.get(), 1);
}

#[test]
fn captured_and_spooled_inputs_remain_owned_and_readable_on_refusal() {
    let captured: AccessInput<Resource, Resource> = AccessInput {
        inner: Input::Captured(Cursor::new(b"captured payload".to_vec())),
    };
    let mut captured = match captured.into_native() {
        Ok(_) => panic!("captured payload became a native resource"),
        Err(input) => input,
    };
    assert!(captured.is_deployment());
    let mut bytes = Vec::new();
    captured.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"captured payload");

    let drops = Rc::new(Cell::new(0));
    let spooled: AccessInput<Resource, Resource> = AccessInput {
        inner: Input::Deployment(Resource {
            identity: 93,
            bytes: Cursor::new(b"spooled payload".to_vec()),
            drops: Rc::clone(&drops),
        }),
    };
    let mut spooled = match spooled.into_native() {
        Ok(_) => panic!("spooled payload became a native resource"),
        Err(input) => input,
    };
    assert_eq!(drops.get(), 0);
    assert!(spooled.is_deployment());
    bytes.clear();
    spooled.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"spooled payload");
    drop(spooled);
    assert_eq!(drops.get(), 1);
}
