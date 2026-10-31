use std::fmt;

#[derive(Debug)]
pub(crate) enum Error {
    UnsupportedPlatform,
    Credentials,
}

impl Error {
    pub(crate) fn invalid_input(&self) -> bool {
        matches!(self, Self::Credentials)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => {
                "offline journal editing requires macOS or Linux; no file was opened"
            }
            Self::Credentials => {
                "journal editing accepts only unencoded Jet4 files; --password is unsupported"
            }
        })
    }
}
