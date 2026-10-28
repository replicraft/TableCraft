use super::*;
use std::{cell::RefCell, rc::Rc};

struct Input {
    events: Rc<RefCell<Vec<&'static str>>>,
}

impl Read for Input {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.events.borrow_mut().push("read");
        bytes[0] = b'\n';
        Ok(1)
    }
}

struct Output {
    events: Rc<RefCell<Vec<&'static str>>>,
    flush_name: &'static str,
    fail_flush: bool,
    bytes: Vec<u8>,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.events.borrow_mut().push("write");
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.events.borrow_mut().push(self.flush_name);
        if self.fail_flush {
            return Err(io::Error::other("publication flush failed"));
        }
        Ok(())
    }
}

fn streams() -> (Input, Output, Output) {
    let events = Rc::new(RefCell::new(Vec::new()));
    (
        Input {
            events: Rc::clone(&events),
        },
        Output {
            events: Rc::clone(&events),
            flush_name: "stdout flush",
            fail_flush: false,
            bytes: Vec::new(),
        },
        Output {
            events,
            flush_name: "stderr flush",
            fail_flush: false,
            bytes: Vec::new(),
        },
    )
}

#[test]
fn interactive_wait_flushes_publication_and_prompt_before_reading() {
    let (mut input, mut output, mut errors) = streams();
    output
        .bytes
        .extend_from_slice(b"already published result\n");
    errors
        .bytes
        .extend_from_slice(b"already published diagnostic\n");
    wait_if_interactive(true, true, true, &mut input, &mut output, &mut errors).unwrap();

    assert_eq!(
        *input.events.borrow(),
        [
            "stdout flush",
            "stderr flush",
            "write",
            "stderr flush",
            "read"
        ]
    );
    assert_eq!(output.bytes, b"already published result\n");
    assert_eq!(
        errors.bytes,
        b"already published diagnostic\nPress Enter to close.\n"
    );
}

#[test]
fn absent_request_or_either_redirected_stream_performs_no_wait_io() {
    for (requested, input_terminal, output_terminal) in [
        (false, true, true),
        (true, false, true),
        (true, true, false),
    ] {
        let (mut input, mut output, mut errors) = streams();
        wait_if_interactive(
            requested,
            input_terminal,
            output_terminal,
            &mut input,
            &mut output,
            &mut errors,
        )
        .unwrap();
        assert!(input.events.borrow().is_empty());
        assert!(output.bytes.is_empty());
        assert!(errors.bytes.is_empty());
    }
}

#[test]
fn failed_publication_flush_does_not_prompt_or_read() {
    let (mut input, mut output, mut errors) = streams();
    output.fail_flush = true;
    assert!(wait_if_interactive(true, true, true, &mut input, &mut output, &mut errors).is_err());
    assert_eq!(*input.events.borrow(), ["stdout flush"]);
    assert!(errors.bytes.is_empty());

    let (mut input, mut output, mut errors) = streams();
    errors.fail_flush = true;
    assert!(wait_if_interactive(true, true, true, &mut input, &mut output, &mut errors).is_err());
    assert_eq!(*input.events.borrow(), ["stdout flush", "stderr flush"]);
    assert!(errors.bytes.is_empty());
}
