use dsh_native_transport::{
    Error, Limits, NativeClient, SecretLaunchUrl, dto::SessionId, prompt_admission::*,
};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader, Lines},
    process::{Child, ChildStdout, Command},
};
struct Fixture {
    child: Child,
    output: Lines<BufReader<ChildStdout>>,
}
impl Fixture {
    async fn start(mode: &str) -> (Self, NativeClient) {
        let mut child = Command::new("/usr/bin/node")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/support/prompt_recovery_fixture.mjs"
            ))
            .arg(mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap()).lines();
        let launch = tokio::time::timeout(Duration::from_secs(5), output.next_line())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let client = NativeClient::connect(
            SecretLaunchUrl::new(launch).unwrap(),
            Limits {
                max_concurrent_rpc: 4,
                timeout: Duration::from_secs(10),
                ..Limits::default()
            },
        )
        .await
        .unwrap();
        (Self { child, output }, client)
    }
    async fn stop(&mut self) {
        self.child.start_kill().unwrap();
        self.child.wait().await.unwrap();
    }
}
fn request() -> PromptAdmissionRequest {
    PromptAdmissionRequest {
        session_id: SessionId::new("PUBLIC session").unwrap(),
        request_id: dsh_native_transport::dto::SessionRequestId::new("PUBLIC original request")
            .unwrap(),
    }
}
#[tokio::test]
async fn independent_stop_lane_reaches_authenticated_http_while_four_normal_rpcs_are_held() {
    let (mut fixture, client) = Fixture::start("held").await;
    let mut normal = tokio::task::JoinSet::new();
    for _ in 0..4 {
        let client = client.clone();
        normal.spawn(async move { client.session_list().await });
    }
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), fixture.output.next_line())
            .await
            .unwrap()
            .unwrap()
            .as_deref(),
        Some("HELD4")
    );
    let accepted = tokio::time::timeout(
        Duration::from_secs(2),
        client.session_cancel(&SessionId::new("PUBLIC session").unwrap()),
    )
    .await
    .expect("Stop must not wait for the four held normal RPC permits")
    .unwrap();
    assert!(accepted.accepted);
    while let Some(result) = normal.join_next().await {
        assert_eq!(
            result.unwrap().unwrap().items[0].cwd.as_deref(),
            Some("retained:1;cancels:1")
        );
    }
    client.close().await;
    fixture.stop().await;
}
#[tokio::test]
async fn cold_admission_identity_read_preserves_fixed_states_and_rejects_unverified_results() {
    for mode in [
        "normal",
        "unknown",
        "admitting",
        "rejected",
        "internal",
        "secret",
        "unsafe",
        "invalid",
        "missing",
    ] {
        let (mut fixture, client) = Fixture::start(mode).await;
        let result = client.session_prompt_admission(request()).await;
        match mode {
            "normal" => assert!(matches!(
                result,
                Ok(PromptAdmission::Accepted {
                    evidence: AdmissionEvidence::Journal,
                    phase: AdmissionPhase::Recorded,
                    seq: Some(0),
                    ..
                })
            )),
            "unknown" => assert_eq!(result, Ok(PromptAdmission::Unknown)),
            "admitting" => assert_eq!(result, Ok(PromptAdmission::Admitting)),
            "rejected" => assert_eq!(
                result,
                Ok(PromptAdmission::Rejected(AdmissionRejection::AgentBusy))
            ),
            "internal" => assert_eq!(
                result,
                Ok(PromptAdmission::Rejected(AdmissionRejection::Internal))
            ),
            _ => {
                assert_eq!(result, Err(Error::InvalidDto));
                assert!(!format!("{result:?}").contains("PRIVATE"));
            }
        }
        client.close().await;
        fixture.stop().await;
    }
}
