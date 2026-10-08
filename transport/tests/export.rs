use dsh_native_transport::{Error, Limits, NativeClient, SecretLaunchUrl, dto::SessionId};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdout, Command},
    time::{Instant, timeout},
};

fn session() -> SessionId {
    SessionId::new("session / opaque:%?&=💾").unwrap()
}
fn limits(bytes: usize, millis: u64) -> Limits {
    Limits {
        max_result_bytes: bytes,
        max_concurrent_rpc: 1,
        timeout: Duration::from_millis(millis),
        ..Limits::default()
    }
}

async fn bounded_line(reader: &mut BufReader<ChildStdout>) -> String {
    let mut bytes = Vec::new();
    let count = timeout(
        Duration::from_secs(5),
        (&mut *reader).take(16_385).read_until(b'\n', &mut bytes),
    )
    .await
    .expect("fixture line timeout")
    .expect("fixture line read");
    assert!(
        count <= 16_384 && bytes.last() == Some(&b'\n'),
        "fixture line bound or EOF"
    );
    bytes.pop();
    String::from_utf8(bytes).unwrap_or_else(|_| panic!("fixture line encoding"))
}

/// Local HTTP only; stdin shutdown is awaited, and panic-drop kills the owned Node child.
struct Fixture {
    child: Child,
    lines: BufReader<ChildStdout>,
    launch: Option<SecretLaunchUrl>,
    len: usize,
}
impl Fixture {
    async fn start(mode: &str) -> Self {
        let mut child = Command::new("/usr/bin/node")
            .arg(format!(
                "{}/tests/export_fixture.mjs",
                env!("CARGO_MANIFEST_DIR")
            ))
            .arg(mode)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .expect("start local export fixture");
        let mut lines = BufReader::new(child.stdout.take().unwrap());
        let url = bounded_line(&mut lines).await;
        let launch = Some(
            SecretLaunchUrl::new(url).unwrap_or_else(|_| panic!("private fixture launch rejected")),
        );
        let length = bounded_line(&mut lines).await;
        let len = length.strip_prefix("length:").unwrap().parse().unwrap();
        Self {
            child,
            lines,
            launch,
            len,
        }
    }
    async fn connect(&mut self, limits: Limits) -> NativeClient {
        NativeClient::connect(self.launch.take().unwrap(), limits)
            .await
            .unwrap_or_else(|e| panic!("connect fixture: {e}"))
    }
    async fn event(&mut self, expected: &str) {
        timeout(Duration::from_secs(3), async {
            loop {
                let event = bounded_line(&mut self.lines).await;
                if event == expected {
                    break;
                }
                assert!(
                    event.starts_with("export:")
                        || event.starts_with("rpc:")
                        || event.starts_with("closed:"),
                    "unexpected fixture metadata"
                );
            }
        })
        .await
        .expect("fixture event timeout");
    }
    async fn stop(mut self) {
        self.child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"stop\n")
            .await
            .unwrap();
        let status = timeout(Duration::from_secs(3), self.child.wait())
            .await
            .expect("fixture shutdown timeout")
            .unwrap();
        assert!(status.success(), "fixture wire assertion failed");
    }
}

