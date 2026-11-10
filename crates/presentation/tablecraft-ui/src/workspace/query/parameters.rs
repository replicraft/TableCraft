//! Shared literal parameter presentation; SQL evaluation remains in runtime.
pub(in crate::workspace) mod editor;
pub(in crate::workspace) mod prompt;
pub(in crate::workspace) mod scratch;

use crate::text_input::MAX_EDITOR_SCALARS;

pub(in crate::workspace) const MAX_PARAMETERS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterSpec {
    pub name: String,
    pub kind: String,
    pub label: String,
    pub nullable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterInput {
    pub name: String,
    pub kind: String,
    pub text: String,
    pub is_null: bool,
}

pub(in crate::workspace) fn inputs_within_limits(parameters: &[ParameterInput]) -> bool {
    parameters.len() <= MAX_PARAMETERS
        && input_bytes(parameters).is_some()
        && parameters
            .iter()
            .all(|input| input.text.chars().nth(MAX_EDITOR_SCALARS).is_none())
}

pub(in crate::workspace) fn input_bytes(parameters: &[ParameterInput]) -> Option<usize> {
    parameters.iter().try_fold(0usize, |bytes, parameter| {
        bytes
            .checked_add(parameter.name.len())?
            .checked_add(parameter.kind.len())?
            .checked_add(parameter.text.len())
    })
}

pub(in crate::workspace) fn inputs_admitted(
    specs: &[ParameterSpec],
    parameters: &[ParameterInput],
) -> bool {
    inputs_within_limits(parameters)
        && specs.len() == parameters.len()
        && specs
            .iter()
            .zip(parameters)
            .all(|(spec, input)| input_admitted(spec, input))
}

pub(in crate::workspace) fn input_admitted(spec: &ParameterSpec, input: &ParameterInput) -> bool {
    spec.name == input.name
        && (spec.kind == "value" || spec.kind == input.kind)
        && (spec.nullable || !input.is_null)
        && editor::INPUT_KINDS
            .iter()
            .any(|(kind, _)| *kind == input.kind)
}

#[allow(clippy::ptr_arg)] // Retained bytes include spare vector capacity.
pub(in crate::workspace) fn retained_inputs(inputs: &Vec<ParameterInput>) -> Option<usize> {
    let mut count = super::super::objects::memory::Counter::new(0);
    count.vector(inputs);
    for input in inputs {
        count.string(&input.name);
        count.string(&input.kind);
        count.string(&input.text);
    }
    count.finish()
}

pub(in crate::workspace) fn input_copy_bytes(inputs: &[ParameterInput]) -> Option<usize> {
    let mut count = super::super::objects::memory::Counter::new(0);
    count.count::<ParameterInput>(inputs.len());
    for input in inputs {
        count.string(&input.name);
        count.string(&input.kind);
        count.string(&input.text);
    }
    count.finish()
}

pub(in crate::workspace) fn copy_text(text: &str) -> Option<String> {
    let mut owned = String::new();
    owned.try_reserve_exact(text.len()).ok()?;
    owned.push_str(text);
    Some(owned)
}

pub(in crate::workspace) fn copy_inputs(inputs: &[ParameterInput]) -> Option<Vec<ParameterInput>> {
    let mut result = Vec::new();
    result.try_reserve_exact(inputs.len()).ok()?;
    for input in inputs {
        result.push(ParameterInput {
            name: copy_text(&input.name)?,
            kind: copy_text(&input.kind)?,
            text: copy_text(&input.text)?,
            is_null: input.is_null,
        });
    }
    Some(result)
}
