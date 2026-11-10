use crate::UpdateError;
use reqwest::{Url, blocking::Client, redirect::Policy};
use std::{
    io::{self, Read},
    time::Duration,
};

// GitHub release assets redirect from github.com to these authenticated hosts.
const DOWNLOAD_HOSTS: [&str; 4] = [
    "api.github.com",
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
];
const MAX_REDIRECTS: usize = 5;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) struct Body {
    pub(crate) reader: Box<dyn Read>,
    pub(crate) length: Option<u64>,
}

pub(crate) trait Transport {
    fn get(&self, url: &str) -> Result<Body, UpdateError>;
}

#[derive(Debug)]
pub(crate) struct Http {
    client: Client,
}

impl Http {
    pub(crate) fn new(version: &str) -> Result<Self, UpdateError> {
        let policy = Policy::custom(|attempt| {
            if attempt.previous().len() > MAX_REDIRECTS || !authorized(attempt.url()) {
                attempt.error("Unauthorized update redirect")
            } else {
                attempt.follow()
            }
        });
        let client = Client::builder()
            .user_agent(format!("TableCraft/{version}"))
            .https_only(true)
            .redirect(policy)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|_| UpdateError::Network)?;
        Ok(Self { client })
    }
}

impl Transport for Http {
    fn get(&self, url: &str) -> Result<Body, UpdateError> {
        let parsed = Url::parse(url).map_err(|_| UpdateError::UnauthorizedUrl)?;
        if !authorized(&parsed) {
            return Err(UpdateError::UnauthorizedUrl);
        }
        let response = self
            .client
            .get(parsed)
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .send()
            .map_err(|_| UpdateError::Network)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(UpdateError::NotFound);
        }
        if response.status() != reqwest::StatusCode::OK {
            return Err(UpdateError::HttpStatus);
        }
        let length = response.content_length();
        Ok(Body {
            reader: Box::new(NetworkReader(response)),
            length,
        })
    }
}

struct NetworkReader(reqwest::blocking::Response);

impl Read for NetworkReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // Redirect URLs can contain temporary credentials. Keep network stack
        // errors out of source chains that a host may render diagnostically.
        self.0
            .read(buffer)
            .map_err(|_| io::Error::other("Update network read failed"))
    }
}

fn authorized(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url
            .host_str()
            .is_some_and(|host| DOWNLOAD_HOSTS.contains(&host))
        && url.fragment().is_none()
}

#[cfg(test)]
mod tests;