#[tokio::test]
async fn exact_private_encoded_get_false_and_consuming_archive_for_stored_and_streamed_zip() {
    for mode in [
        "stored",
        "chunked",
        "deflate-chunked",
        "deflate-level0",
        "type-params",
        "zip-comment",
        "multi-entry",
        "unicode-name",
        "regular-attrs",
        "different-times",
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        let archive = client.export_session(&session()).await.unwrap();
        assert_eq!(archive.len(), fixture.len);
        assert!(!archive.is_empty());
        let debug = format!("{archive:?}");
        assert!(!debug.contains("PRIVATE"));
        assert!(!debug.contains("cookie"));
        let mut bytes = Vec::new();
        assert_eq!(archive.write_to(&mut bytes).unwrap(), fixture.len);
        assert!(bytes.starts_with(b"PK\x03\x04"));
        assert!(
            !bytes
                .windows(b"PRIVATE_NAME".len())
                .any(|w| w == b"PRIVATE_NAME")
        );
        assert!(
            !bytes
                .windows(b"PRIVATE_RESPONSE_TOKEN".len())
                .any(|w| w == b"PRIVATE_RESPONSE_TOKEN")
        );
        if mode != "deflate-chunked" {
            assert!(
                bytes
                    .windows(b"PRIVATE_ROOT_LOG".len())
                    .any(|w| w == b"PRIVATE_ROOT_LOG")
            );
        }
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn only_exact_200_is_accepted_and_raw_failures_and_redirects_are_not_forwarded() {
    for (mode, status) in [
        ("status-201", 201),
        ("status-204", 204),
        ("status-206", 206),
        ("status-404", 404),
        ("status-500", 500),
        ("redirect", 302),
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        let error = client.export_session(&session()).await.unwrap_err();
        assert_eq!(error, Error::HttpStatus(status));
        assert_eq!(error.code(), "http-status");
        assert_eq!(error.to_string(), "native transport: http-status");
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn wrong_or_missing_type_duplicate_type_encoding_and_html_fail_closed() {
    for mode in [
        "html-type",
        "json-type",
        "missing-type",
        "duplicate-type",
        "encoded",
        "html-as-zip",
        "pk-only",
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        let error = client.export_session(&session()).await.unwrap_err();
        assert_eq!(error, Error::InvalidFrame, "{mode}");
        assert_eq!(error.to_string(), "native transport: invalid-frame");
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn complete_classic_zip_structure_is_required_not_just_a_pk_signature() {
    for mode in [
        "bad-footer",
        "bad-count",
        "bad-directory-size",
        "bad-directory-offset",
        "bad-central-header",
        "bad-local-offset",
        "bad-local-name",
        "bad-compressed-size",
        "bad-descriptor",
        "local-nonzero-crc",
        "local-nonzero-compressed",
        "local-nonzero-uncompressed",
        "multi-disk",
        "zip64",
        "encrypted",
        "bad-method",
        "bad-version",
        "bad-local-version",
        "extra-field",
        "local-extra-field",
        "prefixed",
        "trailing",
        "truncated-zip",
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        assert_eq!(
            client.export_session(&session()).await.unwrap_err(),
            Error::InvalidFrame,
            "{mode}"
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn root_only_alpha_export_profile_rejects_unsafe_duplicate_descendant_or_nonregular_entries()
{
    for mode in [
        "absolute-name",
        "backslash-name",
        "control-name",
        "del-name",
        "unicode-control-name",
        "dot-name",
        "dotdot-name",
        "empty-component",
        "directory-name",
        "descendant-name",
        "unsupported-name",
        "duplicate-root",
        "duplicate-attachment",
        "wrong-root-generation",
        "missing-root",
        "directory-attrs",
        "volume-attrs",
        "symlink-attrs",
        "special-attrs",
        "invalid-utf8",
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        assert_eq!(
            client.export_session(&session()).await.unwrap_err(),
            Error::InvalidFrame,
            "{mode}"
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn advertised_uncompressed_entry_and_total_ceilings_are_metadata_only_not_decompression_validation()
 {
    for mode in [
        "advertised-entry-exact",
        "advertised-entry-over",
        "advertised-total-exact",
        "advertised-total-over",
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        let result = client.export_session(&session()).await;
        if mode.ends_with("over") {
            assert_eq!(result.unwrap_err(), Error::Oversize, "{mode}");
        } else {
            assert_eq!(result.unwrap().len(), fixture.len);
        }
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn container_acceptance_does_not_claim_crc_or_actual_deflate_contents_were_verified() {
    for mode in ["unverified-crc", "deflate-unverified-content"] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(Limits::default()).await;
        assert_eq!(
            client.export_session(&session()).await.unwrap().len(),
            fixture.len
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn exact_small_known_and_streamed_limits_pass_and_one_byte_less_is_oversize() {
    for mode in ["stored", "chunked"] {
        for exact in [true, false] {
            let mut fixture = Fixture::start(mode).await;
            let bound = fixture.len - usize::from(!exact);
            let client = fixture.connect(limits(bound, 1000)).await;
            let result = client.export_session(&session()).await;
            if exact {
                assert_eq!(result.unwrap().len(), bound);
            } else {
                assert_eq!(result.unwrap_err(), Error::Oversize);
            }
            client.close().await;
            fixture.stop().await;
        }
    }
}

#[tokio::test]
async fn oversize_chunk_is_rejected_before_extending_owned_download() {
    let mut fixture = Fixture::start("chunked-over").await;
    let client = fixture.connect(limits(64, 1000)).await;
    assert_eq!(
        client.export_session(&session()).await.unwrap_err(),
        Error::Oversize
    );
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn hard_32_mib_ceiling_applies_to_content_length_and_streamed_count_even_with_larger_owner_limits()
 {
    for mode in ["ceiling-known", "ceiling-chunked"] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(limits(64 * 1024 * 1024, 3000)).await;
        assert_eq!(
            client.export_session(&session()).await.unwrap_err(),
            Error::Oversize
        );
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn http_truncation_is_fixed_network_failure_while_cleanly_ended_partial_zip_is_invalid_frame()
{
    for (mode, expected) in [
        ("truncated-http", Error::Network),
        ("truncated-zip", Error::InvalidFrame),
    ] {
        let mut fixture = Fixture::start(mode).await;
        let client = fixture.connect(limits(1024, 1000)).await;
        let error = client.export_session(&session()).await.unwrap_err();
        assert_eq!(error, expected);
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        client.close().await;
        fixture.stop().await;
    }
}

#[tokio::test]
async fn deadline_covers_slow_download_and_releases_shared_capacity() {
    let mut fixture = Fixture::start("slow").await;
    let client = fixture.connect(limits(1024, 150)).await;
    assert_eq!(
        client.export_session(&session()).await.unwrap_err(),
        Error::Timeout
    );
    // A distinct existing RPC gets the same released capacity, without export retry.
    assert!(client.session_list().await.unwrap().items.is_empty());
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn queue_wait_consumes_the_same_deadline_as_the_second_download() {
    let mut fixture = Fixture::start("queue-budget").await;
    let client = fixture.connect(limits(1024, 300)).await;
    let first_client = client.clone();
    let first = tokio::spawn(async move { first_client.export_session(&session()).await });
    fixture.event("export:1").await;
    let start = Instant::now();
    assert_eq!(
        client.export_session(&session()).await.unwrap_err(),
        Error::Timeout
    );
    assert!(start.elapsed() < Duration::from_millis(450));
    assert_eq!(first.await.unwrap().unwrap().len(), fixture.len);
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn queued_export_times_out_before_it_can_send_any_get_behind_existing_rpc_queue() {
    let mut fixture = Fixture::start("rpc-delay").await;
    let client = fixture.connect(limits(1024, 140)).await;
    let c = client.clone();
    let first = tokio::spawn(async move { c.session_list().await });
    fixture.event("rpc:1").await;
    let c = client.clone();
    let second = tokio::spawn(async move { c.session_list().await });
    tokio::task::yield_now().await;
    let c = client.clone();
    let third = tokio::spawn(async move { c.session_list().await });
    tokio::task::yield_now().await;
    assert_eq!(
        client.export_session(&session()).await.unwrap_err(),
        Error::Timeout
    );
    first.await.unwrap().unwrap();
    second.await.unwrap().unwrap();
    third.await.unwrap().unwrap();
    assert_eq!(
        client.export_session(&session()).await.unwrap().len(),
        fixture.len
    );
    // If the timed-out queued export leaked a GET, the next metadata event would be 2.
    fixture.event("export:1").await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn close_cancels_inflight_and_queued_exports_immediately_and_stale_clones_are_closed() {
    let mut fixture = Fixture::start("hold-all").await;
    let client = fixture.connect(limits(1024, 5000)).await;
    let stale = client.clone();
    let c = client.clone();
    let first = tokio::spawn(async move { c.export_session(&session()).await });
    fixture.event("export:1").await;
    let c = client.clone();
    let second = tokio::spawn(async move { c.export_session(&session()).await });
    tokio::task::yield_now().await;
    timeout(Duration::from_millis(300), client.close())
        .await
        .expect("close cancels download and queue");
    assert_eq!(first.await.unwrap().unwrap_err(), Error::Closed);
    assert_eq!(second.await.unwrap().unwrap_err(), Error::Closed);
    assert_eq!(
        stale.export_session(&session()).await.unwrap_err(),
        Error::Closed
    );
    fixture.stop().await;
}

#[tokio::test]
async fn export_and_existing_rpc_share_capacity_and_dropping_download_releases_it() {
    let mut fixture = Fixture::start("hold-first").await;
    let client = fixture.connect(limits(1024, 5000)).await;
    let c = client.clone();
    let first = tokio::spawn(async move { c.export_session(&session()).await });
    fixture.event("export:1").await;
    let c = client.clone();
    let mut rpc = tokio::spawn(async move { c.session_list().await });
    assert!(
        timeout(Duration::from_millis(50), &mut rpc).await.is_err(),
        "RPC must wait behind export"
    );
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    timeout(Duration::from_millis(300), &mut rpc)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    fixture.event("rpc:1").await;
    assert_eq!(
        client.export_session(&session()).await.unwrap().len(),
        fixture.len
    );
    fixture.event("export:2").await;
    client.close().await;
    fixture.stop().await;
}

#[tokio::test]
async fn close_cancels_export_queued_behind_an_existing_rpc() {
    let mut fixture = Fixture::start("rpc-hold").await;
    let client = fixture.connect(limits(1024, 5000)).await;
    let c = client.clone();
    let rpc = tokio::spawn(async move { c.session_list().await });
    fixture.event("rpc:1").await;
    let c = client.clone();
    let export = tokio::spawn(async move { c.export_session(&session()).await });
    tokio::task::yield_now().await;
    timeout(Duration::from_millis(300), client.close())
        .await
        .unwrap();
    assert_eq!(rpc.await.unwrap().unwrap_err(), Error::Closed);
    assert_eq!(export.await.unwrap().unwrap_err(), Error::Closed);
    fixture.stop().await;
}
