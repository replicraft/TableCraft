//! Public Agile page-decoder experiment; no production parallelism is enabled.
//! Generated files pad a real blank database with deterministic, unallocated
//! pages. This measures AES page throughput, not native table qualification.
use clap::{Parser, ValueEnum};
use rayon::{ThreadPool, ThreadPoolBuilder, prelude::*};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    hint::black_box,
    io::{Cursor, Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
use tablecraft_ace::{
    AgilePageDecoder, EncryptionRewrite, InspectError, create_blank_ace16_database,
    read_agile_encryption, rewrite_encryption,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use zeroize::Zeroizing;

const PAGE_BYTES: usize = 4096;
const MIB: usize = 1024 * 1024;
const MAX_INPUT_BYTES: usize = 64 * MIB;
const AES_BLOCK_BYTES: usize = size_of::<aes::cipher::Block<aes::Aes256>>();
const PAGE_CHECK_STEPS: u64 = 2;
const PAGE_STEPS: u64 = (PAGE_BYTES / AES_BLOCK_BYTES) as u64 + PAGE_CHECK_STEPS;
const SETUP_BYTES: u64 = 16 * MAX_INPUT_BYTES as u64;
const SETUP_STEPS: u64 = 100_000_000;
const BENCHMARK_PASSWORD: &str = "TableCraft-public-benchmark";

type PageResult = Result<(), InspectError>;
type Decoded = Zeroizing<Vec<u8>>;

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Verify,
    Serial,
    Rayon,
}

#[derive(Parser)]
struct Arguments {
    #[arg(long, value_enum, default_value = "verify")]
    mode: Mode,
    #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u32).range(1..=100_000))]
    iterations: u32,
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u32).range(1..=64), conflicts_with = "input")]
    mib: u32,
    #[arg(long)]
    input: Option<PathBuf>,
    /// Create a fresh fixture and exit. Never replaces an existing file.
    #[arg(long, conflicts_with = "input")]
    generate: Option<PathBuf>,
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u8).range(2..=16))]
    threads: u8,
    /// Generated fixtures use this public test credential. Do not use private data.
    #[arg(long, default_value = BENCHMARK_PASSWORD)]
    password: String,
}

