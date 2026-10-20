use super::*;
use std::io::{Read, Seek, SeekFrom, Write};

const PAGE_BYTES: usize = 4096;
const FILE_BYTES: u64 = 128 * 1024 * 1024;
const LAST_PAGE: u32 = (FILE_BYTES / PAGE_BYTES as u64 - 1) as u32;
struct SparseCopy {
    path: PathBuf,
    prefix: Vec<u8>,
    tail: Vec<u8>,
}
impl SparseCopy {
    fn new(code: u8) -> Self {
        let mut prefix = fixture::database();
        prefix[20] = code;
        if code != 1 {
            prefix[4..20].copy_from_slice(b"Standard ACE DB\0");
        }
        let tail = prefix[12 * PAGE_BYTES..13 * PAGE_BYTES].to_vec();
        let mut map = vec![0];
        map.extend_from_slice(&LAST_PAGE.to_le_bytes());
        map.push(1);
        prefix[11 * PAGE_BYTES..12 * PAGE_BYTES]
            .copy_from_slice(&support::data_page(1, &[(0, map)]));
        let path = std::env::temp_dir().join(format!(
            "tablecraft-large-browse-{}-{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
            if code == 1 { "mdb" } else { "accdb" }
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(&prefix).unwrap();
        file.seek(SeekFrom::Start(u64::from(LAST_PAGE) * PAGE_BYTES as u64))
            .unwrap();
        file.write_all(&tail).unwrap();
        Self { path, prefix, tail }
    }
    fn run(&self, command: &str, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .arg(command)
            .arg(&self.path)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for SparseCopy {
    fn drop(&mut self) {
        let mut file = fs::File::open(&self.path).unwrap();
        assert_eq!(file.metadata().unwrap().len(), FILE_BYTES);
        let mut prefix = vec![0; self.prefix.len()];
        file.read_exact(&mut prefix).unwrap();
        assert_eq!(prefix, self.prefix);
        file.seek(SeekFrom::Start(u64::from(LAST_PAGE) * PAGE_BYTES as u64))
            .unwrap();
        let mut tail = vec![0; PAGE_BYTES];
        file.read_exact(&mut tail).unwrap();
        assert_eq!(tail, self.tail);
        drop(file);
        fs::remove_file(&self.path).unwrap();
    }
}
#[test]
fn large_native_files_support_discovery_far_page_browsing_and_bounded_queries() {
    for code in [1, 2, 5, 6] {
        let file = SparseCopy::new(code);
        assert_eq!(
            success(file.run("tables", &["--count"])),
            b"Name\tKind\tRow Count\nItems\tUser\t3\n"
        );
        assert_eq!(
            success(file.run("rows", &["Items", "--offset", "1", "--limit", "1"])),
            b"Id\tScore\n2\t20\n"
        );
        success(file.run("inspect", &["--table", "Items"]));
        let query = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
            .args([
                "query",
                "run",
                "SELECT Id,Score FROM Items WHERE Id=2",
                "--database",
            ])
            .arg(&file.path)
            .output()
            .unwrap();
        assert_eq!(success(query), b"Id\tScore\n2\t20\n");
        if code == 1 {
            let output = file.path.with_extension("updated.mdb");
            let update = Command::new(env!("CARGO_BIN_EXE_tablecraft"))
                .args([
                    "query",
                    "update",
                    "UPDATE Items SET Score=Score+1 WHERE Id=2",
                    "--database",
                ])
                .arg(&file.path)
                .arg("--output")
                .arg(&output)
                .output()
                .unwrap();
            assert_eq!(update.status.code(), Some(2));
            assert!(update.stdout.is_empty());
            assert!(!output.exists());
        }
    }
}
