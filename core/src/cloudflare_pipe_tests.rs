use super::*;
// Public fixture values only. Exercises the production Core framing/correlation owner, not a Host.
const FLOW: &str = r#"
const mode = 'PUBLIC_MODE';
const send = value => process.stdout.write('DSH_TAURI:' + JSON.stringify(value) + '\n');
let snapshot = {accountId:'',modelsEnabled:false,browserEnabled:false,freePlanConfirmed:false,tokenConfigured:false,writable:true,revision:0,authStatus:{authMode:'api-token',setupConfigured:false,credentialStored:false,credentialWritable:true,phase:'idle',attemptId:null,retryBlocked:false,errorCode:null,authEpoch:0,sequence:0},oauthSetup:{clientId:'',scopes:[],callbackPort:8977}};
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let input = ''; process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => {input += chunk; while(input.includes('\n')) {
 const at=input.indexOf('\n'); const c=JSON.parse(input.slice(0,at)); input=input.slice(at+1);
 if(c.type==='shutdown'){send({type:'shutdown-complete'});process.exit(0);}
 if(c.type==='cloudflare-save') {
  if(Object.keys(c).sort().join(',')!=='accountId,apiToken,browserEnabled,expectedRevision,freePlanConfirmed,modelsEnabled,requestId,type'
     || (c.apiToken!==null && c.apiToken!=='PUBLIC_FAKE_TOKEN') || c.expectedRevision!==snapshot.revision) {
   send({type:'request-error',requestId:c.requestId,code:'CLOUDFLARE_CONFLICT'});continue;
  }
  snapshot={...snapshot,accountId:c.accountId,modelsEnabled:c.modelsEnabled,browserEnabled:c.browserEnabled,freePlanConfirmed:c.freePlanConfirmed,tokenConfigured:snapshot.tokenConfigured || c.apiToken!==null,writable:true,revision:snapshot.revision+1};
 } else if(c.type==='cloudflare-disable') {snapshot={...snapshot,modelsEnabled:false,browserEnabled:false,revision:snapshot.revision+1};}
 const finish=()=>send({type:'native-result',command:c.type,requestId:c.requestId,value:mode==='extra'&&c.type==='cloudflare-save'?{...snapshot,apiToken:'PUBLIC_FAKE_TOKEN'}:mode==='wrong-revision'&&c.type==='cloudflare-save'?{...snapshot,revision:snapshot.revision+8}:snapshot});
 if(mode==='late'&&c.type==='cloudflare-save'){setTimeout(finish,150);}else finish();
}});
"#;
fn read(backend: &Backend) -> CloudflareSnapshot {
    let PublicReply::Cloudflare(value) = backend
        .request(Command::CloudflareRead, Duration::from_secs(1))
        .unwrap()
    else {
        panic!("metadata")
    };
    value
}
#[test]
fn typed_core_pipe_roundtrip_keeps_null_token_and_rejects_stale_revision() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(&FLOW.replace("PUBLIC_MODE", "normal"));
    backend.wait_ready().unwrap();
    assert!(!read(&backend).token_configured);
    let first = backend
        .save_cloudflare(
            0,
            "0123456789abcdef0123456789abcdef",
            false,
            false,
            false,
            Some(SecretApiKey::new("PUBLIC_FAKE_TOKEN".into()).unwrap()),
            Duration::from_secs(1),
        )
        .unwrap();
    assert!(first.token_configured);
    assert_eq!(first.revision, 1);
    let second = backend
        .save_cloudflare(
            1,
            &first.account_id,
            true,
            false,
            true,
            None,
            Duration::from_secs(1),
        )
        .unwrap();
    assert!(second.models_enabled && second.token_configured);
    assert_eq!(second.revision, 2);
    assert!(
        backend
            .save_cloudflare(0, "", false, false, false, None, Duration::from_secs(1))
            .is_err()
    );
    let PublicReply::Cloudflare(disabled) = backend
        .request(
            Command::CloudflareDisable {
                expected_revision: 2,
            },
            Duration::from_secs(1),
        )
        .unwrap()
    else {
        panic!("disable")
    };
    assert!(!disabled.models_enabled && !disabled.browser_enabled && disabled.token_configured);
    assert!(backend.stop().unwrap().graceful);
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("PUBLIC_FAKE_TOKEN"));
    }
}
#[test]
fn late_receipt_is_unknown_and_explicit_read_only_observes_configuration() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&FLOW.replace("PUBLIC_MODE", "late"));
    backend.wait_ready().unwrap();
    assert_eq!(
        backend.save_cloudflare(
            0,
            "",
            false,
            false,
            false,
            Some(SecretApiKey::new("PUBLIC_FAKE_TOKEN".into()).unwrap()),
            Duration::from_millis(30)
        ),
        Err(Error::RequestTimeout)
    );
    let snapshot = read(&backend);
    assert!(snapshot.token_configured);
    assert_eq!(snapshot.revision, 1);
    assert!(backend.inner.pending.lock().unwrap().is_empty());
    assert!(backend.stop().unwrap().exited);
}
#[test]
fn extra_secret_and_mismatched_revision_never_become_confirmed_metadata() {
    for mode in ["extra", "wrong-revision"] {
        let fixture = Fixture::new();
        let (backend, events) = fixture.spawn(&FLOW.replace("PUBLIC_MODE", mode));
        backend.wait_ready().unwrap();
        assert_eq!(
            backend.save_cloudflare(
                0,
                "",
                false,
                false,
                false,
                Some(SecretApiKey::new("PUBLIC_FAKE_TOKEN".into()).unwrap()),
                Duration::from_secs(1)
            ),
            Err(Error::Protocol)
        );
        assert!(backend.stop().unwrap().exited);
        for event in events.try_iter() {
            assert!(!format!("{event:?}").contains("PUBLIC_FAKE_TOKEN"));
        }
    }
}

