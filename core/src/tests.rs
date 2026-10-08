use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

#[cfg(feature = "transport")]
#[path = "agent_discovery_tests.rs"]
mod agent_discovery_tests;
#[path = "secret_tests.rs"]
mod secret_tests;

#[cfg(feature = "transport")]
const HTTP_FIXTURE: &str = r#"
import http from 'node:http';
import crypto from 'node:crypto';
const send = value => process.stdout.write('DSH_TAURI:' + JSON.stringify(value) + '\n');
let cookieName;
const server = http.createServer((request, response) => {
  if (request.method === 'GET') {
    const exchange = () => {
      response.writeHead(303, { location: './', 'set-cookie': `${cookieName}=PUBLIC_FAKE_COOKIE; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600; Expires=${new Date(Date.now()+3600000).toUTCString()}` });
      response.end();
    };
    if (DELAY_EXCHANGE) setTimeout(exchange, 5000); else exchange();
  } else {
    response.writeHead(500); response.end();
  }
});
server.listen(0, '127.0.0.1', () => {
  const authority = `127.0.0.1:${server.address().port}`;
  cookieName = `dsh-auth-${crypto.createHash('sha256').update(authority).digest('base64url')}`;
  send({type:'ready', url:`http://${authority}/?token=PUBLIC_FAKE_LAUNCH`, version:'0.2.1-alpha.1', profile:'desktop'});
});
process.on('SIGTERM', () => {});
let pending = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => {
  pending += chunk;
  while (pending.includes('\n')) {
    const at = pending.indexOf('\n'); const command = JSON.parse(pending.slice(0,at)); pending = pending.slice(at+1);
    if (command.type === 'inspect') process.stdout.write('DSH_TAURI:{broken\n');
  }
});
"#;

#[cfg(feature = "transport")]
#[tokio::test]
async fn monitor_fault_revokes_client_before_root_reap() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&HTTP_FIXTURE.replace("DELAY_EXCHANGE", "false"));
    backend.wait_ready().unwrap();
    let client = backend
        .connect_transport(dsh_native_transport::Limits::default())
        .await
        .unwrap();
    let _ = backend.request(Command::Inspect, Duration::from_millis(50));
    tokio::time::timeout(Duration::from_secs(1), async {
        while !backend.inner.stopping.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        client.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    client.close().await;
    assert!(backend.stop().unwrap().exited);
}

#[cfg(feature = "transport")]
#[tokio::test]
async fn core_stop_cancels_pending_private_auth_exchange() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&HTTP_FIXTURE.replace("DELAY_EXCHANGE", "true"));
    backend.wait_ready().unwrap();
    let backend = Arc::new(backend);
    let copy = backend.clone();
    let pending = tokio::spawn(async move {
        copy.connect_transport(dsh_native_transport::Limits {
            timeout: Duration::from_secs(60),
            ..Default::default()
        })
        .await
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let stopped = backend.stop_async();
    let answer = tokio::time::timeout(Duration::from_secs(1), pending)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        answer,
        Err(ConnectError::Backend(Error::Stopping))
    ));
    drop(stopped);
    assert!(backend.stop().unwrap().exited);
}

const NORMAL: &str = r#"
import fs from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const dir = dirname(fileURLToPath(import.meta.url));
const privateStage = (fs.statSync(dir).mode & 0o777) === 0o700
  && ['fork-supervisor.mjs', 'supervisor.mjs'].every(n => fs.existsSync(join(dir,n)) && (fs.statSync(join(dir,n)).mode & 0o777) === 0o600);
const isolated = !process.env.DEEPSEEK_API_KEY && !process.env.NODE_OPTIONS && !process.env.DSH_HOME
  && process.env.DSH_TAURI_RUNTIME && process.env.DSH_TAURI_HOME && process.env.DSH_TAURI_EXPECTED_VERSION === '0.2.1-alpha.1';
