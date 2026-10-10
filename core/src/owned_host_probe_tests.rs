use super::*;

const PROBE_HOST: &str = r#"
import http from 'node:http';
import crypto from 'node:crypto';
import fs from 'node:fs';
import { join } from 'node:path';
const mode='PUBLIC_PROBE_MODE';
const home=process.env.DSH_TAURI_HOME;
fs.mkdirSync(home,{recursive:true,mode:0o700});
const send=value=>process.stdout.write('DSH_TAURI:'+JSON.stringify(value)+'\n');
let cookieName,exchanges=0,reads=0;
const server=http.createServer((request,response)=>{
 if(request.method==='GET'){
  fs.writeFileSync(join(home,'exchange-count'),String(++exchanges));
  if(exchanges!==1)throw Error('PUBLIC repeated authentication refused');
  response.writeHead(303,{location:'./','set-cookie':`${cookieName}=PUBLIC_FAKE_COOKIE; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600; Expires=${new Date(Date.now()+3600000).toUTCString()}`});response.end();return;
 }
 let body='';request.on('data',chunk=>body+=chunk);request.on('end',()=>{
  const frame=JSON.parse(body);
  if(frame.method!=='session/list')throw Error('PUBLIC mutating request refused');
  fs.writeFileSync(join(home,'read-count'),String(++reads));
  response.writeHead(200,{'content-type':'application/json'});
  response.end(JSON.stringify({type:'server-response',rpcId:frame.rpcId,result:{ok:true,value:{items:[]}}}));
 });
});
server.listen(0,'127.0.0.1',()=>{
 const authority=`127.0.0.1:${server.address().port}`;
 cookieName=`dsh-auth-${crypto.createHash('sha256').update(authority).digest('base64url')}`;
 send({type:'ready',url:`http://${authority}/?token=PUBLIC_FAKE_LAUNCH`,version:'0.2.1-alpha.1',profile:'desktop'});
});
let pending='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>{
 pending+=chunk;while(pending.includes('\n')){
  const at=pending.indexOf('\n');const command=JSON.parse(pending.slice(0,at));pending=pending.slice(at+1);
  if(command.type==='shutdown'){if(mode!=='eof')send({type:'shutdown-complete'});server.close(()=>process.exit(0));continue;}
  if(command.type!=='inspect')throw Error('PUBLIC non-read command refused');
  fs.writeFileSync(join(home,'probe-inspected'),'PUBLIC inspected');
  const ack=()=>send({type:'inspection',requestId:command.requestId,activeTasks:0,scheduledTasks:0,unknown:true});
  if(mode==='timeout')continue;
  if(mode==='wrong'){send({type:'workspace-added',requestId:command.requestId});continue;}
  if(mode==='eof'){fs.closeSync(1);continue;}
  if(mode==='delayed'){setTimeout(ack,100);continue;}
  ack();
 }
});
"#;

async fn connected(mode: &str) -> (Fixture, Arc<Backend>, dsh_native_transport::NativeClient) {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&PROBE_HOST.replace("PUBLIC_PROBE_MODE", mode));
    backend.wait_ready().unwrap();
    let backend = Arc::new(backend);
    let client = backend
        .connect_transport(dsh_native_transport::Limits::default())
        .await
        .unwrap();
    (fixture, backend, client)
}

