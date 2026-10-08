use crate::*;
use serde_json::json;
use std::{fs, time::Duration};

fn public_metadata() -> serde_json::Value {
    json!({"available":true,"credentialStored":false,"routeConfigured":false,
        "phase":"waiting-browser","attempt":1,"prompt":2,"browserAvailable":false,
        "settingsRevision":0,"retryBlocked":false,"token":"PUBLIC_FAKE_TOKEN"})
}
#[test]
fn metadata_is_closed_and_safe_integer_fenced() {
    let metadata = crate::codex::metadata(&public_metadata()).unwrap();
    assert!(!format!("{metadata:?}").contains("PUBLIC_FAKE_TOKEN"));
    for (field, value) in [
        ("attempt", json!(0)),
        ("prompt", json!(9_007_199_254_740_992u64)),
        ("phase", json!("unknown")),
        ("available", json!("PUBLIC_FAKE_TOKEN")),
    ] {
        let mut input = public_metadata();
        input[field] = value;
        assert!(crate::codex::metadata(&input).is_err());
    }
    assert!(Command::CodexCancel { attempt: 0 }.encode("1").is_err());
    assert!(
        Command::CodexEnableModels {
            expected_revision: 9_007_199_254_740_992
        }
        .encode("1")
        .is_err()
    );
}
#[test]
fn response_is_redacted_bounded_and_private_framed() {
    let response = SecretCodexCallback::new("PUBLIC_FAKE_\"CODE\\".into()).unwrap();
    assert_eq!(format!("{response:?}"), "SecretCodexCallback([REDACTED])");
    let frame = response.frame("1", 1, 2).unwrap();
    let value: serde_json::Value = serde_json::from_slice(frame.as_slice()).unwrap();
    assert_eq!(value["response"], "PUBLIC_FAKE_\"CODE\\");
    assert_eq!(value["attempt"], 1);
    assert_eq!(value["prompt"], 2);
    assert!(response.frame("1", 0, 2).is_err());
    for raw in [
        "".into(),
        " code".into(),
        "code\n".into(),
        "é".into(),
        "x".repeat(16_385),
    ] {
        assert!(SecretCodexCallback::new(raw).is_err());
    }
}
fn public_url() -> String {
    format!(
        "https://auth.openai.com/oauth/authorize?response_type=code&client_id=app_EMoamEEZ73f0CkXaXp7hrann&redirect_uri=http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid+profile+email+offline_access&code_challenge={}&code_challenge_method=S256&state={}&id_token_add_organizations=true&codex_cli_simplified_flow=true&originator=pi",
        "A".repeat(43),
        "a".repeat(32)
    )
}
#[test]
fn browser_validation_never_launches_and_denies_injection() {
    let url = public_url();
    assert!(crate::browser::validate(&url).is_ok());
    for invalid in [
        url.replace("https:", "http:"),
        url.replace("auth.openai.com", "evil.example"),
        url.clone() + "#fragment",
        url.clone() + "&state=duplicate",
        url.replace("originator=pi", "originator=unknown"),
        url.replace("/oauth/authorize", "/other"),
        url.replace("https://", "https://user@"),
    ] {
        assert!(crate::browser::validate(&invalid).is_err());
    }
    let event = json!({"type":"codex-state","value":public_metadata(),"browserUrl":null});
    let bytes = format!("DSH_TAURI:{event}");
    assert!(matches!(
        crate::protocol::parse(bytes.as_bytes(), "0.2.1-alpha.1"),
        Ok(crate::protocol::Event::Codex(_, None))
    ));
}

#[test]
fn opener_environment_is_pure_gui_identity_allowlist() {
    let input = [
        ("HOME", "/public/fixture"),
        ("WAYLAND_DISPLAY", "public-wayland"),
        ("DBUS_SESSION_BUS_ADDRESS", "public-dbus"),
        ("XDG_CONFIG_HOME", "/public/config"),
        ("DEEPSEEK_API_KEY", "PUBLIC_FAKE_SECRET"),
        ("OPENAI_API_KEY", "PUBLIC_FAKE_SECRET"),
        ("NODE_OPTIONS", "PUBLIC_FAKE_SECRET"),
        ("BROWSER", "PUBLIC_FAKE_SHELL_OVERRIDE"),
        ("PATH", "/public/untrusted"),
        ("TOKEN", "PUBLIC_FAKE_SECRET"),
    ]
    .into_iter()
    .map(|(key, value)| (key.into(), value.into()));
    let selected = crate::browser::environment(input);
    let output: std::collections::BTreeMap<_, _> = selected.into_iter().collect();
    assert_eq!(output.len(), 5);
    assert_eq!(
        output.get(std::ffi::OsStr::new("HOME")),
        Some(&std::ffi::OsString::from("/public/fixture"))
    );
    assert_eq!(
        output.get(std::ffi::OsStr::new("PATH")),
        Some(&std::ffi::OsString::from("/usr/bin:/bin"))
    );
    assert!(!output.contains_key(std::ffi::OsStr::new("BROWSER")));
    assert!(
        !output
            .values()
            .any(|value| value.to_string_lossy().contains("PUBLIC_FAKE_SECRET"))
    );
}