const send = v => process.stdout.write('DSH_TAURI:' + JSON.stringify(v) + '\n');
fs.mkdirSync(process.env.DSH_TAURI_HOME, { recursive: true });
fs.writeFileSync(join(process.env.DSH_TAURI_HOME, 'fixture-pid'), String(process.pid));
send({type:'platform-session',session:{origin:'https://platform.example',token:'PUBLIC_FAKE_PLATFORM',requestHeaders:{'X-Fixture':'PUBLIC_FAKE_HEADER'}}});
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
process.stdout.write('unframed PUBLIC_FAKE_SECRET\n');
process.stderr.write('arbitrary PUBLIC_FAKE_SECRET\n');
send({type:'unrecognized',value:'PUBLIC_FAKE_SECRET'});
send({type:'account-state',state:{status:'signed-out',attempt:null,tokenValue:'PUBLIC_FAKE_SECRET'}});
let pending='';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => {
 pending += chunk;
 while(pending.includes('\n')) {
  const at=pending.indexOf('\n'); const c=JSON.parse(pending.slice(0,at)); pending=pending.slice(at+1);
  if(c.type==='shutdown'){send({type:'platform-session',session:null});send({type:'shutdown-complete'});process.exit(0);}
  if(c.type==='inspect') send({type:'inspection',requestId:c.requestId,activeTasks:0,scheduledTasks:0,unknown:!privateStage || !isolated});
  else if(c.type==='open-workspace') send({type:'workspace-added',requestId:c.requestId});
  else {
   const value=c.type==='onboarding-read' ? {loggedIn:false,hasApiKey:false,writable:true,tokenValue:'PUBLIC_FAKE_SECRET'}
    : c.type==='cancel-generation' ? {cancelled:true,tokenValue:'PUBLIC_FAKE_SECRET'}
    : c.type==='account-unsubscribe' ? {subscribed:false,tokenValue:'PUBLIC_FAKE_SECRET'}
    : {status:'credential-stored',attempt:{phase:'waiting-browser',id:'PUBLIC_FAKE_SECRET',authorizeUrl:'https://platform.example/?token=PUBLIC_FAKE_SECRET'},token:'PUBLIC_FAKE_SECRET'};
   send({type:'native-result',requestId:c.requestId,command:c.type,value});
  }
 }
});
"#;
const READY: &str = "process.stdout.write('DSH_TAURI:' + JSON.stringify({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'}) + '\\n');";

struct Fixture {
    directory: tempfile::TempDir,
    options: RustBackendOptions,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let runtime = directory.path().join("runtime");
        let user = directory.path().join("user");
        fs::create_dir(&runtime).unwrap();
        fs::create_dir(&user).unwrap();
        fs::write(
            runtime.join("package.json"),
            r#"{"name":"@deepseek-ai/dsh","version":"0.2.1-alpha.1"}"#,
        )
        .unwrap();
        let options = RustBackendOptions {
            runtime,
            expected_version: "0.2.1-alpha.1".into(),
            native_home: directory.path().join("native-home"),
            user_home: user,
            working_directory: directory.path().to_owned(),
            absolute_node_path: Some("/usr/bin/node".into()),
            startup_timeout: Duration::from_secs(2),
            stop_policy: StopPolicy {
                graceful: Duration::from_millis(120),
                terminate: Duration::from_millis(120),
                kill: Duration::from_millis(200),
            },
        };
        Self { directory, options }
    }
    fn spawn(&self, script: &str) -> (Backend, mpsc::Receiver<PublicEvent>) {
        Backend::spawn(self.options.validate().unwrap(), Some(script)).unwrap()
    }
    fn root_pid(&self) -> i32 {
        fs::read_to_string(self.options.native_home.join("fixture-pid"))
            .unwrap()
            .parse()
            .unwrap()
    }
}
fn ready_script(suffix: &str) -> String {
    format!("{}\n{suffix}", READY)
}
fn parse(value: Value) -> Result<protocol::Event, Error> {
    let mut bytes = protocol::PREFIX.to_vec();
    bytes.extend(serde_json::to_vec(&value).unwrap());
    protocol::parse(&bytes, "0.2.1-alpha.1")
}

