//! Optional launcher pause after the command's output or error is published.
use std::io::{self, IsTerminal, Read, Write};

const PROMPT: &[u8] = b"Press Enter to close.\n";

pub(crate) fn after_publication(requested: bool) -> io::Result<()> {
    if !requested {
        return Ok(());
    }

    let input = io::stdin();
    let output = io::stdout();
    let errors = io::stderr();
    let input_terminal = input.is_terminal();
    let output_terminal = output.is_terminal();
    if !input_terminal || !output_terminal {
        return Ok(());
    }

    wait_if_interactive(
        requested,
        input_terminal,
        output_terminal,
        &mut input.lock(),
        &mut output.lock(),
        &mut errors.lock(),
    )
}

fn wait_if_interactive(
    requested: bool,
    input_terminal: bool,
    output_terminal: bool,
    input: &mut impl Read,
    output: &mut impl Write,
    errors: &mut impl Write,
) -> io::Result<()> {
    if !requested || !input_terminal || !output_terminal {
        return Ok(());
    }

    output.flush()?;
    errors.flush()?;
    errors.write_all(PROMPT)?;
    errors.flush()?;

    let mut byte = [0];
    loop {
        match input.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) if byte[0] == b'\n' => return Ok(()),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests;