#[tokio::test]
async fn probe_returns_only_cached_owner_client_without_authentication_or_receipt_reconciliation() {
    let (fixture, backend, client) = connected("ack").await;
    backend
        .inner
        .codex_account_control
        .lock()
        .unwrap()
        .uncertain = true;
    let proven = backend
        .probe_owned_host(Duration::from_secs(1))
        .await
        .unwrap();
    assert!(proven.session_list().await.unwrap().items.is_empty());
    assert!(
        backend
            .inner
            .codex_account_control
            .lock()
            .unwrap()
            .uncertain
    );
    assert_eq!(
        fs::read_to_string(fixture.options.native_home.join("exchange-count")).unwrap(),
        "1"
    );
    assert_eq!(
        fs::read_to_string(fixture.options.native_home.join("read-count")).unwrap(),
        "1"
    );
    // Existing transport lifetime, not endpoint equality, establishes the cached clone identity.
    client.close().await;
    assert!(matches!(
        proven.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    let still_same_closed = backend
        .probe_owned_host(Duration::from_secs(1))
        .await
        .unwrap();
    assert!(matches!(
        still_same_closed.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    assert_eq!(
        fs::read_to_string(fixture.options.native_home.join("exchange-count")).unwrap(),
        "1"
    );
    assert!(matches!(
        backend
            .connect_transport(dsh_native_transport::Limits::default())
            .await,
        Err(ConnectError::AlreadyAttempted)
    ));
    assert!(backend.stop().unwrap().exited);
    assert!(matches!(
        backend.probe_owned_host(Duration::from_secs(1)).await,
        Err(Error::Exited | Error::Stopping)
    ));
}

#[tokio::test]
async fn probe_without_initialized_transport_does_not_authenticate_or_send_inspection() {
    let fixture = Fixture::new();
    let (backend, _) = fixture.spawn(&PROBE_HOST.replace("PUBLIC_PROBE_MODE", "ack"));
    backend.wait_ready().unwrap();
    let backend = Arc::new(backend);
    assert!(matches!(
        backend.probe_owned_host(Duration::from_secs(1)).await,
        Err(Error::Starting)
    ));
    assert!(!fixture.options.native_home.join("exchange-count").exists());
    assert!(!fixture.options.native_home.join("probe-inspected").exists());
    for timeout in [Duration::ZERO, Duration::from_secs(31)] {
        assert!(matches!(
            backend.probe_owned_host(timeout).await,
            Err(Error::InvalidCommand)
        ));
    }
    assert!(backend.stop().unwrap().exited);
}

#[tokio::test]
async fn probe_timeout_proves_nothing_and_does_not_reauthenticate_or_restart() {
    let (fixture, backend, client) = connected("timeout").await;
    assert!(matches!(
        backend.probe_owned_host(Duration::from_millis(25)).await,
        Err(Error::RequestTimeout)
    ));
    assert_eq!(
        fs::read_to_string(fixture.options.native_home.join("exchange-count")).unwrap(),
        "1"
    );
    assert!(fixture.options.native_home.join("probe-inspected").exists());
    client.close().await;
    assert!(backend.stop().unwrap().exited);
}

#[tokio::test]
async fn proof_rechecks_stop_after_a_matching_original_pipe_ack() {
    let (fixture, backend, client) = connected("delayed").await;
    let copy = Arc::clone(&backend);
    let pending = tokio::spawn(async move { copy.probe_owned_host(Duration::from_secs(1)).await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while !fixture.options.native_home.join("probe-inspected").exists() {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    // Revoke Core ownership without disposing the fixture, so its valid ACK still arrives.
    backend.inner.begin_stop();
    assert!(matches!(pending.await.unwrap(), Err(Error::Stopping)));
    assert!(matches!(
        client.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    assert!(backend.stop().unwrap().exited);
}

#[tokio::test]
async fn wrong_inspection_ack_faults_and_revokes_cached_client() {
    let (_fixture, backend, client) = connected("wrong").await;
    assert!(matches!(
        backend.probe_owned_host(Duration::from_secs(1)).await,
        Err(Error::Protocol)
    ));
    assert!(matches!(
        client.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    assert!(backend.stop().unwrap().exited);
}

#[tokio::test]
async fn protocol_fault_revokes_transport_before_monitor_can_observe_it() {
    let (_fixture, backend, client) = connected("ack").await;
    backend.inner.protocol_fault();
    assert!(
        backend
            .inner
            .state
            .0
            .lock()
            .unwrap()
            .transport_cancel
            .is_cancelled()
    );
    assert!(matches!(
        client.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    assert!(
        backend
            .probe_owned_host(Duration::from_secs(1))
            .await
            .is_err()
    );
    assert!(backend.stop().unwrap().exited);
}

#[tokio::test]
async fn private_stdout_eof_revokes_cached_client_and_cannot_prove_owner_liveness() {
    let (_fixture, backend, client) = connected("eof").await;
    assert!(
        backend
            .probe_owned_host(Duration::from_millis(100))
            .await
            .is_err()
    );
    assert!(matches!(
        client.session_list().await,
        Err(dsh_native_transport::Error::Closed)
    ));
    assert!(
        backend
            .probe_owned_host(Duration::from_secs(1))
            .await
            .is_err()
    );
    assert!(backend.stop().unwrap().exited);
}

#[tokio::test]
async fn poisoned_writer_cannot_admit_same_host_proof() {
    let (fixture, backend, client) = connected("ack").await;
    backend.inner.poisoned.store(true, Ordering::Release);
    assert!(matches!(
        backend.probe_owned_host(Duration::from_secs(1)).await,
        Err(Error::ControlClosed)
    ));
    assert!(!fixture.options.native_home.join("probe-inspected").exists());
    client.close().await;
    assert!(backend.stop().unwrap().exited);
}