const OAUTH_FLOW: &str = r#"
const send=value=>process.stdout.write('DSH_TAURI:'+JSON.stringify(value)+'\n');
const oauthSetup={clientId:'PUBLIC-client',scopes:['PUBLIC:scope'],callbackPort:8977};
const status={authMode:'oauth',setupConfigured:true,credentialStored:false,credentialWritable:true,phase:'waiting-browser',attemptId:'PUBLIC-attempt',retryBlocked:false,errorCode:null,authEpoch:1,sequence:1};
const snapshot=()=>({accountId:'0123456789abcdef0123456789abcdef',modelsEnabled:false,browserEnabled:false,freePlanConfirmed:false,tokenConfigured:false,writable:true,revision:1,oauthSetup,authStatus:{...status}});
const url=new URL('https://dash.cloudflare.com/oauth2/auth');
for(const [k,v] of Object.entries({response_type:'code',client_id:oauthSetup.clientId,redirect_uri:'http://127.0.0.1:8977/cloudflare/callback',scope:oauthSetup.scopes.join(' '),state:'A'.repeat(43),code_challenge:'B'.repeat(43),code_challenge_method:'S256'}))url.searchParams.set(k,v);
const emit=browserUrl=>send({type:'cloudflare-auth-state',value:{...status},browserUrl,oauthSetup});
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let input='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>{input+=chunk;while(input.includes('\n')){
 const at=input.indexOf('\n'),c=JSON.parse(input.slice(0,at));input=input.slice(at+1);
 if(c.type==='shutdown'){send({type:'shutdown-complete'});process.exit(0);}
 if(c.type==='cloudflare-oauth-start'){emit(null);emit(url.toString());emit(null);}
 if(c.type==='cloudflare-oauth-cancel'){emit(url.toString());}
 send({type:'native-result',command:c.type,requestId:c.requestId,value:snapshot()});
}});
"#;
#[test]
fn private_oauth_grant_augment_preserve_revoke_and_public_projection_use_production_pipe() {
    let fixture = Fixture::new();
    let (backend, events) = fixture.spawn(OAUTH_FLOW);
    backend.wait_ready().unwrap();
    let permit = std::sync::atomic::AtomicBool::new(true);
    let snapshot = backend
        .start_cloudflare(1, &permit, Duration::from_secs(1))
        .unwrap();
    assert!(snapshot.auth_status.browser_available);
    assert!(
        backend
            .inner
            .state
            .0
            .lock()
            .unwrap()
            .cloudflare_browser
            .is_some()
    );
    // A full read without URI must retain the owned private grant.
    assert!(read(&backend).auth_status.browser_available);
    permit.store(false, std::sync::atomic::Ordering::Release);
    assert_eq!(
        backend.open_cloudflare_browser("PUBLIC-attempt", &permit),
        Err(Error::InvalidCommand)
    );
    backend.revoke_cloudflare_browser();
    let PublicReply::Cloudflare(cancel) = backend
        .request(
            Command::CloudflareOauthCancel {
                attempt_id: "PUBLIC-attempt".into(),
            },
            Duration::from_secs(1),
        )
        .unwrap()
    else {
        panic!("snapshot");
    };
    assert!(!cancel.auth_status.browser_available);
    assert!(
        backend
            .inner
            .state
            .0
            .lock()
            .unwrap()
            .cloudflare_browser
            .is_none()
    );
    for event in events.try_iter() {
        assert!(!format!("{event:?}").contains("dash.cloudflare.com"));
    }
    assert!(backend.stop().unwrap().exited);
}
#[test]
fn revoked_unknown_attempt_cannot_be_rearmed_by_generic_start_or_late_event() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(OAUTH_FLOW);
    backend.wait_ready().unwrap();
    backend.revoke_cloudflare_browser();
    let PublicReply::Cloudflare(snapshot) = backend
        .request(
            Command::CloudflareOauthStart {
                expected_revision: 1,
            },
            Duration::from_secs(1),
        )
        .unwrap()
    else {
        panic!("snapshot");
    };
    assert!(!snapshot.auth_status.browser_available);
    assert!(
        backend
            .inner
            .state
            .0
            .lock()
            .unwrap()
            .cloudflare_browser
            .is_none()
    );
    assert_eq!(
        backend
            .open_cloudflare_browser("PUBLIC-attempt", &std::sync::atomic::AtomicBool::new(true)),
        Err(Error::InvalidCommand)
    );
    assert!(backend.stop().unwrap().exited);
}
