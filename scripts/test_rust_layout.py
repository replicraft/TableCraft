"""Regression checks for the module-layout source guard."""
import unittest

from check_rust_layout import composition_error


class CompositionTests(unittest.TestCase):
    def test_composition_and_opaque_attributes(self):
        source = '''//! fn ignored_in_docs() {}
#![doc = r###"struct AlsoIgnored;"###]
/* outer /* fn hidden() {} */ comment */
#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub(crate) mod platform;
mod tests;
pub use platform::{Task, Outcome};
use std::path::Path;
'''
        self.assertIsNone(composition_error(source))

    def test_business_items_and_inline_tests_are_rejected(self):
        for source in (
            'pub fn run() {}',
            'pub struct State;',
            'pub enum Command { Open }',
            'impl State {}',
            'const LIMIT: usize = 8;',
            '#[cfg(test)] mod tests { #[test] fn works() {} }',
        ):
            with self.subTest(source=source):
                self.assertIsNotNone(composition_error(source))

    def test_broken_source_is_not_silently_accepted(self):
        self.assertIsNotNone(composition_error('#[cfg(test)'))
        self.assertIsNotNone(composition_error('pub use module::Thing'))
        with self.assertRaises(ValueError):
            composition_error('/* unfinished')


if __name__ == "__main__":
    unittest.main()
