use super::*;

#[test]
fn preflight_refuses_unsafe_geometry_before_requesting_or_writing_storage() {
    let original = fixture::cabinet(&database(5), false);
    for (offset, replacement) in [
        (26, 2u16.to_le_bytes().to_vec()),
        (28, 2u16.to_le_bytes().to_vec()),
        (30, 1u16.to_le_bytes().to_vec()),
        (34, 1u16.to_le_bytes().to_vec()),
        (48, 1u32.to_le_bytes().to_vec()),
        (60, b"../".to_vec()),
        (42, 2u16.to_le_bytes().to_vec()),
        (44, u32::MAX.to_le_bytes().to_vec()),
    ] {
        let mut package = original.clone();
        package[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let result = AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
            Cursor::new(package),
            || panic!("unqualified input must not request storage"),
            &mut budget(),
            &|| false,
        );
        assert!(result.is_err(), "offset {offset}");
    }
    for length in [4, 35, 43, 60, original.len() - 1] {
        assert!(
            AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
                Cursor::new(&original[..length]),
                || panic!("truncated input"),
                &mut budget(),
                &|| false,
            )
            .is_err()
        );
    }
}

#[test]
fn nonempty_factory_failure_and_post_factory_cancellation_never_publish_or_overwrite() {
    let package = fixture::cabinet(&database(6), true);
    let dropped = Rc::new(Cell::new(false));
    let written = Rc::new(Cell::new(0));
    let mut spool = Spool::fresh(dropped.clone(), written.clone());
    spool
        .inner
        .get_mut()
        .extend_from_slice(b"existing host bytes");
    assert!(matches!(
        AccessInput::open_spooled(Cursor::new(&package), || Ok(spool), &mut budget(), &|| {
            false
        },),
        Err(AccessInputError::UnsupportedPackage)
    ));
    assert!(dropped.get());
    assert_eq!(written.get(), 0);

    assert!(matches!(
        AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
            Cursor::new(&package),
            || Err(InspectError::Io(io::ErrorKind::PermissionDenied)),
            &mut budget(),
            &|| false,
        ),
        Err(AccessInputError::Native(InspectError::Io(
            io::ErrorKind::PermissionDenied
        )))
    ));

    let cancelled = Cell::new(false);
    let dropped = Rc::new(Cell::new(false));
    let spool = Spool::fresh(dropped.clone(), written.clone());
    assert!(matches!(
        AccessInput::open_spooled(
            Cursor::new(&package),
            || {
                cancelled.set(true);
                Ok(spool)
            },
            &mut budget(),
            &|| cancelled.get(),
        ),
        Err(AccessInputError::Native(InspectError::Cancelled))
    ));
    assert!(dropped.get());
    assert_eq!(written.get(), 0);
}

#[test]
fn write_zero_host_failure_seek_misreport_and_flush_failure_retain_typed_errors() {
    let package = fixture::cabinet(&database(5), true);
    for case in 0..4 {
        let dropped = Rc::new(Cell::new(false));
        let mut spool = Spool::fresh(dropped.clone(), Rc::new(Cell::new(0)));
        let expected = match case {
            0 => {
                spool.maximum_write = 0;
                InspectError::Io(io::ErrorKind::WriteZero)
            }
            1 => {
                spool.failure = Some(io::ErrorKind::PermissionDenied);
                InspectError::Io(io::ErrorKind::PermissionDenied)
            }
            2 => {
                spool.seek_mismatch = true;
                InspectError::Malformed
            }
            _ => {
                spool.flush_failure = Some(io::ErrorKind::Other);
                InspectError::Io(io::ErrorKind::Other)
            }
        };
        let result =
            AccessInput::open_spooled(Cursor::new(&package), || Ok(spool), &mut budget(), &|| {
                false
            });
        let Err(error) = result else {
            panic!("failed host operation must refuse")
        };
        assert_eq!(error, AccessInputError::Native(expected));
        assert!(!error.to_string().contains("private"));
        assert!(dropped.get());
    }
}

