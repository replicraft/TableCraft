//! UI controls and publication cache around the renderer-neutral display service.
mod options;

pub(crate) use options::show_options;
use tablecraft_display::{Engine, Options, raw};

#[derive(Default)]
pub(crate) struct Cache {
    policy: Option<Options>,
    engine: Option<Engine>,
}

impl Cache {
    pub fn update(&mut self, options: &Options) -> bool {
        if self.policy.as_ref() == Some(options) {
            return false;
        }

        self.engine = Engine::new(options).ok();
        self.policy = Some(options.clone());
        true
    }

    pub fn apply(&self, cell: &mut crate::Cell) {
        let Some(presentation) = cell.presentation else {
            return;
        };
        cell.text = match &self.engine {
            Some(engine) => engine.format(presentation),
            None => raw(presentation),
        };
    }

    pub fn apply_data(&self, data: &mut crate::Data) {
        for row in &mut data.rows {
            for cell in row {
                self.apply(cell);
            }
        }
    }

    pub fn system_examples(&self) -> Option<(String, String)> {
        self.engine.as_ref().map(Engine::system_examples)
    }

    pub fn status(&self) -> Option<(&str, bool, Option<&str>)> {
        self.engine.as_ref().map(|engine| {
            (
                engine.locale.as_str(),
                engine.fallback_locale,
                engine.system_currency.as_deref(),
            )
        })
    }
}

#[cfg(test)]
mod tests;