const MOCK: &str = r#"
import assert from 'node:assert/strict';
import { createCodexBridge, validCodexUrl } from './codex.mjs';
import { parseForkCommand } from './fork-supervisor.mjs';
const send = value => process.stdout.write('DSH_TAURI:' + JSON.stringify(value) + '\n');
const key = 'llm-pi-ai/openai-codex';
let configured=false, routes=[], revision=0, value={providers:{}}, mutateCount=0;
let mode='normal', recordKind='grant';
const services = {
 credentials:{describeRecord:async candidate=>{assert.equal(candidate,key);return {configured,kind:recordKind};}},
 llm:{listConfigurableProviders:()=>[{provider:'openai-codex',settingsNs:'owned',settingsPath:['providers','openai-codex']}],listProviders:()=>routes},
 settings:{writable:true,describe:()=>[{ns:'owned',revision,value}],mutate:async(ns,ops,expected)=>{assert.equal(ns,'owned');assert.equal(expected,revision);assert.deepEqual(ops,[{op:'set',path:['providers','openai-codex'],value:{}}]);mutateCount++;value.providers['openai-codex']={};revision++;routes=[{id:'openai-codex'}];}},
 authorization:{describe:candidate=>{assert.equal(candidate,key);return {methods:[{id:'oauth'}]};},cancel:candidate=>assert.equal(candidate,key),
 begin:async({key:candidate,method,interaction,signal})=>{
 assert.equal(candidate,key);assert.equal(method,'oauth');
 const options=mode==='unknown'?[{id:'unknown'}]:[{id:'browser'},{id:'device_code'}];
 assert.equal(await interaction.prompt({kind:'select',options}),'browser');
 const result=await interaction.prompt({kind:'text',signal,message:'Complete login in your browser, or paste the authorization code / redirect URL here:',placeholder:'http://localhost:1455/auth/callback'});
 assert.equal(result,'PUBLIC_FAKE_CODE');configured=true;return {status:'authorized'};
 }},
};
const bridge=createCodexBridge({ctx:{get:name=>services[name]},emit:()=>{}});
const start=await bridge.handle({type:'codex-start'});
assert.equal(start.attempt,1);
await new Promise(resolve=>setImmediate(resolve));
let status=await bridge.handle({type:'codex-status'});
assert.equal(status.prompt,2);
assert.deepEqual(await bridge.handle({type:'codex-callback',attempt:1,prompt:99,response:'PUBLIC_FAKE_CODE'}),{ok:false});
assert.deepEqual(await bridge.handle({type:'codex-callback',attempt:1,prompt:2,response:'PUBLIC_FAKE_CODE'}),{ok:true});
assert.deepEqual(await bridge.handle({type:'codex-callback',attempt:1,prompt:2,response:'PUBLIC_FAKE_CODE'}),{ok:false});
await new Promise(resolve=>setImmediate(resolve));
assert.equal((await bridge.handle({type:'codex-status'})).phase,'authorized');
recordKind='api-key';
assert.equal((await bridge.handle({type:'codex-status'})).credentialStored,false);
await assert.rejects(bridge.handle({type:'codex-enable-models',expectedRevision:0}));
recordKind='grant';
await assert.rejects(bridge.handle({type:'codex-enable-models',expectedRevision:99}));
await bridge.handle({type:'codex-enable-models',expectedRevision:0});
assert.equal(mutateCount,1);
await bridge.handle({type:'codex-enable-models',expectedRevision:1});
assert.equal(mutateCount,1);
await bridge.stop();
const cancelled=createCodexBridge({ctx:{get:name=>services[name]},emit:()=>{}});
const attempt=await cancelled.handle({type:'codex-start'});
await new Promise(resolve=>setImmediate(resolve));
await cancelled.handle({type:'codex-cancel',attempt:attempt.attempt});
assert.equal((await cancelled.handle({type:'codex-status'})).retryBlocked,true);
await assert.rejects(cancelled.handle({type:'codex-start'}));
await cancelled.stop();
mode='unknown';
const unknown=createCodexBridge({ctx:{get:name=>services[name]},emit:()=>{}});
await unknown.handle({type:'codex-start'});
await new Promise(resolve=>setImmediate(resolve));
assert.equal((await unknown.handle({type:'codex-status'})).phase,'failed');
assert.equal((await unknown.handle({type:'codex-status'})).retryBlocked,true);
await unknown.stop();
// The seam may settle cancellation before an admitted store write finishes.
configured=false;
const orphanServices={...services,authorization:{...services.authorization,begin:({signal})=>new Promise(resolve=>{
 signal.addEventListener('abort',()=>{resolve({status:'cancelled'});setTimeout(()=>{configured=true;},15);},{once:true});
})}};
const expired=createCodexBridge({ctx:{get:name=>orphanServices[name]},emit:()=>{},timeoutMs:5});
await expired.handle({type:'codex-start'});
await new Promise(resolve=>setTimeout(resolve,35));
const late=await expired.handle({type:'codex-status'});
assert.equal(late.phase,'indeterminate');assert.equal(late.retryBlocked,true);assert.equal(late.credentialStored,true);
await assert.rejects(expired.handle({type:'codex-start'}));
await expired.stop();
assert.equal(validCodexUrl('https://evil.example/'),false);
assert.throws(()=>parseForkCommand(JSON.stringify({type:'codex-start',requestId:'1',key:'arbitrary'})));
// A delayed local credential read must not mix old presence with a new attempt phase.
mode='normal';configured=false;
let delayNext=false;const delayedReads=[];const snapshots=[];
const delayedServices={...services,credentials:{describeRecord:async()=>{
 const captured={configured,kind:'grant'};
 if(delayNext){delayNext=false;return new Promise(resolve=>delayedReads.push(()=>resolve(captured)));}
 return captured;
}}};
const delayed=createCodexBridge({ctx:{get:name=>delayedServices[name]},emit:packet=>snapshots.push(packet.value)});
await delayed.handle({type:'codex-start'});
await new Promise(resolve=>setImmediate(resolve));
const beforeReply=await delayed.handle({type:'codex-status'});
delayNext=true;const staleReply=delayed.handle({type:'codex-status'});
delayNext=true;
await delayed.handle({type:'codex-callback',attempt:beforeReply.attempt,prompt:beforeReply.prompt,response:'PUBLIC_FAKE_CODE'});
await new Promise(resolve=>setImmediate(resolve));
assert.equal(delayedReads.length,2);
delayedReads.forEach(resolve=>resolve());
const coherent=await staleReply;
assert.equal(coherent.phase,'authorized');assert.equal(coherent.credentialStored,true);
await new Promise(resolve=>setImmediate(resolve));
assert.equal(snapshots.some(value=>value.phase==='authorized'&&!value.credentialStored),false);
await delayed.stop();
mode='normal';
const controlBridge=createCodexBridge({ctx:{get:name=>services[name]},emit:send});
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let pending='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>{pending+=chunk;while(pending.includes('\n')){const at=pending.indexOf('\n');const c=JSON.parse(pending.slice(0,at));pending=pending.slice(at+1);
 void (async()=>{if(c.type==='shutdown'){await controlBridge.stop();send({type:'shutdown-complete'});process.exit(0);}const value=await controlBridge.handle(parseForkCommand(JSON.stringify(c)));send({type:'native-result',requestId:c.requestId,command:c.type,value});})().catch(()=>send({type:'request-error',requestId:c.requestId,message:'Fixed mock refusal'}));
}});
"#;
fn staged_fixture(script: &str) -> (tempfile::TempDir, Backend) {
    let directory = tempfile::tempdir().unwrap();
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
        native_home: directory.path().join("native"),
        user_home: user,
        working_directory: directory.path().into(),
        absolute_node_path: Some("/usr/bin/node".into()),
        startup_timeout: Duration::from_secs(3),
        stop_policy: StopPolicy::default(),
    };
    let (backend, _) = Backend::spawn(options.validate().unwrap(), Some(script)).unwrap();
    backend.wait_ready().unwrap();
    (directory, backend)
}