#[test]
fn stages_both_helpers_and_explicit_isolated_environment() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(NORMAL);
    backend.wait_ready().unwrap();
    assert_eq!(
        backend
            .request(Command::Inspect, Duration::from_secs(1))
            .unwrap(),
        PublicReply::Inspection(Inspection {
            active_tasks: 0,
            scheduled_tasks: 0,
            unknown: false
        })
    );
    let root = process::identity(fixture.root_pid()).unwrap();
    assert_eq!(root.group, root.pid);
    assert!(backend.stop().unwrap().graceful);
    assert!(process::identity(root.pid).is_none());
    assert!(backend.with_private_endpoint(|_, _| ()).is_none());
    assert!(backend.with_private_account_session(|_, _, _| ()).is_none());
}

#[test]
fn short_lived_startup_thread_does_not_trigger_pdeathsig() {
    let fixture = Fixture::new();
    let options = fixture.options.validate().unwrap();
    let (backend, _) = thread::spawn(move || Backend::spawn(options, Some(NORMAL)).unwrap())
        .join()
        .unwrap();
    backend.wait_ready().unwrap();
    assert!(
        backend
            .request(Command::Inspect, Duration::from_secs(1))
            .is_ok()
    );
    assert!(backend.stop().unwrap().graceful);
}

#[test]
fn public_events_and_results_never_contain_private_values() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(NORMAL);
    backend.wait_ready().unwrap();
    assert!(
        backend
            .with_private_endpoint(|origin, launch| origin == "http://127.0.0.1:49876"
                && launch.contains("PUBLIC_FAKE_LAUNCH"))
            .unwrap()
    );
    assert!(
        backend
            .with_private_account_session(
                |_, token, headers| token == "PUBLIC_FAKE_PLATFORM" && headers.len() == 1
            )
            .unwrap()
    );
    for command in [
        Command::AccountState,
        Command::AccountSubscribe,
        Command::AccountUnsubscribe,
        Command::OnboardingRead,
        Command::CancelGeneration("fixture-session".into()),
        Command::OpenWorkspace(fixture.directory.path().to_owned()),
    ] {
        let reply = backend.request(command, Duration::from_secs(1)).unwrap();
        let public = format!("{reply:?}");
        assert!(!public.contains("PUBLIC_FAKE"));
        assert!(!public.contains("http"));
    }
    backend.stop().unwrap();
    for event in events.try_iter() {
        let public = format!("{event:?}");
        assert!(!public.contains("PUBLIC_FAKE"));
        assert!(!public.contains("http"));
    }
}

#[test]
fn closed_commands_serialize_only_the_supported_fields() {
    for command in [
        Command::Inspect,
        Command::OpenWorkspace("/tmp/public-fixture".into()),
        Command::OnboardingRead,
        Command::AccountState,
        Command::AccountSubscribe,
        Command::AccountUnsubscribe,
        Command::CancelGeneration("fixture-session".into()),
    ] {
        let value = command.encode("42").unwrap();
        assert_eq!(value["type"], command.name());
        assert_eq!(value["requestId"], "42");
        assert_eq!(
            value.as_object().unwrap().len(),
            if matches!(
                command,
                Command::OpenWorkspace(_) | Command::CancelGeneration(_)
            ) {
                3
            } else {
                2
            }
        );
    }
    for command in [
        Command::OpenWorkspace("relative".into()),
        Command::OpenWorkspace("/tmp/../escape".into()),
        Command::CancelGeneration("bad id".into()),
        Command::CancelGeneration("".into()),
    ] {
        assert_eq!(command.encode("42"), Err(Error::InvalidCommand));
    }
}