#[test]
fn late_checksum_failure_discards_prior_spool_prefix_and_signature_overlay_is_inert() {
    let expected = database(5);
    for compressed in [false, true] {
        let original = fixture::cabinet(&expected, compressed);
        let mut package = original.clone();
        let first_block = u32::from_le_bytes(package[36..40].try_into().unwrap()) as usize;
        let first_size = usize::from(u16::from_le_bytes(
            package[first_block + 4..first_block + 6]
                .try_into()
                .unwrap(),
        ));
        let last_block = first_block + 8 + first_size;
        package[last_block..last_block + 4].copy_from_slice(&0xdead_beefu32.to_le_bytes());
        let dropped = Rc::new(Cell::new(false));
        let written = Rc::new(Cell::new(0));
        let spool = Spool::fresh(dropped.clone(), written.clone());
        assert!(matches!(
            AccessInput::open_spooled(Cursor::new(&package), || Ok(spool), &mut budget(), &|| {
                false
            },),
            Err(AccessInputError::MalformedPackage)
        ));
        assert_eq!(written.get(), 32768);
        assert!(dropped.get());

        let mut package = original;
        package.extend_from_slice(b"unverified signature overlay, not database bytes");
        let mut source = AccessInput::open_spooled(
            Cursor::new(&package),
            || Ok(Cursor::new(Vec::new())),
            &mut budget(),
            &|| false,
        )
        .unwrap();
        let mut actual = Vec::new();
        source.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, expected);
    }
}

#[test]
fn cancellation_and_cumulative_limits_are_observed_without_storage_publication() {
    let package = fixture::cabinet(&database(6), false);
    let mut seen = 0;
    for stop in [1, 20, 100, 200, 400] {
        let calls = Cell::new(0);
        let dropped = Rc::new(Cell::new(false));
        let made = Cell::new(false);
        let result = AccessInput::open_spooled(
            Cursor::new(&package),
            || {
                made.set(true);
                Ok(Spool::fresh(dropped.clone(), Rc::new(Cell::new(0))))
            },
            &mut budget(),
            &|| {
                calls.set(calls.get() + 1);
                calls.get() == stop
            },
        );
        if let Err(AccessInputError::Native(InspectError::Cancelled)) = result {
            seen += 1;
            assert_eq!(dropped.get(), made.get());
        } else {
            assert!(calls.get() < stop);
        }
    }
    assert!(seen >= 3);
    for allowance in [
        ResourceBudget::new(1, 0, 1_000_000),
        ResourceBudget::new(64 * 1024 * 1024, 0, 1),
    ] {
        let result = AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
            Cursor::new(&package),
            || Ok(Cursor::new(Vec::new())),
            &mut { allowance },
            &|| false,
        );
        assert!(matches!(
            result,
            Err(AccessInputError::Native(InspectError::Limit(_)))
        ));
    }

    let cancelled = Rc::new(Cell::new(false));
    let dropped = Rc::new(Cell::new(false));
    let mut spool = Spool::fresh(dropped.clone(), Rc::new(Cell::new(0)));
    spool.cancel_on_flush = Some(cancelled.clone());
    assert!(matches!(
        AccessInput::open_spooled(Cursor::new(&package), || Ok(spool), &mut budget(), &|| {
            cancelled.get()
        },),
        Err(AccessInputError::Native(InspectError::Cancelled))
    ));
    assert!(dropped.get());
}

struct FailingSource {
    inner: Cursor<Vec<u8>>,
    interrupted: bool,
}

impl Read for FailingSource {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.len() > 100 {
            return Err(io::Error::new(
                if self.interrupted {
                    io::ErrorKind::Interrupted
                } else {
                    io::ErrorKind::PermissionDenied
                },
                "private read detail",
            ));
        }
        self.inner.read(bytes)
    }
}

impl Seek for FailingSource {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.inner.seek(position)
    }
}

