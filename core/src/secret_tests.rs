use super::*;

// Public fake sentinel only. The fixture persists a boolean marker, never a credential value.
const PUBLIC_FAKE_KEY: &str = "PUBLIC_FAKE_API_KEY";
const KEY_FLOW: &str = r#"
import fs from 'node:fs';
import { join } from 'node:path';
const mode = 'FIXTURE_MODE';
const send = value => process.stdout.write('DSH_TAURI:' + JSON.stringify(value) + '\n');
fs.mkdirSync(process.env.DSH_TAURI_HOME, {recursive:true});
const marker = join(process.env.DSH_TAURI_HOME, 'public-fake-configured');
let saved = fs.existsSync(marker);
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let pending = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => {
 pending += chunk;
 while (pending.includes('\n')) {
  const at = pending.indexOf('\n'); const c = JSON.parse(pending.slice(0,at)); pending = pending.slice(at+1);
  if (c.type === 'shutdown') {send({type:'shutdown-complete'});process.exit(0);}
  if (c.type === 'onboarding-read') {
   send({type:'native-result',command:c.type,requestId:c.requestId,value:{loggedIn:false,hasApiKey:saved,writable:mode !== 'refuse'}});
  } else if (c.type === 'save-api-key') {
   // Echo only our explicitly public fixture sentinel through hostile diagnostics/extra fields.
   process.stderr.write('fixture discarded ' + c.apiKey + '\n');
   process.stdout.write('unframed fixture discarded ' + c.apiKey + '\n');
   const valid = Object.keys(c).sort().join(',') === 'apiKey,requestId,type'
    && typeof c.requestId === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(c.requestId)
    && c.apiKey === 'PUBLIC_FAKE_API_KEY';
   if (!valid || mode === 'error') {send({type:'request-error',requestId:c.requestId,message:c.apiKey});continue;}
   if (mode === 'malformed') {send({type:'native-result',command:c.type,requestId:c.requestId,value:{ok:c.apiKey}});continue;}
   if (mode === 'wrong-command') {send({type:'native-result',command:'account-state',requestId:c.requestId,value:{status:'signed-out',attempt:null,token:c.apiKey}});continue;}
   const finish = () => {
    const ok = mode !== 'refuse';
    if (ok) {fs.writeFileSync(marker, 'true', {mode:0o600});saved=true;}
    send({type:'native-result',command:c.type,requestId:c.requestId,value:{ok,apiKey:c.apiKey,message:c.apiKey,path:c.apiKey}});
   };
   if (mode === 'late') setTimeout(finish,150); else finish();
  }
 }
});
"#;

fn key() -> SecretApiKey {
    SecretApiKey::new(PUBLIC_FAKE_KEY.to_owned()).unwrap()
}
fn onboarding(backend: &Backend) -> OnboardingMetadata {
    let PublicReply::Onboarding(value) = backend
        .request(Command::OnboardingRead, Duration::from_secs(1))
        .unwrap()
    else {
        panic!("wrong public metadata reply");
    };
    value
}

#[test]
fn secret_debug_is_fixed_and_all_invalid_input_is_consumed_without_echo() {
    assert_eq!(format!("{:?}", key()), "SecretApiKey([REDACTED])");
    for raw in [
        String::new(),
        "PUBLIC_FAKE_API_KEY ".into(),
        " PUBLIC_FAKE_API_KEY".into(),
        "PUBLIC_FAKE\nKEY".into(),
        "PUBLIC_FAKE\0KEY".into(),
        "PUBLIC_FAKE\u{7f}KEY".into(),
        "PUBLIC_FAKE_é".into(),
        "A".repeat(16_385),
    ] {
        let answer = SecretApiKey::new(raw);
        assert!(matches!(answer, Err(Error::InvalidCommand)));
        assert!(!format!("{answer:?}").contains("PUBLIC_FAKE"));
    }
}

#[test]
fn secret_frame_escapes_quotes_and_backslashes_without_arbitrary_fields() {
    let raw = "PUBLIC_FAKE_\"quoted\"_\\path";
    let secret = SecretApiKey::new(raw.into()).unwrap();
    let frame = secret.frame("42").unwrap();
    assert_eq!(frame.as_slice().last(), Some(&b'\n'));
    let value: Value = serde_json::from_slice(frame.as_slice()).unwrap();
    assert!(value["apiKey"].as_str() == Some(raw));
    assert!(value["requestId"] == "42" && value["type"] == "save-api-key");
    assert_eq!(value.as_object().unwrap().len(), 3);
    assert!(secret.frame("bad\"id").is_err());
    assert!(secret.frame("").is_err());
}

#[test]
fn maximum_secret_input_remains_within_complete_stdin_frame_limit() {
    let raw = "\\\"".repeat(8192);
    let secret = SecretApiKey::new(raw).unwrap();
    let frame = secret.frame(&"r".repeat(128)).unwrap();
    assert!(frame.as_slice().len() <= protocol::MAX_LINE);
    assert!(frame.as_slice().len() > 32768);
    let parsed: Value = serde_json::from_slice(frame.as_slice()).unwrap();
    assert_eq!(parsed["apiKey"].as_str().unwrap().len(), 16384);
}