#[test]
fn validates_home_runtime_node_and_version_without_fallback() {
    let fixture = Fixture::new();
    assert!(fixture.options.validate().is_ok());
    let mut options = fixture.options.clone();
    options.runtime = "relative".into();
    assert!(options.validate().is_err());
    let mut options = fixture.options.clone();
    options.native_home = options.user_home.join(".dsh/child");
    assert!(options.validate().is_err());
    let mut options = fixture.options.clone();
    options.native_home = options.user_home.clone();
    assert!(options.validate().is_err());
    let mut options = fixture.options.clone();
    options.expected_version = "0.2.0-rc.2".into();
    assert!(options.validate().is_err());
    let mut options = fixture.options.clone();
    options.absolute_node_path = Some("node".into());
    assert!(options.validate().is_err());
    let mut options = fixture.options.clone();
    options.absolute_node_path = None;
    assert_eq!(
        options.validate().unwrap().node,
        PathBuf::from("/usr/bin/node").canonicalize().unwrap()
    );
    let mut options = fixture.options.clone();
    options.startup_timeout = Duration::from_secs(31);
    assert!(options.validate().is_err());
    let mut options = fixture.options.clone();
    options.stop_policy.kill = Duration::ZERO;
    assert!(options.validate().is_err());
}

#[test]
fn rejects_symlink_home_alias_of_legacy_home() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.options.user_home.join(".dsh")).unwrap();
    std::os::unix::fs::symlink(
        fixture.options.user_home.join(".dsh"),
        &fixture.options.native_home,
    )
    .unwrap();
    assert!(fixture.options.validate().is_err());
}

#[test]
fn strict_private_ready_url_validation() {
    for url in [
        "https://127.0.0.1:49876/?token=PUBLIC_FAKE",
        "http://localhost:49876/?token=PUBLIC_FAKE",
        "http://127.0.0.1:0/?token=PUBLIC_FAKE",
        "http://user@127.0.0.1:49876/?token=PUBLIC_FAKE",
        "http://127.0.0.1:49876/page?token=PUBLIC_FAKE",
        "http://127.0.0.1:49876/?token=PUBLIC_FAKE#fragment",
        "http://127.0.0.1:49876/?token=PUBLIC_FAKE&token=PUBLIC_FAKE",
        "http://127.0.0.1:49876/?token=",
    ] {
        assert!(
            parse(json!({"type":"ready","url":url,"version":"0.2.1-alpha.1","profile":"desktop"}))
                .is_err()
        );
    }
    assert!(parse(json!({"type":"ready","url":"http://127.0.0.1:49876/?token=PUBLIC_FAKE","version":"0.2.0-rc.2","profile":"desktop"})).is_err());
}

#[test]
fn unknown_inspection_is_not_idle_and_bad_counts_remain_unknown() {
    for data in [
        json!({"activeTasks":0,"scheduledTasks":0,"unknown":true}),
        json!({"activeTasks":0,"scheduledTasks":0}),
        json!({"activeTasks":-1,"scheduledTasks":0,"unknown":false}),
        json!({"activeTasks":10001,"scheduledTasks":0,"unknown":false}),
    ] {
        let mut packet = data;
        packet["type"] = json!("inspection");
        packet["requestId"] = json!("42");
        let protocol::Event::Reply {
            reply: Ok(PublicReply::Inspection(inspection)),
            ..
        } = parse(packet).unwrap()
        else {
            panic!("wrong redacted reply")
        };
        assert!(!inspection.is_known_idle());
        assert!(inspection.unknown);
    }
    assert!(
        Inspection {
            active_tasks: 0,
            scheduled_tasks: 0,
            unknown: false
        }
        .is_known_idle()
    );
}

