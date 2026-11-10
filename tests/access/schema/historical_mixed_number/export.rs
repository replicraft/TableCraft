use std::{env, error::Error, fs::OpenOptions, io::Write, path::PathBuf};

use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_mixed::{
    self as raw, Case,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env::args_os().nth(1).ok_or("expected export directory")?);

    for (name, case) in [
        ("text", Case::Text),
        ("binary", Case::Binary),
        ("later", Case::LaterAbsentText),
    ] {
        let [_, before] = raw::pair(case);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join(format!("{name}-before.accdb")))?;
        file.write_all(&before)?;

        for row in 0..3 {
            let after = raw::after(&before, row, 37);
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(root.join(format!("{name}-after-{row}.accdb")))?;
            file.write_all(&after)?;
        }
    }

    Ok(())
}
