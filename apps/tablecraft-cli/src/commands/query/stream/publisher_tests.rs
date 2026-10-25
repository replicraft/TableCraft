use super::*;
use std::io;

fn names() -> Vec<String> {
    vec!["N".into()]
}

#[test]
fn complete_batches_share_one_header_and_one_cumulative_allowance() {
    let mut output = Vec::new();
    let mut publisher = Publisher::new(&names(), 1, 6, &mut output).unwrap();
    publisher.append(&[Value::Int32(1)]).unwrap();
    publisher.append(&[Value::Int32(2)]).unwrap();
    assert!(publisher.finish().is_ok());
    assert_eq!(output, b"N\n1\n2\n");

    let mut output = Vec::new();
    let mut publisher = Publisher::new(&names(), 1, 5, &mut output).unwrap();
    assert!(publisher.append(&[Value::Int32(1)]).is_ok());
    assert!(matches!(
        publisher.append(&[Value::Int32(2)]),
        Err(ExecutionError::Operation(_))
    ));
    drop(publisher);
    assert_eq!(output, b"N\n1\n");
}

#[test]
fn pending_rows_do_not_publish_without_checked_finish() {
    let mut output = Vec::new();
    let mut publisher = Publisher::new(&names(), 2, 100, &mut output).unwrap();
    assert!(publisher.append(&[Value::Int32(1)]).is_ok());
    drop(publisher);
    assert!(output.is_empty());

    let publisher = Publisher::new(&names(), 2, 100, &mut output).unwrap();
    assert!(publisher.finish().is_ok());
    assert_eq!(output, b"N\n");
}

struct FailureWriter {
    bytes: Vec<u8>,
    fail_write: bool,
    fail_flush: bool,
}

impl Write for FailureWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.fail_write {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture write failure",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "fixture flush failure",
            ))
        } else {
            Ok(())
        }
    }
}

#[test]
fn write_and_flush_errors_remain_transport_failures() {
    for (fail_write, fail_flush, expected) in [
        (true, false, io::ErrorKind::BrokenPipe),
        (false, true, io::ErrorKind::PermissionDenied),
    ] {
        let mut writer = FailureWriter {
            bytes: Vec::new(),
            fail_write,
            fail_flush,
        };
        let mut publisher = Publisher::new(&names(), 1, 100, &mut writer).unwrap();
        let result = publisher.append(&[Value::Int32(1)]);
        assert!(matches!(result, Err(ExecutionError::Output(error)) if error.kind() == expected));
    }
}

#[test]
fn quoting_expansion_counts_toward_encoded_output_limit() {
    let mut output = Vec::new();
    let mut publisher = Publisher::new(&names(), 1, 12, &mut output).unwrap();
    let result = publisher.append(&[Value::Text("quote\"".into())]);
    assert!(matches!(result, Err(ExecutionError::Operation(_))));
    drop(publisher);
    assert!(output.is_empty());
}
