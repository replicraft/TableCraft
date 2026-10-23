//! Bounded flat Fluent catalogs, without references, variables or formatter functions.
use super::{ErrorMessageCode, ErrorMessages};
use crate::EvalError;
use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use fluent_syntax::ast::{Entry, PatternElement};
use std::{borrow::Cow, sync::OnceLock};
use unic_langid::LanguageIdentifier;

const KEY_PREFIX: &str = "error-";
const NEGATIVE_PREFIX: &str = "negative-";
const UNKNOWN_KEY: &str = "error-unknown";
const MAX_CATALOG_BYTES: usize = 131_072;
const MAX_LANGUAGE_BYTES: usize = 64;
const ENGLISH: &str = include_str!("../../../data/error_messages/en-US.ftl");

/// Host-owned immutable flat Fluent messages, suitable for concurrent query hosts.
/// Only error-N, error-negative-N and error-unknown IDs are accepted; values must be single plain
/// Text elements. There is no executable formatting or implicit locale fallback.
pub struct FluentErrorMessages {
    bundle: FluentBundle<FluentResource>,
    keys: Vec<(ErrorMessageCode, String)>,
}

impl FluentErrorMessages {
    /// Parse bounded host resources, validating message shape before formatting.
    /// Construction owns its bounded staging; it is not a per-query allocation meter.
    pub fn new(language: &str, resource: &str) -> Result<Self, EvalError> {
        if language.is_empty()
            || language.len() > MAX_LANGUAGE_BYTES
            || resource.len() > MAX_CATALOG_BYTES
        {
            return Err(EvalError::InvalidErrorCatalog);
        }
        // Reject recursive placeable syntax before the parser can allocate or
        // recurse through it. Flat diagnostic resources never need braces.
        if resource.bytes().any(|byte| matches!(byte, b'{' | b'}')) {
            return Err(EvalError::InvalidErrorCatalog);
        }
        let language: LanguageIdentifier = language
            .parse()
            .map_err(|_| EvalError::InvalidErrorCatalog)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(resource.len())
            .map_err(|_| EvalError::Allocation)?;
        owned.push_str(resource);
        let resource =
            FluentResource::try_new(owned).map_err(|_| EvalError::InvalidErrorCatalog)?;
        let mut keys = Vec::new();
        let mut fallback = false;
        for entry in resource.entries() {
            let Entry::Message(message) = entry else {
                if matches!(
                    entry,
                    Entry::Comment(_) | Entry::GroupComment(_) | Entry::ResourceComment(_)
                ) {
                    continue;
                }
                return Err(EvalError::InvalidErrorCatalog);
            };
            let pattern = message
                .value
                .as_ref()
                .ok_or(EvalError::InvalidErrorCatalog)?;
            if !message.attributes.is_empty()
                || !matches!(
                    pattern.elements.as_slice(),
                    [PatternElement::TextElement { .. }]
                )
            {
                return Err(EvalError::InvalidErrorCatalog);
            }
            let id = message.id.name;
            if id == UNKNOWN_KEY {
                fallback = true;
            } else {
                let suffix = id
                    .strip_prefix(KEY_PREFIX)
                    .ok_or(EvalError::InvalidErrorCatalog)?;
                let (digits, negative) = match suffix.strip_prefix(NEGATIVE_PREFIX) {
                    Some(digits) => (digits, true),
                    None => (suffix, false),
                };
                if digits.is_empty()
                    || !digits.bytes().all(|byte| byte.is_ascii_digit())
                    || digits.starts_with('0')
                {
                    return Err(EvalError::InvalidErrorCatalog);
                }
                let magnitude: i64 = digits.parse().map_err(|_| EvalError::InvalidErrorCatalog)?;
                let number = if negative { -magnitude } else { magnitude };
                let number = i32::try_from(number).map_err(|_| EvalError::InvalidErrorCatalog)?;
                let code =
                    ErrorMessageCode::new(number).map_err(|_| EvalError::InvalidErrorCatalog)?;
                let mut key = String::new();
                key.try_reserve_exact(id.len())
                    .map_err(|_| EvalError::Allocation)?;
                key.push_str(id);
                keys.try_reserve(1).map_err(|_| EvalError::Allocation)?;
                keys.push((code, key));
            }
        }
        if !fallback {
            return Err(EvalError::InvalidErrorCatalog);
        }
        keys.sort_unstable_by_key(|(code, _)| *code);
        let mut bundle = FluentBundle::new_concurrent(vec![language]);
        bundle
            .add_resource(resource)
            .map_err(|_| EvalError::InvalidErrorCatalog)?;
        Ok(Self { bundle, keys })
    }
}

impl ErrorMessages for FluentErrorMessages {
    fn message(&self, code: ErrorMessageCode) -> Result<&str, EvalError> {
        let id = match self.keys.binary_search_by_key(&code, |(code, _)| *code) {
            Ok(index) => self.keys[index].1.as_str(),
            Err(_) => UNKNOWN_KEY,
        };
        let message = self
            .bundle
            .get_message(id)
            .ok_or(EvalError::ErrorCatalogFailed)?;
        let pattern = message.value().ok_or(EvalError::ErrorCatalogFailed)?;
        let mut errors = Vec::new();
        let value = self.bundle.format_pattern(pattern, None, &mut errors);
        if !errors.is_empty() {
            return Err(EvalError::ErrorCatalogFailed);
        }
        match value {
            Cow::Borrowed(text) => Ok(text),
            Cow::Owned(_) => Err(EvalError::ErrorCatalogFailed),
        }
    }
}

/// Lazy fixed English reference messages. No setup occurs for queries that never
/// request a nonzero message. A bounded initialization failure is retained;
/// lookups never silently retry or switch language.
#[derive(Default)]
pub struct EnglishErrorMessages {
    catalog: OnceLock<Result<FluentErrorMessages, EvalError>>,
}

impl ErrorMessages for EnglishErrorMessages {
    fn message(&self, code: ErrorMessageCode) -> Result<&str, EvalError> {
        let catalog = self
            .catalog
            .get_or_init(|| FluentErrorMessages::new("en-US", ENGLISH));
        catalog.as_ref().map_err(|error| *error)?.message(code)
    }
}