#[test]
fn early_exit_reaps_without_readiness_or_gui_callback() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn("process.exit(7)");
    let start = Instant::now();
    assert_eq!(backend.wait_ready(), Err(Error::Exited));
    let result = backend.stop().unwrap();
    assert!(result.exited);
    assert!(!result.graceful);
    assert_eq!(result.exit_code, Some(7));
    assert!(result.containment_unknown);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn startup_deadline_queues_background_shutdown() {
    let fixture = Fixture::new();
    let mut options = fixture.options.clone();
    options.startup_timeout = Duration::from_millis(120);
    let (backend, _) = Backend::spawn(
        options.validate().unwrap(),
        Some("setInterval(()=>{},1000)"),
    )
    .unwrap();
    assert_eq!(backend.wait_ready(), Err(Error::RequestTimeout));
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn stalled_child_is_escalated_and_stop_async_is_nonblocking() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&ready_script(
        "process.on('SIGTERM',()=>{});setInterval(()=>{},1000)",
    ));
    backend.wait_ready().unwrap();
    let start = Instant::now();
    let stopped = backend.stop_async();
    assert!(start.elapsed() < Duration::from_millis(100));
    let result = stopped.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(result.exited);
    assert!(!result.graceful);
    assert_eq!(result.observed_descendants_remaining, 0);
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn full_control_pipe_write_has_500ms_deadline_and_poisoned_framing() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&ready_script("setInterval(()=>{},1000)"));
    backend.wait_ready().unwrap();
    let fd = backend.inner.input.lock().unwrap().as_raw_fd();
    assert!(unsafe { libc::fcntl(fd, libc::F_SETPIPE_SZ, 4096) } >= 0);
    let start = Instant::now();
    assert_eq!(
        backend.inner.send(&json!({"payload":"a".repeat(60000)})),
        Err(Error::WriteTimeout)
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    let retry = Instant::now();
    assert_eq!(backend.inner.send_shutdown(), Err(Error::ControlClosed));
    assert!(retry.elapsed() < Duration::from_millis(100));
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn oversized_write_is_rejected_without_poisoning_or_writing() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(NORMAL);
    backend.wait_ready().unwrap();
    assert_eq!(
        backend.inner.send(&json!({"payload":"a".repeat(65536)})),
        Err(Error::CommandTooLarge)
    );
    assert!(!backend.inner.poisoned.load(Ordering::Acquire));
    assert!(backend.stop().unwrap().graceful);
}

#[test]
fn busy_writer_returns_immediately_without_poisoning() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(NORMAL);
    backend.wait_ready().unwrap();
    {
        let _guard = backend.inner.input.lock().unwrap();
        let start = Instant::now();
        assert_eq!(backend.inner.send_shutdown(), Err(Error::WriterBusy));
        assert!(start.elapsed() < Duration::from_millis(100));
        assert!(!backend.inner.poisoned.load(Ordering::Acquire));
    }
    assert!(backend.stop().unwrap().graceful);
}

#[test]
fn request_deadline_removes_pending_response() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&ready_script(
        "process.stdin.resume();setInterval(()=>{},1000)",
    ));
    backend.wait_ready().unwrap();
    assert_eq!(
        backend.request(Command::Inspect, Duration::from_millis(30)),
        Err(Error::RequestTimeout)
    );
    assert!(backend.inner.pending.lock().unwrap().is_empty());
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn oversized_unterminated_stdout_fault_stops_child_without_echo() {
    let fixture = Fixture::new();
    let (backend, events) =
        fixture.spawn("process.stdout.write('x'.repeat(65537));setInterval(()=>{},1000)");
    assert_eq!(backend.wait_ready(), Err(Error::Protocol));
    assert!(backend.stop().unwrap().exited);
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("xxxxx"));
    }
}

#[test]
fn malformed_framed_stdout_and_unterminated_tail_fail_closed() {
    for script in [
        "process.stdout.write('DSH_TAURI:{not-json}\\n');setInterval(()=>{},1000)",
        "process.stdout.write('DSH_TAURI:{');process.exit(0)",
        "process.stdout.write(Buffer.from([68,83,72,95,84,65,85,82,73,58,255,10]));setInterval(()=>{},1000)",
    ] {
        let fixture = Fixture::new();
        let (backend, _) = fixture.spawn(script);
        let ready = backend.wait_ready();
        assert!(ready.is_err());
        assert!(backend.stop().unwrap().exited);
    }
}

