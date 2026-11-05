//! Document opening selects a visible local table without following stored links.
use std::path::Path;

pub(super) fn document_title(path: &Path, filename: &str, format: &str) -> String {
    let name = path
        .file_stem()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    // These labels describe file compatibility, not the application that last
    // saved the file. Both ACE 16 format families use the Access 2016 title
    // convention; their distinct raw format codes remain in database metadata.
    // A header cannot identify the producing Access release.
    let profile = match format.split(" · ").next().unwrap_or(format) {
        "Ace2016" | "Ace2019" => "Access 2016",
        "Ace2010" => "Access 2010",
        "Ace2007" => "Access 2007",
        "Jet4" => "Access 2000–2003",
        _ => format,
    };
    format!("{name} : Database | {filename} ({profile})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_use_the_opened_path_and_file_compatibility_profile() {
        assert_eq!(
            document_title(
                Path::new("/data/TestDatabase.accdb"),
                "TestDatabase.accdb",
                "Ace2016"
            ),
            "TestDatabase : Database | TestDatabase.accdb (Access 2016)"
        );
        assert_eq!(
            document_title(
                Path::new("/data/Client Records.v2.accdb"),
                "Client Records.v2.accdb",
                "Ace2019 · deployment package"
            ),
            "Client Records.v2 : Database | Client Records.v2.accdb (Access 2016)"
        );
        assert_eq!(
            document_title(Path::new("/data/New.accdb"), "New.accdb", "UnknownProfile"),
            "New : Database | New.accdb (UnknownProfile)"
        );
        assert_eq!(
            document_title(Path::new("/data/Old.mdb"), "Old.mdb", "Jet4"),
            "Old : Database | Old.mdb (Access 2000–2003)"
        );
    }
}
