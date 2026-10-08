use super::*;
use dsh_native_transport::dto::SessionId;
const SCRIPT: &str = r#"
const send = value => process.stdout.write('DSH_TAURI:' + JSON.stringify(value) + '\n');
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FIXTURE',version:'0.2.1-alpha.1',profile:'desktop'});
let pending='';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => {
 pending+=chunk;
 while(pending.includes('\n')) {
  const at=pending.indexOf('\n'); const c=JSON.parse(pending.slice(0,at)); pending=pending.slice(at+1);
  if(c.type==='shutdown'){send({type:'shutdown-complete'});process.exit(0);}
  else send({type:'native-result',requestId:c.requestId,command:c.type,value:RESULT});
 }
});
"#;
#[test]
fn private_discovery_request_echo_and_reply_are_correlated() {
    for (value, valid) in [
        (r#"{"sessionId":"selected","agentId":"registered"}"#, true),
        (r#"{"sessionId":"selected","agentId":null}"#, true),
        (r#"{"sessionId":"other","agentId":"registered"}"#, false),
        (
            r#"{"sessionId":"selected","agentId":"registered","extra":true}"#,
            false,
        ),
    ] {
        let fixture = Fixture::new();
        let (backend, _) = fixture.spawn(&SCRIPT.replace("RESULT", value));
        backend.wait_ready().unwrap();
        let reply = backend.request(
            Command::DiscoverSessionAgent(SessionId::new("selected").unwrap()),
            Duration::from_secs(1),
        );
        if valid {
            let PublicReply::AgentDiscovery(found) = reply.unwrap() else {
                panic!("discovery");
            };
            assert_eq!(found.session_id.as_str(), "selected");
            assert_eq!(
                found.agent_id.as_ref().map(|id| id.as_str()),
                if value.contains("null") {
                    None
                } else {
                    Some("registered")
                }
            );
        } else {
            assert_eq!(reply, Err(Error::Protocol));
        }
        assert!(backend.stop().unwrap().exited);
    }
}
