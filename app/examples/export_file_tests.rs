//! Headless public-dummy HTTP export → explicit destination tests.
//! No GUI, real Host, Core profile, real session data, extraction, or physical-input claim.
//! The only subprocess is the owned transport Node HTTP fixture in `public-normal` mode.
//! Public test ZIPs remain in fresh private temporary directories; no cleanup deletion.
#[cfg(test)]
#[path = "../src/export_file.rs"]
mod export_file;

fn main() {
    println!("Headless public export fixture only; run cargo test --example export_file_tests.");
}

#[cfg(test)]
mod composition {
    use super::export_file::{Destination, Failure, Saved};
    use dsh_native_transport::{Error, Limits, NativeClient, SecretLaunchUrl, dto::SessionId};
    use std::{
        fs::{self, File},
        io::{self, Read},
        os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt},
        path::{Path, PathBuf},
        process::Stdio,
        sync::atomic::{AtomicU64, Ordering},
        time::Duration,
    };
    use tokio::{
        io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
        process::{Child, ChildStdout, Command},
        time::timeout,
    };

    const PUBLIC_MARKER: &[u8] = b"PUBLIC_ROOT_LOG_WITH_ATTACHMENT_REFERENCE\n";
    const MAX_PUBLIC_FIXTURE_BYTES: usize = 16 * 1024;
    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    fn session() -> SessionId {
        SessionId::new("session / opaque:%?&=💾").expect("public fixture SessionId")
    }
    fn owned_directory() -> PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "dsh-export-composition-PUBLIC-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .recursive(false)
            .mode(0o700)
            .create(&directory)
            .unwrap_or_else(|_| panic!("create owned public fixture directory"));
        assert_eq!(
            fs::metadata(&directory)
                .unwrap_or_else(|_| panic!("public fixture directory metadata"))
                .permissions()
                .mode()
                & 0o777,
            0o700,
            "private fixture directory mode"
        );
        directory
    }
    fn destination(path: &Path) -> Destination {
        let input = path
            .to_str()
            .unwrap_or_else(|| panic!("public fixture path encoding"))
            .to_owned();
        Destination::parse(input).expect("explicit public fixture destination")
    }
    fn read_public_archive(path: &Path, expected: usize) -> Vec<u8> {
        assert!(
            expected <= MAX_PUBLIC_FIXTURE_BYTES,
            "public archive read bound"
        );
        let metadata =
            fs::symlink_metadata(path).unwrap_or_else(|_| panic!("saved public fixture metadata"));
        assert!(metadata.is_file(), "saved public fixture is regular");
        assert_eq!(metadata.len(), expected as u64, "saved public byte count");
        assert_eq!(
            metadata.permissions().mode() & 0o777,
            0o600,
            "private saved file mode"
        );
        let file = File::open(path).unwrap_or_else(|_| panic!("open saved public fixture"));
        let mut bytes = Vec::with_capacity(expected + 1);
        // The extra byte detects excess output without an unbounded read or raw ZIP decoding.
        Read::take(file, (expected + 1) as u64)
            .read_to_end(&mut bytes)
            .unwrap_or_else(|_| panic!("bounded saved public fixture read"));
        assert_eq!(bytes.len(), expected, "bounded public output length");
        assert!(
            bytes.starts_with(b"PK\x03\x04"),
            "public fixture ZIP marker"
        );
        assert!(
            bytes
                .windows(PUBLIC_MARKER.len())
                .any(|window| window == PUBLIC_MARKER),
            "only public fixture log bytes were saved"
        );
        assert!(
            !bytes
                .windows(b"PRIVATE".len())
                .any(|window| window == b"PRIVATE"),
            "private response-header/body markers are not saved"
        );
        bytes
    }
    async fn bounded_line(reader: &mut BufReader<ChildStdout>) -> String {
        let mut bytes = Vec::with_capacity(MAX_PUBLIC_FIXTURE_BYTES + 1);
        let count = timeout(
            Duration::from_secs(5),
            (&mut *reader)
                .take((MAX_PUBLIC_FIXTURE_BYTES + 1) as u64)
                .read_until(b'\n', &mut bytes),
        )
        .await
        .unwrap_or_else(|_| panic!("public fixture startup line timeout"))
        .unwrap_or_else(|_| panic!("public fixture startup line read"));
        assert!(
            count <= MAX_PUBLIC_FIXTURE_BYTES && bytes.last() == Some(&b'\n'),
            "public fixture startup line size/termination"
        );
        bytes.pop();
        String::from_utf8(bytes).unwrap_or_else(|_| panic!("public fixture startup encoding"))
    }

    /// Own the child before readiness reads; normal shutdown is bounded and awaited.
    /// Unwind/drop requests a kill through the still-owned child, with Tokio's child reaper.
    struct Fixture {
        child: Option<Child>,
        reader: BufReader<ChildStdout>,
        launch: Option<SecretLaunchUrl>,
        len: usize,
    }
    impl Fixture {
        async fn start() -> Self {
            let mut child = Command::new("/usr/bin/node")
                .arg(format!(
                    "{}/../transport/tests/export_fixture.mjs",
                    env!("CARGO_MANIFEST_DIR")
                ))
                .arg("public-normal")
                .env_clear()
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                // Failure output stays fixed in this Rust fixture; do not forward child data.
                .stderr(Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap_or_else(|_| panic!("start owned public HTTP fixture"));
            let stdout = child
                .stdout
                .take()
                .unwrap_or_else(|| panic!("owned public fixture readiness pipe"));
            let mut fixture = Self {
                child: Some(child),
                reader: BufReader::new(stdout),
                launch: None,
                len: 0,
            };
            let url = bounded_line(&mut fixture.reader).await;
            fixture.launch = Some(
                SecretLaunchUrl::new(url)
                    .unwrap_or_else(|_| panic!("private fixture launch rejected")),
            );
            let metadata = bounded_line(&mut fixture.reader).await;
            fixture.len = metadata
                .strip_prefix("length:")
                .and_then(|count| count.parse().ok())
                .unwrap_or_else(|| panic!("public fixture length metadata"));
            assert!(
                (1..=MAX_PUBLIC_FIXTURE_BYTES).contains(&fixture.len),
                "public fixture archive length bound"
            );
            fixture
        }
        async fn connect(&mut self) -> NativeClient {
            let launch = self
                .launch
                .take()
                .unwrap_or_else(|| panic!("single public fixture launch exchange"));
            NativeClient::connect(
                launch,
                Limits {
                    max_result_bytes: MAX_PUBLIC_FIXTURE_BYTES,
                    max_concurrent_rpc: 1,
                    timeout: Duration::from_secs(2),
                    ..Limits::default()
                },
            )
            .await
            .expect("public fixture authenticated client")
        }
        async fn stop(mut self) {
            let mut child = self
                .child
                .take()
                .unwrap_or_else(|| panic!("owned public fixture child"));
            let result = timeout(Duration::from_secs(3), async {
                child
                    .stdin
                    .as_mut()
                    .ok_or(io::ErrorKind::BrokenPipe)?
                    .write_all(b"stop\n")
                    .await?;
                child.wait().await
            })
            .await;
            let status = match result {
                Ok(Ok(status)) => status,
                _ => {
                    // Do not leave the pure Node fixture alive on failed normal teardown.
                    child
                        .start_kill()
                        .unwrap_or_else(|_| panic!("kill owned public fixture"));
                    timeout(Duration::from_secs(3), child.wait())
                        .await
                        .unwrap_or_else(|_| panic!("owned public fixture reap timeout"))
                        .unwrap_or_else(|_| panic!("owned public fixture reap"));
                    panic!("owned public fixture normal shutdown failed");
                }
            };
            assert!(
                status.success(),
                "public fixture wire assertions or shutdown failed"
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(child) = self.child.as_mut() {
                let _ = child.start_kill();
            }
        }
    }

    #[tokio::test]
    async fn authenticated_public_archive_composes_with_explicit_private_file_save() {
        let mut fixture = Fixture::start().await;
        let client = fixture.connect().await;
        let directory = owned_directory();
        let path = directory.join("PUBLIC-export.zip");
        let prepared = destination(&path)
            .prepare()
            .expect("prepare explicit public destination");
        assert!(
            !path.exists(),
            "preparation does not create the destination"
        );
        let archive = client
            .export_session(&session())
            .await
            .expect("public fixture archive");
        assert_eq!(archive.len(), fixture.len, "transport public archive count");
        assert_eq!(
            prepared.save(archive),
            Ok(Saved { bytes: fixture.len }),
            "composed transport archive save"
        );
        let _public_bytes = read_public_archive(&path, fixture.len);
        client.close().await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn second_actual_export_is_consumed_without_overwriting_the_same_explicit_destination() {
        let mut fixture = Fixture::start().await;
        let client = fixture.connect().await;
        let directory = owned_directory();
        let path = directory.join("PUBLIC-existing.zip");
        let first = client
            .export_session(&session())
            .await
            .expect("first public fixture archive");
        assert_eq!(
            destination(&path)
                .prepare()
                .expect("first public preparation")
                .save(first),
            Ok(Saved { bytes: fixture.len })
        );
        let before = read_public_archive(&path, fixture.len);
        let inode = fs::metadata(&path)
            .unwrap_or_else(|_| panic!("public inode metadata"))
            .ino();
        let second = client
            .export_session(&session())
            .await
            .expect("second actual public export");
        assert_eq!(
            destination(&path)
                .prepare()
                .expect("repeat public preparation")
                .save(second),
            Err(Failure::NotCreated),
            "exclusive destination refuses overwrite and consumes the second archive"
        );
        let after = read_public_archive(&path, fixture.len);
        assert!(
            before == after,
            "existing public archive bytes remain unchanged"
        );
        assert_eq!(
            fs::metadata(&path)
                .unwrap_or_else(|_| panic!("unchanged public inode metadata"))
                .ino(),
            inode,
            "existing public archive inode remains unchanged"
        );
        client.close().await;
        fixture.stop().await;
    }

    #[tokio::test]
    async fn completed_owned_archive_is_not_implicitly_revoked_when_input_transport_is_closed() {
        let mut fixture = Fixture::start().await;
        let client = fixture.connect().await;
        let stale = client.clone();
        let directory = owned_directory();
        let path = directory.join("PUBLIC-owned-after-close.zip");
        let prepared = destination(&path)
            .prepare()
            .expect("public destination admission");
        let archive = client
            .export_session(&session())
            .await
            .expect("completed public fixture archive");
        client.close().await;
        assert_eq!(
            stale.export_session(&session()).await.unwrap_err(),
            Error::Closed,
            "revoked input transport grants no new archive"
        );
        assert_eq!(
            prepared.save(archive),
            Ok(Saved { bytes: fixture.len }),
            "completed archive remains a separately owned save capability"
        );
        let _public_bytes = read_public_archive(&path, fixture.len);
        fixture.stop().await;
    }
}