#[test]
fn decoder_source_failures_and_repeated_interrupts_keep_original_typed_error() {
    for interrupted in [false, true] {
        let package = fixture::cabinet(&database(5), false);
        let source = FailingSource {
            inner: Cursor::new(package),
            interrupted,
        };
        let dropped = Rc::new(Cell::new(false));
        let spool = Spool::fresh(dropped.clone(), Rc::new(Cell::new(0)));
        let result = AccessInput::open_spooled(
            source,
            || Ok(spool),
            &mut ResourceBudget::new(64 * 1024 * 1024, 0, 1000),
            &|| false,
        );
        let Err(error) = result else {
            panic!("decoder callback failure must refuse")
        };
        if interrupted {
            assert!(matches!(
                error,
                AccessInputError::Native(InspectError::Limit(_))
            ));
        } else {
            assert_eq!(
                error,
                AccessInputError::Native(InspectError::Io(io::ErrorKind::PermissionDenied))
            );
        }
        assert!(dropped.get());
        assert!(!error.to_string().contains("private"));
    }
}

#[test]
fn native_header_validation_keeps_complete_spool_and_factory_failure_order() {
    for (offset, expected) in [
        (0, AccessInputError::Native(InspectError::Malformed)),
        (20, AccessInputError::Native(InspectError::Unsupported)),
    ] {
        let mut native = database(5);
        native[offset] = 0xff;
        let package = fixture::cabinet(&native, true);
        let called = Cell::new(false);
        let dropped = Rc::new(Cell::new(false));
        let written = Rc::new(Cell::new(0));
        let spool = Spool::fresh(dropped.clone(), written.clone());
        let result = AccessInput::open_spooled(
            Cursor::new(&package),
            || {
                called.set(true);
                Ok(spool)
            },
            &mut budget(),
            &|| false,
        );
        let Err(error) = result else {
            panic!("invalid native payload must not be published")
        };
        assert_eq!(error, expected);
        assert!(called.get());
        assert!(dropped.get());
        assert_eq!(written.get(), native.len());

        let called = Cell::new(false);
        let mut exhausted = ResourceBudget::new(0, 1000, 1_000_000);
        let result = AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
            Cursor::new(&package),
            || {
                called.set(true);
                Ok(Cursor::new(Vec::new()))
            },
            &mut exhausted,
            &|| false,
        );
        assert!(matches!(
            result,
            Err(AccessInputError::Native(InspectError::Limit(_)))
        ));
        assert!(!called.get());
    }
}