#[test]
fn descendant_retaining_stdout_does_not_prevent_reap_or_get_broad_post_exit_signal() {
    let fixture = Fixture::new();
    let script = format!(
        r#"
import {{spawn}} from 'node:child_process';import fs from 'node:fs';import {{join}} from 'node:path';
fs.mkdirSync(process.env.DSH_TAURI_HOME,{{recursive:true}});
fs.writeFileSync(join(process.env.DSH_TAURI_HOME,'fixture-pid'),String(process.pid));
const child=spawn('/usr/bin/node',['-e',"process.on('SIGTERM',()=>{{}});setInterval(()=>{{}},1000)"],{{stdio:['ignore',1,2]}});
fs.writeFileSync(join(process.env.DSH_TAURI_HOME,'descendant-pid'),String(child.pid));
{}
setTimeout(()=>process.exit(9),250);
"#,
        READY
    );
    let (backend, _) = fixture.spawn(&script);
    backend.wait_ready().unwrap();
    let root = fixture.root_pid();
    let descendant: i32 = fs::read_to_string(fixture.options.native_home.join("descendant-pid"))
        .unwrap()
        .parse()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    {
        let mut state = backend.inner.state.0.lock().unwrap();
        while !state.root_exited {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .expect("root must be reaped independently");
            state = backend
                .inner
                .state
                .1
                .wait_timeout(state, remaining)
                .unwrap()
                .0;
        }
    }
    assert!(process::identity(root).is_none());
    let result = backend.stop().unwrap();
    assert!(result.exited);
    assert_eq!(result.exit_code, Some(9));
    assert!(result.containment_unknown);
    assert_eq!(result.observed_descendants_remaining, 0);
    // Orphan zombies may remain briefly until init reaps; they cannot execute or retain pipes.
    if let Some(identity) = process::identity(descendant) {
        let stat = fs::read_to_string(format!("/proc/{}/stat", identity.pid)).unwrap();
        assert_eq!(
            stat.rsplit_once(')').unwrap().1.split_whitespace().next(),
            Some("Z")
        );
    }
}

#[test]
fn pid_identity_parser_handles_parentheses_and_matches_owned_group() {
    let own = process::identity(std::process::id() as i32).unwrap();
    assert_eq!(own.pid, std::process::id() as i32);
    assert!(own.start > 0);
    assert!(process::can_capture_identity(own));
    assert!(!process::can_capture_identity(process::Identity {
        start: own.start + 1,
        ..own
    }));
    assert!(!process::can_capture_identity(process::Identity {
        group: own.group + 1,
        ..own
    }));
    assert!(process::identity(i32::MAX).is_none());
}

#[test]
fn drop_queues_stop_without_blocking_calling_thread() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(&ready_script("setInterval(()=>{},1000)"));
    backend.wait_ready().unwrap();
    let start = Instant::now();
    drop(backend);
    assert!(start.elapsed() < Duration::from_millis(100));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let event = events
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        if let PublicEvent::Exited(result) = event {
            assert!(result.exited);
            break;
        }
    }
}

#[test]
fn event_channel_is_bounded_and_private_auth_events_are_not_enqueued() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(NORMAL);
    backend.wait_ready().unwrap();
    for _ in 0..200 {
        backend.inner.emit(PublicEvent::Account(AccountMetadata {
            status: AccountStatus::SignedOut,
            phase: None,
        }));
    }
    assert!(backend.dropped_event_count() > 0);
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn platform_session_invalid_header_token_and_origin_are_rejected_privately() {
    for session in [
        json!({"origin":"https://user@platform.example","token":"PUBLIC_FAKE"}),
        json!({"origin":"https://platform.example/path","token":"PUBLIC_FAKE"}),
        json!({"origin":"https://platform.example","token":"bad\nvalue"}),
        json!({"origin":"https://platform.example","token":"PUBLIC_FAKE","requestHeaders":{"Bad\nHeader":"bad"}}),
    ] {
        assert!(parse(json!({"type":"platform-session","session":session})).is_err());
    }
    assert!(matches!(
        parse(json!({"type":"platform-session","session":null})).unwrap(),
        protocol::Event::Auth(None)
    ));
}

#[test]
fn malformed_account_state_never_returns_arbitrary_public_values() {
    for value in [
        json!({"status":"PUBLIC_FAKE_TOKEN","attempt":null}),
        json!({"status":"signed-out","attempt":{"phase":"PUBLIC_FAKE_TOKEN"}}),
        json!({"status":"signed-out"}),
    ] {
        assert!(parse(json!({"type":"account-state","state":value})).is_err());
    }
}