#[derive(Debug)]
struct Failure {
    page: Option<u32>,
    cause: InspectError,
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.page {
            Some(page) => write!(formatter, "page {page}: {}", self.cause),
            None => write!(formatter, "coordinator: {}", self.cause),
        }
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

#[derive(Clone, Copy)]
struct PagePolicy {
    bytes: u64,
    steps: u64,
    invalid_ordinals: bool,
    cancelled_pages: bool,
}

impl PagePolicy {
    fn normal() -> Self {
        Self {
            bytes: PAGE_BYTES as u64,
            steps: PAGE_STEPS,
            invalid_ordinals: false,
            cancelled_pages: false,
        }
    }
}

fn setup_budget() -> ResourceBudget {
    ResourceBudget::new(SETUP_BYTES, 1_000_000, SETUP_STEPS)
}

fn allowance(length: usize, policy: PagePolicy) -> ResourceBudget {
    let pages = (length / PAGE_BYTES - 1) as u64;
    // Prepay the output clone, every worker's bounded scratch allowance and the
    // ordered status buffer before allocation or scheduling any page callbacks.
    let bytes = length as u64 + pages * (policy.bytes + size_of::<PageResult>() as u64);
    ResourceBudget::new(bytes, 0, pages * policy.steps)
}

fn decode(
    encrypted: &[u8],
    decoder: &AgilePageDecoder,
    pool: Option<&ThreadPool>,
    coordinator: &mut ResourceBudget,
    policy: PagePolicy,
    checks: Option<&AtomicUsize>,
) -> Result<Decoded, Failure> {
    let pages = encrypted.len() / PAGE_BYTES - 1;
    let prepay = || -> Result<(), InspectError> {
        coordinator.charge(Resource::Bytes, encrypted.len() as u64)?;
        coordinator
            .charge_allocation(pages as u64, policy.bytes + size_of::<PageResult>() as u64)?;
        coordinator.charge(Resource::Steps, pages as u64 * policy.steps)?;
        Ok(())
    };
    let mut prepay = prepay;
    prepay().map_err(|cause| Failure { page: None, cause })?;

    let mut decoded = Zeroizing::new(encrypted.to_vec());
    let transform = |(index, page): (usize, &mut [u8])| {
        let number = index as u32 + 1;
        let ordinal = if policy.invalid_ordinals && matches!(number, 2 | 5) {
            0
        } else {
            number
        };
        let mut local = ResourceBudget::new(policy.bytes, 0, policy.steps);
        decoder.decode_page(ordinal, page, &mut local, &|| {
            if let Some(checks) = checks {
                checks.fetch_add(1, Ordering::Relaxed);
            }
            policy.cancelled_pages && matches!(number, 2 | 5)
        })
    };
    // Indexed collection retains original page order. Workers do not race to
    // publish an error, cancel peers or expose a partially decoded buffer.
    let outcomes: Vec<PageResult> = match pool {
        Some(pool) => pool.install(|| {
            decoded[PAGE_BYTES..]
                .par_chunks_exact_mut(PAGE_BYTES)
                .enumerate()
                .map(transform)
                .collect()
        }),
        None => decoded[PAGE_BYTES..]
            .as_chunks_mut::<PAGE_BYTES>()
            .0
            .iter_mut()
            .enumerate()
            .map(|(index, page)| transform((index, page.as_mut_slice())))
            .collect(),
    };
    for (index, outcome) in outcomes.into_iter().enumerate() {
        outcome.map_err(|cause| Failure {
            page: Some(index as u32 + 1),
            cause,
        })?;
    }
    Ok(decoded)
}

fn fixture(mib: u32, password: &str) -> Result<Decoded, Box<dyn Error>> {
    let blank = create_blank_ace16_database(&mut setup_budget(), &|| false)?;
    let mut plain = Zeroizing::new(blank.as_bytes().to_vec());
    let original_length = plain.len();
    plain.resize(mib as usize * MIB, 0);
    // Distinct ordinal-dependent plaintext catches accidental repeated-page or
    // wrong-IV processing. Padding remains outside allocated database objects.
    for (index, page) in plain[original_length..]
        .as_chunks_mut::<PAGE_BYTES>()
        .0
        .iter_mut()
        .enumerate()
    {
        let number = (original_length / PAGE_BYTES + index) as u32;
        for (offset, byte) in page.iter_mut().enumerate() {
            *byte = number.wrapping_mul(37).wrapping_add(offset as u32) as u8;
        }
    }
    let encrypted = rewrite_encryption(
        &mut Cursor::new(plain.as_slice()),
        EncryptionRewrite::Add {
            password: None,
            new_password: password,
        },
        &mut setup_budget(),
        &|| false,
    )?;
    Ok(Zeroizing::new(encrypted.as_bytes().to_vec()))
}

fn verify(
    encrypted: &[u8],
    decoder: &AgilePageDecoder,
    pool: &ThreadPool,
    generated: bool,
) -> Result<(), Box<dyn Error>> {
    let policy = PagePolicy::normal();
    let checks = AtomicUsize::new(0);
    let serial = decode(
        encrypted,
        decoder,
        None,
        &mut allowance(encrypted.len(), policy),
        policy,
        Some(&checks),
    )?;
    let parallel = decode(
        encrypted,
        decoder,
        Some(pool),
        &mut allowance(encrypted.len(), policy),
        policy,
        Some(&checks),
    )?;
    if *serial != *parallel || serial[..PAGE_BYTES] != encrypted[..PAGE_BYTES] {
        return Err("serial/parallel decoded bytes or untouched header differ".into());
    }

    if generated {
        let blank = create_blank_ace16_database(&mut setup_budget(), &|| false)?;
        let start = blank.as_bytes().len();
        for (index, byte) in serial[start..].iter().enumerate() {
            let page = ((start + index) / PAGE_BYTES) as u32;
            let offset = (index % PAGE_BYTES) as u32;
            if *byte != page.wrapping_mul(37).wrapping_add(offset) as u8 {
                return Err("decoded generated padding differs from original plaintext".into());
            }
        }
    }

    for policy in [
        PagePolicy {
            invalid_ordinals: true,
            ..policy
        },
        PagePolicy {
            cancelled_pages: true,
            ..policy
        },
        PagePolicy {
            bytes: PAGE_BYTES as u64 - 1,
            ..policy
        },
        PagePolicy {
            steps: PAGE_STEPS - 1,
            ..policy
        },
    ] {
        let mut failures = Vec::new();
        for selected_pool in [None, Some(pool)] {
            let failure = decode(
                encrypted,
                decoder,
                selected_pool,
                &mut allowance(encrypted.len(), policy),
                policy,
                Some(&checks),
            )
            .err()
            .ok_or("expected refusal was accepted")?;
            failures.push(failure);
        }
        let expected_page = if policy.invalid_ordinals || policy.cancelled_pages {
            2
        } else {
            1
        };
        if failures
            .iter()
            .any(|failure| failure.page != Some(expected_page))
        {
            return Err("failure did not retain the lowest original page ordinal".into());
        }
        let correct = |cause: &InspectError| {
            if policy.invalid_ordinals {
                matches!(cause, InspectError::MalformedEncryption)
            } else if policy.cancelled_pages {
                matches!(cause, InspectError::Cancelled)
            } else {
                matches!(cause, InspectError::Limit(_))
            }
        };
        if failures.iter().any(|failure| !correct(&failure.cause)) {
            return Err("serial/parallel refusal kinds differ".into());
        }
    }

    for (ordinal, bytes, steps, cancelled) in [
        (0, PAGE_BYTES as u64, PAGE_STEPS, false),
        (2, PAGE_BYTES as u64, PAGE_STEPS, true),
        (2, PAGE_BYTES as u64 - 1, PAGE_STEPS, false),
        (2, PAGE_BYTES as u64, PAGE_STEPS - 1, false),
    ] {
        let original = &encrypted[2 * PAGE_BYTES..3 * PAGE_BYTES];
        let mut page = Zeroizing::new(original.to_vec());
        let failure = decoder.decode_page(
            ordinal,
            &mut page,
            &mut ResourceBudget::new(bytes, 0, steps),
            &|| cancelled,
        );
        if failure.is_ok() || page.as_slice() != original {
            return Err("failed decoder changed caller ciphertext".into());
        }
    }

    for resource in [Resource::Bytes, Resource::Steps] {
        let normal = allowance(encrypted.len(), policy);
        let mut exhausted = ResourceBudget::new(
            if resource == Resource::Bytes {
                0
            } else {
                normal.remaining(Resource::Bytes)
            },
            0,
            if resource == Resource::Steps {
                0
            } else {
                normal.remaining(Resource::Steps)
            },
        );
        checks.store(0, Ordering::Relaxed);
        let failure = decode(
            encrypted,
            decoder,
            Some(pool),
            &mut exhausted,
            policy,
            Some(&checks),
        )
        .err()
        .ok_or("coordinator quota was ignored")?;
        if failure.page.is_some() || checks.load(Ordering::Relaxed) != 0 {
            return Err("workers ran before coordinator admission".into());
        }
    }
    let digest = Sha256::digest(&serial[PAGE_BYTES..]);
    let mut digest_hex = String::new();
    for byte in digest {
        write!(&mut digest_hex, "{byte:02x}")?;
    }
    println!(
        "verified decoded_bytes={} sha256={digest_hex}; ordered errors, cancellation, worker/coordinator quotas",
        serial.len() - PAGE_BYTES,
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Arguments::parse();
    let password = Zeroizing::new(args.password);
    let generated = args.input.is_none();
    let encrypted = match args.input {
        Some(path) => {
            let file = fs::File::open(path)?;
            let length = file.metadata()?.len();
            if length > MAX_INPUT_BYTES as u64
                || length < (PAGE_BYTES * 6) as u64
                || !length.is_multiple_of(PAGE_BYTES as u64)
            {
                return Err("input must contain 6 or more complete pages, at most 64 MiB".into());
            }
            let mut bytes = Zeroizing::new(Vec::new());
            file.take(MAX_INPUT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 != length {
                return Err("input size changed while reading".into());
            }
            bytes
        }
        None => fixture(args.mib, &password)?,
    };
    if let Some(path) = args.generate {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(&encrypted)?;
        file.sync_all()?;
        println!(
            "generated encrypted_bytes={} public benchmark credential",
            encrypted.len()
        );
        return Ok(());
    }

    let envelope = read_agile_encryption(
        &mut Cursor::new(encrypted.as_slice()),
        &mut setup_budget(),
        &|| false,
    )?;
    let decoder = envelope.unlock(Some(&password), &mut setup_budget(), &|| false)?;
    let pool = ThreadPoolBuilder::new()
        .num_threads(args.threads as usize)
        .build()?;
    if matches!(args.mode, Mode::Verify) {
        return verify(&encrypted, &decoder, &pool, generated);
    }

    let selected_pool = matches!(args.mode, Mode::Rayon).then_some(&pool);
    let policy = PagePolicy::normal();
    let start = Instant::now();
    for _ in 0..args.iterations {
        let decoded = decode(
            black_box(&encrypted),
            &decoder,
            selected_pool,
            &mut allowance(encrypted.len(), policy),
            policy,
            None,
        )?;
        black_box(decoded);
    }
    let elapsed = start.elapsed();
    let decoded_bytes = (encrypted.len() - PAGE_BYTES) as u64 * u64::from(args.iterations);
    println!(
        "mode={} threads={} iterations={} total_ms={:.3} ms_per_image={:.3} MiB_per_second={:.2} workload=prepay+clone+decode+ordered-status+zeroize",
        if selected_pool.is_some() {
            "rayon"
        } else {
            "serial"
        },
        if selected_pool.is_some() {
            args.threads
        } else {
            1
        },
        args.iterations,
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / f64::from(args.iterations),
        decoded_bytes as f64 / MIB as f64 / elapsed.as_secs_f64()
    );
    Ok(())
}