#[test]
fn header_errors_precede_final_rewind_host_cancel_and_work_failures() {
    let valid = database(5);
    let mut invalid = valid.clone();
    invalid[0] = 0xff;
    let valid_package = fixture::cabinet(&valid, false);
    let invalid_package = fixture::cabinet(&invalid, false);

    let mut healthy = AccessInput::open_spooled(
        Cursor::new(&valid_package),
        || Ok(Cursor::new(Vec::new())),
        &mut budget(),
        &|| false,
    )
    .unwrap();
    let mut actual = Vec::new();
    healthy.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, valid);

    for header_valid in [false, true] {
        for cancel in [false, true] {
            let package = if header_valid {
                &valid_package
            } else {
                &invalid_package
            };
            let dropped = Rc::new(Cell::new(false));
            let written = Rc::new(Cell::new(0));
            let cancelled = Rc::new(Cell::new(false));
            let mut spool = Spool::fresh(dropped.clone(), written.clone());
            if cancel {
                spool.cancel_on_final_rewind = Some(cancelled.clone());
            } else {
                spool.final_rewind_failure = Some(io::ErrorKind::PermissionDenied);
            }
            let result = AccessInput::open_spooled(
                Cursor::new(package),
                || Ok(spool),
                &mut budget(),
                &|| cancelled.get(),
            );
            let Err(error) = result else {
                panic!("invalid header or final rewind must refuse publication")
            };
            let expected = if !header_valid {
                InspectError::Malformed
            } else if cancel {
                InspectError::Cancelled
            } else {
                InspectError::Io(io::ErrorKind::PermissionDenied)
            };
            assert_eq!(error, AccessInputError::Native(expected));
            assert_eq!(cancelled.get(), header_valid && cancel);
            assert!(dropped.get());
            assert_eq!(written.get(), valid.len());
        }
    }

    // Measure only the work preceding a native-header refusal. That same
    // allowance reaches validation for both equally sized stored CABs, but
    // leaves no step for the valid header's deferred final rewind.
    let mut measured = budget();
    let initial_steps = measured.remaining(tablecraft_core::limits::Resource::Steps);
    let result = AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
        Cursor::new(&invalid_package),
        || Ok(Cursor::new(Vec::new())),
        &mut measured,
        &|| false,
    );
    assert!(matches!(
        result,
        Err(AccessInputError::Native(InspectError::Malformed))
    ));
    let steps = initial_steps - measured.remaining(tablecraft_core::limits::Resource::Steps);
    for (package, malformed) in [(&invalid_package, true), (&valid_package, false)] {
        let mut allowance = ResourceBudget::new(64 * 1024 * 1024, 1000, steps);
        let result = AccessInput::<_, Cursor<Vec<u8>>>::open_spooled(
            Cursor::new(package),
            || Ok(Cursor::new(Vec::new())),
            &mut allowance,
            &|| false,
        );
        if malformed {
            assert!(matches!(
                result,
                Err(AccessInputError::Native(InspectError::Malformed))
            ));
        } else {
            assert!(matches!(
                result,
                Err(AccessInputError::Native(InspectError::Limit(limit)))
                    if limit.resource == tablecraft_core::limits::Resource::Steps
            ));
        }
    }
}

#[test]
fn captured_native_validation_precedes_terminal_cancellation_in_both_routes() {
    fn open_small(
        package: &[u8],
        hybrid: bool,
        allowance: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), AccessInputError> {
        if hybrid {
            AccessInput::<_, Cursor<Vec<u8>>>::open_with_spool(
                Cursor::new(package),
                || panic!("small captured input must not request host storage"),
                allowance,
                cancelled,
            )
            .map(|_| ())
        } else {
            AccessInput::open(Cursor::new(package), allowance, cancelled).map(|_| ())
        }
    }

    let native = database(5);
    let valid = fixture::cabinet(&native, false);
    for hybrid in [false, true] {
        let calls = Cell::new(0);
        open_small(&valid, hybrid, &mut budget(), &|| {
            calls.set(calls.get() + 1);
            false
        })
        .unwrap();
        let terminal = calls.get();
        for (payload, expected) in [
            (native.clone(), InspectError::Cancelled),
            (
                {
                    let mut invalid = native.clone();
                    invalid[0] = 0xff;
                    invalid
                },
                InspectError::Malformed,
            ),
            (
                {
                    let mut unknown = native.clone();
                    unknown[20] = 0xff;
                    unknown
                },
                InspectError::Unsupported,
            ),
        ] {
            let package = fixture::cabinet(&payload, false);
            let current = Cell::new(0);
            let result = open_small(&package, hybrid, &mut budget(), &|| {
                current.set(current.get() + 1);
                current.get() == terminal
            });
            assert_eq!(result.unwrap_err(), AccessInputError::Native(expected));
        }

        // A structurally valid Jet header is outside the deployment ACE family.
        // Its family refusal also precedes the terminal cancellation callback.
        let mut jet = native.clone();
        jet[4..19].copy_from_slice(b"Standard Jet DB");
        jet[20] = 1;
        let package = fixture::cabinet(&jet, false);
        let current = Cell::new(0);
        let result = open_small(&package, hybrid, &mut budget(), &|| {
            current.set(current.get() + 1);
            current.get() == terminal
        });
        assert_eq!(result.unwrap_err(), AccessInputError::UnsupportedPackage);
    }
}
