use super::authorized;
use reqwest::Url;

#[test]
fn redirects_only_authorize_exact_https_hosts_and_default_ports() {
    for url in [
        "https://api.github.com/repos/replicraft/TableCraft/releases/latest",
        "https://github.com/replicraft/TableCraft/releases/download/v0.1.0/a.zip",
        "https://release-assets.githubusercontent.com/path?token=sensitive",
        "https://objects.githubusercontent.com:443/path",
    ] {
        assert!(authorized(&Url::parse(url).unwrap()), "{url}");
    }
    for url in [
        "http://github.com/path",
        "https://github.com.evil.example/path",
        "https://evil.example/github.com/path",
        "https://user@github.com/path",
        "https://user:secret@github.com/path",
        "https://github.com:444/path",
        "https://github.com/path#fragment",
        "file:///tmp/update.zip",
    ] {
        assert!(!authorized(&Url::parse(url).unwrap()), "{url}");
    }
}
