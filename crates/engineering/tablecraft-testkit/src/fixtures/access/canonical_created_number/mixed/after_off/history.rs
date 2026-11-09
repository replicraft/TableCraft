//! Independent pinned sources whose live user body follows an earlier DB body.
#[cfg(test)]
mod tests;

/// Retained first-ADD-after-OFF sources for the independent second-ADD experiment.
#[derive(Clone, Copy, Debug)]
pub enum SourceCase {
    /// Generated ID + Score → OFF → Number AfterOff.
    NumberThenNumber,
    /// Generated ID + Text/Number/Text → OFF → Text LabelOff.
    MixedThenText,
}

/// Build the complete retained SDK BEFORE without a production encoder/parser.
/// Appending another field reuses the actual after_off::append_number/text owner.
pub fn source(case: SourceCase) -> Vec<u8> {
    match case {
        SourceCase::NumberThenNumber => {
            super::append_number(&super::source(super::SourceCase::Number), "AfterOff")
        }
        SourceCase::MixedThenText => {
            super::append_text(&super::source(super::SourceCase::Mixed), "LabelOff")
        }
    }
}
