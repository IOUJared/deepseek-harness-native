use super::*;
use crate::{Limits, SecretLaunchUrl};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdout, Command},
    time::{Instant, timeout},
};
fn session() -> SessionId {
    SessionId::new("session / opaque:%?&=💾").unwrap()
}
fn payload() -> FileUpload {
    FileUpload::new(vec![0, 255, 128, 10], Some("PUBLIC report é.txt".into())).unwrap()
}
async fn line(reader: &mut BufReader<ChildStdout>) -> String {
    let mut bytes = Vec::new();
    let count = timeout(
        Duration::from_secs(3),
        (&mut *reader).take(16385).read_until(b'\n', &mut bytes),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(count <= 16384 && bytes.last() == Some(&b'\n'));
    bytes.pop();
    String::from_utf8(bytes).unwrap()
}
struct Fixture {
    child: Child,
    lines: BufReader<ChildStdout>,
    client: NativeClient,
}
impl Fixture {
    async fn new(mode: &str, millis: u64) -> Self {
        let mut child = Command::new("/usr/bin/node")
            .arg(format!(
                "{}/tests/upload_fixture.mjs",
                env!("CARGO_MANIFEST_DIR")
            ))
            .arg(mode)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut lines = BufReader::new(child.stdout.take().unwrap());
        let client = NativeClient::connect(
            SecretLaunchUrl::new(line(&mut lines).await).unwrap(),
            Limits {
                max_concurrent_rpc: 1,
                timeout: Duration::from_millis(millis),
                ..Limits::default()
            },
        )
        .await
        .unwrap();
        Self {
            child,
            lines,
            client,
        }
    }
    async fn received(&mut self) {
        loop {
            let event = line(&mut self.lines).await;
            if event.starts_with("upload:") {
                return;
            }
            assert!(event.starts_with("closed:"));
        }
    }
    async fn closed_first(&mut self) {
        loop {
            let event = line(&mut self.lines).await;
            if event == "closed:1" {
                return;
            }
            assert!(event.starts_with("upload:") || event.starts_with("closed:"));
        }
    }
    async fn stop(mut self) {
        self.client.close().await;
        self.child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b"stop\n")
            .await
            .unwrap();
        assert!(
            timeout(Duration::from_secs(3), self.child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
}
#[tokio::test]
async fn raw_private_cookie_origin_percent_encoded_target_empty_and_exact_body_ceiling() {
    for mode in ["good", "empty", "max"] {
        let f = Fixture::new(mode, 5000).await;
        let file = match mode {
            "empty" => FileUpload::new(Vec::new(), None).unwrap(),
            "max" => FileUpload::new(
                vec![0xab; MAX_UPLOAD_BYTES],
                Some("PUBLIC report é.txt".into()),
            )
            .unwrap(),
            _ => payload(),
        };
        let length = file.len();
        let result = f.client.upload_file(&session(), file).await.unwrap();
        assert_eq!(result.file.bytes, length as u64);
        f.stop().await;
    }
}
#[tokio::test]
async fn receipt_http_and_complete_json_failures_never_return_remote_bodies() {
    for (mode, expected) in [
        ("wrong-bytes", Error::InvalidDto),
        ("partial", Error::InvalidDto),
        ("extra", Error::InvalidDto),
        ("remote", Error::RemoteFailure),
        ("bad-error", Error::InvalidFrame),
        ("extra-error", Error::InvalidFrame),
        ("too-many-items", Error::Oversize),
        ("invalid-json", Error::InvalidJson),
        ("http", Error::HttpStatus(401)),
        ("redirect", Error::HttpStatus(303)),
        ("wrong-type", Error::InvalidFrame),
        ("oversize", Error::Oversize),
        ("chunked-over", Error::Oversize),
    ] {
        let f = Fixture::new(mode, 1000).await;
        let error = f
            .client
            .upload_file(&session(), payload())
            .await
            .unwrap_err();
        assert_eq!(error, expected, "{mode}");
        assert!(!format!("{error:?} {error}").contains("PRIVATE"));
        f.stop().await;
    }
}
#[tokio::test]
async fn whole_deadline_includes_semaphore_wait_and_abandoned_future_releases_slot() {
    let f = Fixture::new("good", 100).await;
    let permit = f.client.inner.rpc_slots.acquire().await.unwrap();
    let started = Instant::now();
    let error = timeout(
        Duration::from_secs(1),
        f.client.upload_file(&session(), payload()),
    )
    .await
    .unwrap()
    .unwrap_err();
    assert_eq!(error, Error::Timeout);
    assert!(started.elapsed() < Duration::from_millis(750));
    drop(permit);
    assert!(f.client.upload_file(&session(), payload()).await.is_ok());
    f.stop().await;
    let mut f = Fixture::new("hold-first", 3000).await;
    let c = f.client.clone();
    let first = tokio::spawn(async move { c.upload_file(&session(), payload()).await });
    f.received().await;
    let selected = session();
    let queued_client = f.client.clone();
    let mut queued = Box::pin(queued_client.upload_file(&selected, payload()));
    assert!(futures_util::poll!(queued.as_mut()).is_pending());
    drop(queued);
    first.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    f.closed_first().await;
    assert!(
        timeout(
            Duration::from_secs(1),
            f.client.upload_file(&session(), payload())
        )
        .await
        .unwrap()
        .is_ok()
    );
    assert_eq!(f.client.inner.rpc_slots.available_permits(), 1);
    f.stop().await;
}
#[tokio::test]
async fn owned_upload_worker_admission_is_bounded_before_shared_rpc_queueing() {
    let f = Fixture::new("good", 3000).await;
    let permit = f.client.inner.rpc_slots.acquire().await.unwrap();
    let selected = session();
    let mut pending = Vec::new();
    for _ in 0..MAX_UPLOAD_JOBS {
        let mut future = Box::pin(f.client.upload_file(&selected, payload()));
        assert!(futures_util::poll!(future.as_mut()).is_pending());
        pending.push(future);
    }
    assert_eq!(
        f.client
            .upload_file(&selected, payload())
            .await
            .unwrap_err(),
        Error::QueueFull
    );
    drop(pending);
    drop(permit);
    timeout(Duration::from_secs(1), f.client.close())
        .await
        .unwrap();
    assert_eq!(
        f.client.inner.upload_slots.available_permits(),
        MAX_UPLOAD_JOBS
    );
    f.stop().await;
}
#[tokio::test]
async fn parked_admitted_future_does_not_block_close_or_return_stale_receipt() {
    let mut f = Fixture::new("hold-all", 3000).await;
    let client = f.client.clone();
    let selected = session();
    let mut parked = Box::pin(client.upload_file(&selected, payload()));
    tokio::select! { result = &mut parked => panic!("unexpected early result: {result:?}"), _ = f.received() => {} }
    // Caller deliberately stops polling an already admitted operation while closing its owner.
    timeout(Duration::from_secs(1), client.close())
        .await
        .unwrap();
    f.closed_first().await;
    assert_eq!(parked.await.unwrap_err(), Error::Closed);
    assert_eq!(client.inner.rpc_slots.available_permits(), 1);
    f.stop().await;
}
#[tokio::test]
async fn scheduling_after_expiry_never_sends_and_completed_parked_success_is_revoked() {
    let f = Fixture::new("no-upload", 100).await;
    let selected = session();
    let mut parked = Box::pin(f.client.upload_file(&selected, payload()));
    assert!(futures_util::poll!(parked.as_mut()).is_pending());
    // Deliberately stall this single-thread test runtime before the worker's first poll.
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(parked.await.unwrap_err(), Error::Timeout);
    f.stop().await;
    let mut f = Fixture::new("good", 3000).await;
    let c = f.client.clone();
    let selected = session();
    let mut parked = Box::pin(c.upload_file(&selected, payload()));
    assert!(futures_util::poll!(parked.as_mut()).is_pending());
    f.received().await;
    let slots = timeout(
        Duration::from_secs(1),
        c.inner.upload_slots.acquire_many(MAX_UPLOAD_JOBS as u32),
    )
    .await
    .unwrap()
    .unwrap();
    // Good wire completion released its job slot without cancellation, timeout or caller consumption.
    drop(slots);
    timeout(Duration::from_secs(1), c.close()).await.unwrap();
    assert_eq!(parked.await.unwrap_err(), Error::Closed);
    f.stop().await;
}
#[tokio::test]
async fn held_receipt_deadline_explicit_invalidation_and_exact_result_byte_limit() {
    let mut f = Fixture::new("hold-all", 150).await;
    assert_eq!(
        f.client
            .upload_file(&session(), payload())
            .await
            .unwrap_err(),
        Error::Timeout
    );
    f.closed_first().await;
    f.stop().await;
    let mut f = Fixture::new("hold-all", 3000).await;
    let c = f.client.clone();
    let active = tokio::spawn(async move { c.upload_file(&session(), payload()).await });
    f.received().await;
    f.client.invalidate();
    assert_eq!(
        timeout(Duration::from_secs(1), active)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err(),
        Error::Closed
    );
    f.closed_first().await;
    f.stop().await;
    let f = Fixture::new("exact-result", 1000).await;
    assert!(f.client.upload_file(&session(), payload()).await.is_ok());
    f.stop().await;
}
#[tokio::test]
async fn owner_close_revokes_inflight_queued_and_stale_clone_uploads() {
    let mut f = Fixture::new("hold-all", 3000).await;
    let c = f.client.clone();
    let first = tokio::spawn(async move { c.upload_file(&session(), payload()).await });
    f.received().await;
    let selected = session();
    let queued_client = f.client.clone();
    let mut queued = Box::pin(queued_client.upload_file(&selected, payload()));
    assert!(futures_util::poll!(queued.as_mut()).is_pending());
    timeout(Duration::from_secs(1), f.client.close())
        .await
        .unwrap();
    f.closed_first().await;
    assert_eq!(
        timeout(Duration::from_secs(1), first)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err(),
        Error::Closed
    );
    assert_eq!(
        timeout(Duration::from_secs(1), queued)
            .await
            .unwrap()
            .unwrap_err(),
        Error::Closed
    );
    assert_eq!(
        f.client
            .upload_file(&session(), payload())
            .await
            .unwrap_err(),
        Error::Closed
    );
    assert_eq!(f.client.inner.rpc_slots.available_permits(), 1);
    f.stop().await;
}