#[test]
fn staged_mock_flow_authorizes_cas_fences_cancel_and_unknown_without_network() {
    let (_directory, backend) = staged_fixture(MOCK);
    let PublicReply::Codex(mut metadata) = backend
        .request(Command::CodexStart, Duration::from_secs(1))
        .unwrap()
    else {
        panic!("fixed metadata expected");
    };
    if metadata.prompt.is_none() {
        let PublicReply::Codex(current) = backend
            .request(Command::CodexStatus, Duration::from_secs(1))
            .unwrap()
        else {
            panic!("fixed metadata expected");
        };
        metadata = current;
    }
    let attempt = metadata.attempt.unwrap();
    let prompt = metadata.prompt.unwrap();
    assert!(
        !backend
            .submit_codex_callback(
                attempt,
                prompt + 1,
                SecretCodexCallback::new("PUBLIC_FAKE_CODE".into()).unwrap(),
                Duration::from_secs(1)
            )
            .unwrap()
    );
    assert!(
        backend
            .submit_codex_callback(
                attempt,
                prompt,
                SecretCodexCallback::new("PUBLIC_FAKE_CODE".into()).unwrap(),
                Duration::from_secs(1)
            )
            .unwrap()
    );
    assert!(
        !backend
            .submit_codex_callback(
                attempt,
                prompt,
                SecretCodexCallback::new("PUBLIC_FAKE_CODE".into()).unwrap(),
                Duration::from_secs(1)
            )
            .unwrap()
    );
    assert!(matches!(
        backend.open_codex_browser(1),
        Err(Error::InvalidCommand)
    ));
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn acknowledged_attempt_cancel_tombstone_precedes_late_private_browser_packet() {
    let script = r#"
const send=v=>process.stdout.write('DSH_TAURI:'+JSON.stringify(v)+'\n');
const url='PUBLIC_URL';
const value={available:true,credentialStored:false,routeConfigured:false,phase:'waiting-browser',attempt:11,prompt:null,browserAvailable:false,settingsRevision:0,retryBlocked:false};
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let pending='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>{pending+=chunk;while(pending.includes('\n')){const at=pending.indexOf('\n');const c=JSON.parse(pending.slice(0,at));pending=pending.slice(at+1);
 if(c.type==='shutdown'){send({type:'shutdown-complete'});process.exit(0);}
 if(c.type==='codex-start'){send({type:'native-result',requestId:c.requestId,command:c.type,value});}
 else if(c.type==='codex-cancel'&&value.attempt===11){send({type:'codex-state',value:{...value,prompt:22,browserAvailable:true},browserUrl:url});send({type:'native-result',requestId:c.requestId,command:c.type,value:{...value,phase:'indeterminate',retryBlocked:true}});}
 else if(c.type==='codex-status'){value.attempt=12;value.prompt=23;value.browserAvailable=true;send({type:'codex-state',value,browserUrl:url});send({type:'native-result',requestId:c.requestId,command:c.type,value});}
 else {send({type:'native-result',requestId:c.requestId,command:c.type,value});}
}});
"#.replace("PUBLIC_URL", &public_url());
    let (_directory, backend) = staged_fixture(&script);
    let PublicReply::Codex(start) = backend
        .request(Command::CodexStart, Duration::from_secs(1))
        .unwrap()
    else {
        panic!("metadata expected");
    };
    assert_eq!(start.attempt, Some(11));
    {
        let state = backend.inner.state.0.lock().unwrap();
        assert!(state.codex_metadata.is_none());
        assert_eq!(state.codex_known_attempt, Some(11));
    }
    backend
        .request(Command::CodexCancel { attempt: 11 }, Duration::from_secs(1))
        .unwrap();
    {
        let state = backend.inner.state.0.lock().unwrap();
        assert_eq!(state.codex_cancelled, Some(11));
        assert!(state.codex_browser.is_none()); // Failure stops before any opener could run.
    }
    assert_eq!(backend.open_codex_browser(11), Err(Error::InvalidCommand));
    assert_eq!(
        backend.request(Command::CodexStart, Duration::from_secs(1)),
        Err(Error::RequestFailed)
    );
    backend
        .request(Command::CodexStatus, Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        backend.inner.state.0.lock().unwrap().codex_known_attempt,
        Some(12)
    );
    backend
        .request(Command::CodexCancel { attempt: 11 }, Duration::from_secs(1))
        .unwrap();
    {
        let state = backend.inner.state.0.lock().unwrap();
        assert_eq!(state.codex_known_attempt, Some(12));
        assert!(state.codex_browser.is_some()); // A stale cancel never retires newer authority.
        assert_eq!(
            state.codex_metadata.unwrap().phase,
            CodexPhase::WaitingBrowser
        );
    }
    assert!(backend.stop().unwrap().exited);
}