#[test]
fn save_ack_and_onboarding_only_publish_boolean_metadata_in_native_home() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(&KEY_FLOW.replace("FIXTURE_MODE", "save"));
    backend.wait_ready().unwrap();
    assert!(!onboarding(&backend).has_api_key);
    assert_eq!(
        backend.save_api_key(key(), Duration::from_secs(1)),
        Ok(true)
    );
    assert!(onboarding(&backend).has_api_key);
    let marker = fixture.options.native_home.join("public-fake-configured");
    assert_eq!(fs::read_to_string(&marker).unwrap(), "true");
    assert_eq!(
        marker.metadata().unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(
        !fixture
            .options
            .user_home
            .join("public-fake-configured")
            .exists()
    );
    assert!(!fixture.options.user_home.join(".dsh").exists());
    assert!(backend.inner.pending.lock().unwrap().is_empty());
    assert!(backend.stop().unwrap().graceful);
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("PUBLIC_FAKE"));
    }
}

#[test]
fn refused_save_returns_false_without_persistence_or_secret_error() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(&KEY_FLOW.replace("FIXTURE_MODE", "refuse"));
    backend.wait_ready().unwrap();
    assert!(!onboarding(&backend).writable);
    assert_eq!(
        backend.save_api_key(key(), Duration::from_secs(1)),
        Ok(false)
    );
    assert!(!onboarding(&backend).has_api_key);
    assert!(
        !fixture
            .options
            .native_home
            .join("public-fake-configured")
            .exists()
    );
    assert!(backend.stop().unwrap().graceful);
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("PUBLIC_FAKE"));
    }
}

#[test]
fn failed_or_malformed_save_replies_never_echo_secret_or_free_text() {
    for (mode, expected) in [
        ("error", Error::RequestFailed),
        ("malformed", Error::Protocol),
        ("wrong-command", Error::Protocol),
    ] {
        let fixture = Fixture::new();
        let (backend, events) = fixture.spawn(&KEY_FLOW.replace("FIXTURE_MODE", mode));
        backend.wait_ready().unwrap();
        let reply = backend.save_api_key(key(), Duration::from_secs(1));
        assert_eq!(reply, Err(expected));
        assert!(!format!("{reply:?}").contains("PUBLIC_FAKE"));
        assert!(
            !fixture
                .options
                .native_home
                .join("public-fake-configured")
                .exists()
        );
        assert!(backend.inner.pending.lock().unwrap().is_empty());
        assert!(backend.stop().unwrap().graceful);
        for event in events.try_iter() {
            assert!(!format!("{event:?}").contains("PUBLIC_FAKE"));
        }
    }
}

#[test]
fn save_reply_timeout_is_indeterminate_and_late_ack_has_no_public_secret() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(&KEY_FLOW.replace("FIXTURE_MODE", "late"));
    backend.wait_ready().unwrap();
    assert_eq!(
        backend.save_api_key(key(), Duration::from_millis(10)),
        Err(Error::RequestTimeout)
    );
    assert!(backend.inner.pending.lock().unwrap().is_empty());
    // A timeout cannot promise the provider didn't commit. Only metadata re-read is allowed.
    let deadline = Instant::now() + Duration::from_secs(1);
    while !onboarding(&backend).has_api_key {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(20));
    }
    assert!(backend.stop().unwrap().graceful);
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("PUBLIC_FAKE"));
    }
}

#[test]
fn secret_save_uses_bounded_writer_and_poisons_partial_framing() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(&ready_script("setInterval(()=>{},1000)"));
    backend.wait_ready().unwrap();
    let fd = backend.inner.input.lock().unwrap().as_raw_fd();
    assert!(unsafe { libc::fcntl(fd, libc::F_SETPIPE_SZ, 4096) } >= 0);
    let start = Instant::now();
    let secret = SecretApiKey::new("PUBLIC_FAKE_".to_owned() + &"A".repeat(16372)).unwrap();
    let result = backend.save_api_key(secret, Duration::from_millis(10));
    assert_eq!(result, Err(Error::WriteTimeout));
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(backend.inner.poisoned.load(Ordering::Acquire));
    assert!(backend.inner.pending.lock().unwrap().is_empty());
    assert!(!format!("{result:?}").contains("PUBLIC_FAKE"));
    assert!(backend.stop().unwrap().exited);
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("PUBLIC_FAKE"));
    }
}

#[test]
fn secret_save_busy_writer_and_invalid_timeout_return_fixed_codes() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&KEY_FLOW.replace("FIXTURE_MODE", "save"));
    backend.wait_ready().unwrap();
    assert_eq!(
        backend.save_api_key(key(), Duration::ZERO),
        Err(Error::InvalidCommand)
    );
    assert_eq!(
        backend.save_api_key(key(), Duration::from_secs(31)),
        Err(Error::InvalidCommand)
    );
    {
        let _writer = backend.inner.input.lock().unwrap();
        let start = Instant::now();
        assert_eq!(
            backend.save_api_key(key(), Duration::from_secs(1)),
            Err(Error::WriterBusy)
        );
        assert!(start.elapsed() < Duration::from_millis(100));
    }
    assert!(!backend.inner.poisoned.load(Ordering::Acquire));
    assert!(backend.inner.pending.lock().unwrap().is_empty());
    assert!(
        !fixture
            .options
            .native_home
            .join("public-fake-configured")
            .exists()
    );
    assert!(backend.stop().unwrap().graceful);
}

#[test]
fn secret_save_before_ready_or_after_stop_does_not_write_or_leak() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn("setInterval(()=>{},1000)");
    assert_eq!(
        backend.save_api_key(key(), Duration::from_millis(10)),
        Err(Error::Starting)
    );
    assert!(backend.stop().unwrap().exited);
    assert_eq!(
        backend.save_api_key(key(), Duration::from_millis(10)),
        Err(Error::Stopping)
    );
}
