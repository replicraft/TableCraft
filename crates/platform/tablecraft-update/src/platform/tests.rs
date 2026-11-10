use super::Platform;

#[test]
fn native_detection_does_not_claim_another_architecture_or_abi() {
    if let Some(platform) = Platform::detect() {
        let target = platform.target();
        let architecture = std::env::consts::ARCH;
        assert!(target.starts_with(architecture));

        match std::env::consts::OS {
            "windows" => assert!(target.ends_with("windows-msvc")),
            "macos" => assert!(target.ends_with("apple-darwin")),
            "linux" => assert!(target.ends_with("linux-gnu")),
            other => panic!("unexpected supported host: {other}"),
        }
    }
}
