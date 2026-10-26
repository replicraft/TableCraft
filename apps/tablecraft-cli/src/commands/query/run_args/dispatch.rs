//! Compose owned CLI arguments with borrowed query and native-source controls.
use super::RunArgs;
use crate::commands::query::{Error, parse, retrieval};

impl RunArgs {
    pub(in crate::commands::query) fn run_buffered(
        self,
        password: Option<&str>,
    ) -> Result<String, Error> {
        if self.stream {
            return Err(Error::Parameter);
        }
        let parsed = self.sql.as_deref().map(parse).transpose()?;
        let input = match (parsed.as_ref(), self.saved.as_deref()) {
            (Some(query), None) => retrieval::Input::Sql(query.query()),
            (None, Some(name)) => retrieval::Input::Saved(name),
            _ => return Err(Error::Parameter),
        };

        retrieval::run(
            input,
            &self.parameters,
            self.controls(),
            self.native_input(password),
            self.sqlite.as_deref(),
        )
    }

    pub(in crate::commands::query) fn controls(&self) -> retrieval::Controls {
        retrieval::Controls {
            binding_depth: self.binding_depth,
            declared_integral: self.declared_integral,
            max_text_bytes: self.large_values.text_bytes.map(|bytes| bytes as usize),
            output_bytes: self
                .large_values
                .output_bytes
                .map_or(crate::output::MAX_OUTPUT_BYTES, |bytes| bytes as usize),
            execution_limits: self.execution_limits.limits(),
            comparison_limits: self.comparison_limits,
            text_policy: self.text_policy,
            collation: self.collation,
            like_mode: self.like_mode,
            year_window: self.year_window,
            character_code_page: self.character_code_page,
            number_format: self.number_format,
            date_format: self.date_format,
            reference_year: self.reference_year,
            first_day_of_week: self.first_day_of_week,
            first_week_of_year: self.first_week_of_year,
            case_locale: self.case_locale.clone(),
            clock: self.clock,
            random_seed: self.random_seed,
        }
    }

    pub(in crate::commands::query) fn native_input<'a>(
        &'a self,
        password: Option<&'a str>,
    ) -> Option<retrieval::NativeInput<'a>> {
        self.database.as_deref().map(|path| retrieval::NativeInput {
            path,
            password,
            links: &self.links,
            recomputed_columns: &self.recomputed_columns,
            external: &self.external,
            external_index: &self.external_index,
            bytes: self
                .native_bytes
                .unwrap_or(crate::native::DEFAULT_NATIVE_BYTES),
            rows: self
                .native_rows
                .unwrap_or(crate::native::DEFAULT_NATIVE_ROWS),
            steps: self
                .native_steps
                .unwrap_or(crate::native::DEFAULT_NATIVE_STEPS),
            index: &self.index,
            auto_index: self.auto_index,
            multivalues: self.multivalues,
            collections: self.collections,
            decoded_attachments: self.decoded_attachments,
            stored_calculations: self.stored_calculations,
            attachment_bytes: self.attachment_bytes,
            native_value_bytes: self.native_value_bytes,
            native_text_bytes: self.native_text_bytes,
        })
    }
}
