//! Original engineering probe. This is not a native Access/NLS oracle.
use clap::{Parser, ValueEnum};
use icu_collator::{
    CollatorBorrowed,
    options::{CollatorOptions, Strength},
};
use icu_locale_core::Locale;
use std::{cmp::Ordering, hint::black_box, time::Instant};
use tablecraft_ace::{NativeSortOrder, compare_general_text};
use tablecraft_core::limits::ResourceBudget;

const DEFAULT_ROUNDS: u32 = 2000;
const MAX_ROUNDS: i64 = 100000;

const SAMPLES: &[&str] = &[
    "", "a", "A", "a ", "a  ", "ab", "a-b", "a'b", "a.b", "a_b", "apple", "APPLE", "é", "e\u{301}",
    "e", "ß", "ss", "æ", "ae", "œ", "oe", "I", "i", "İ", "ı", "å", "aa", "ä", "ö", "z", "ø", "\0",
    "\u{80}", "\u{301}", "\u{fd6f}", "😀", "\u{e000}", "co-op", "coop", "can't", "cant",
];
const BENCH_PAIRS: &[(&str, &str)] = &[
    ("apple", "APPLE"),
    ("Straße", "STRASSE"),
    ("é", "e\u{301}"),
    ("co-op", "coop"),
    ("a", "a "),
    ("😀", "\u{e000}"),
];
#[derive(Clone, Copy, ValueEnum)]
enum Provider {
    Icu,
    GeneralLegacy,
    #[value(name = "general-2010")]
    General2010,
}
impl Provider {
    fn label(self) -> &'static str {
        match self {
            Self::Icu => "icu",
            Self::GeneralLegacy => "general-legacy",
            Self::General2010 => "general-2010",
        }
    }
}
#[derive(Parser)]
struct Args {
    #[arg(long, value_enum)]
    provider: Option<Provider>,
    #[arg(long, default_value_t = DEFAULT_ROUNDS, value_parser = clap::value_parser!(u32).range(1..=MAX_ROUNDS))]
    rounds: u32,
}

fn budget() -> ResourceBudget {
    ResourceBudget::new(u64::MAX, u64::MAX, u64::MAX)
}
fn collator(locale: &str) -> CollatorBorrowed<'static> {
    let mut options = CollatorOptions::default();
    options.strength = Some(Strength::Secondary);
    CollatorBorrowed::try_new(locale.parse::<Locale>().unwrap().into(), options).unwrap()
}
fn native(version: u8, left: &str, right: &str, budget: &mut ResourceBudget) -> Ordering {
    compare_general_text(
        NativeSortOrder::from_native_bytes([9, 4, 0, version]),
        left,
        right,
        budget,
        &|| false,
    )
    .unwrap()
}
fn main() {
    let args = Args::parse();
    let english = collator("en-US");
    if let Some(provider) = args.provider {
        benchmark(provider, args.rounds, &english);
        return;
    }
    let turkish = collator("tr-TR");
    // Independent capability expectations, not Access compatibility assertions.
    assert_eq!(english.compare("a", "A"), Ordering::Equal);
    assert_ne!(english.compare("a", "á"), Ordering::Equal);
    assert_eq!(english.compare("é", "e\u{301}"), Ordering::Equal);
    assert_eq!(turkish.compare("I", "ı"), Ordering::Equal);
    assert_ne!(turkish.compare("I", "i"), Ordering::Equal);
    assert_eq!(english.compare_utf16(&[0xd800], &[0xfffd]), Ordering::Equal);
    println!("capabilities\tcase/accent/canonical/Turkish/surrogate controls passed");
    for version in [0, 1] {
        let mut raw_mismatches = 0;
        let mut trimmed_mismatches = 0;
        let mut budget = budget();
        for &left in SAMPLES {
            for &right in SAMPLES {
                let native = native(version, left, right, &mut budget);
                let raw = english.compare(left, right);
                let trimmed =
                    english.compare(left.trim_end_matches(' '), right.trim_end_matches(' '));
                if native != raw {
                    raw_mismatches += 1;
                }
                if native != trimmed {
                    trimmed_mismatches += 1;
                    if trimmed_mismatches <= 8 {
                        println!(
                            "counterexample\t{version}\t{left:?}\t{right:?}\t{native:?}\t{trimmed:?}"
                        );
                    }
                }
            }
        }
        println!(
            "comparison\t{version}\t{}\t{raw_mismatches}\t{trimmed_mismatches}",
            SAMPLES.len() * SAMPLES.len()
        );
    }
    for provider in [
        Provider::Icu,
        Provider::GeneralLegacy,
        Provider::General2010,
    ] {
        benchmark(provider, args.rounds, &english);
    }
}
fn benchmark(provider: Provider, rounds: u32, english: &CollatorBorrowed<'_>) {
    let label = provider.label();
    {
        let mut budget = budget();
        let mut checksum = 0_i64;
        let start = Instant::now();
        for _ in 0..rounds {
            for &(left, right) in BENCH_PAIRS {
                let order = match provider {
                    Provider::Icu => english.compare(black_box(left), black_box(right)),
                    Provider::GeneralLegacy => {
                        native(0, black_box(left), black_box(right), &mut budget)
                    }
                    _ => native(1, black_box(left), black_box(right), &mut budget),
                };
                checksum += black_box(order) as i64;
            }
        }
        println!(
            "benchmark\t{label}\t{}\t{}\t{}",
            rounds as usize * BENCH_PAIRS.len(),
            start.elapsed().as_nanos(),
            black_box(checksum)
        );
    }
}