#[test]
fn early_stdout_eof_while_root_lives_is_a_bounded_protocol_failure() {
    let fixture = Fixture::new();
    let (backend, _) =
        fixture.spawn("import fs from 'node:fs';fs.closeSync(1);setInterval(()=>{},1000)");
    assert_eq!(backend.wait_ready(), Err(Error::Protocol));
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn exact_stdout_byte_limit_is_accepted_without_splitting() {
    let fixture = Fixture::new();
    let script = format!("process.stdout.write('x'.repeat(65535)+'\\n');\n{NORMAL}");
    let (backend, _) = fixture.spawn(&script);
    backend.wait_ready().unwrap();
    assert!(!backend.inner.fault.load(Ordering::Acquire));
    assert!(backend.stop().unwrap().graceful);
}

#[test]
fn wrong_correlated_reply_command_is_rejected_without_raw_value() {
    let fixture = Fixture::new();
    let script = NORMAL.replace("command:c.type,value", "command:'onboarding-read',value");
    let (backend, _) = fixture.spawn(&script);
    backend.wait_ready().unwrap();
    assert_eq!(
        backend.request(Command::AccountUnsubscribe, Duration::from_secs(1)),
        Err(Error::Protocol)
    );
    assert!(backend.stop().unwrap().graceful);
}

#[test]
fn owner_death_helper() {
    let Some(root) = std::env::var_os("DSH_NATIVE_CORE_TEST_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let options = RustBackendOptions {
        runtime: root.join("runtime"),
        expected_version: "0.2.1-alpha.1".into(),
        native_home: root.join("native-home"),
        user_home: root.join("user"),
        working_directory: root,
        absolute_node_path: Some("/usr/bin/node".into()),
        startup_timeout: Duration::from_secs(2),
        stop_policy: StopPolicy::default(),
    };
    let (backend, _) = Backend::spawn(options.validate().unwrap(), Some(NORMAL)).unwrap();
    backend.wait_ready().unwrap();
    loop {
        thread::park();
    }
}

#[test]
fn owner_process_death_signals_the_private_root() {
    let fixture = Fixture::new();
    let mut helper = ProcessCommand::new(std::env::current_exe().unwrap())
        .args(["--exact", "tests::owner_death_helper", "--nocapture"])
        .env_clear()
        .env("DSH_NATIVE_CORE_TEST_ROOT", fixture.directory.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let marker = fixture.options.native_home.join("fixture-pid");
    let deadline = Instant::now() + Duration::from_secs(2);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !marker.exists() {
        let _ = helper.kill();
        let _ = helper.wait();
        panic!("fixture helper did not boot");
    }
    let pid = fixture.root_pid();
    let root_identity = process::identity(pid).expect("fixture root alive");
    let cleanup = process::pin_fixture_cleanup(root_identity);
    helper.kill().unwrap();
    helper.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let running = fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .is_some_and(|stat| {
                stat.rsplit_once(')').unwrap().1.split_whitespace().next() != Some("Z")
            });
        if !running {
            break;
        }
        if Instant::now() >= deadline {
            // Cleanup only this known fixture root if the tested death signal failed.
            cleanup();
            panic!("owned root remained runnable after owner death");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn unsolicited_shutdown_ack_does_not_fake_graceful_stop() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn("process.stdout.write('DSH_TAURI:{\"type\":\"shutdown-complete\"}\\n');setInterval(()=>{},1000)");
    assert_eq!(backend.wait_ready(), Err(Error::Protocol));
    assert!(!backend.stop().unwrap().graceful);
}

#[test]
fn simultaneous_stop_requests_and_cached_stop_all_complete() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(NORMAL);
    backend.wait_ready().unwrap();
    let receivers: Vec<_> = (0..64).map(|_| backend.stop_async()).collect();
    for receiver in receivers {
        assert!(
            receiver
                .recv_timeout(Duration::from_secs(2))
                .unwrap()
                .exited
        );
    }
    let stopped = backend.stop().unwrap();
    for _ in 0..16 {
        assert_eq!(
            backend
                .stop_async()
                .recv_timeout(Duration::from_millis(50))
                .unwrap(),
            stopped
        );
    }
}

#[test]
fn fixture_directory_permissions_are_private() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture
            .directory
            .path()
            .metadata()
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}
