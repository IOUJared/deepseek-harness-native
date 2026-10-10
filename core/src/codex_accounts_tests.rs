use crate::*;
use serde_json::{Value, json};
use std::{fs, time::Duration};

fn staged_account_fixture(script: &str) -> (tempfile::TempDir, Backend) {
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
fn unknown_select_ack_blocks_writes_until_explicit_reconciliation_and_does_not_replay() {
    let fixture = r#"
const send=v=>process.stdout.write('DSH_TAURI:'+JSON.stringify(v)+'\n');
const id='account-11111111-1111-4111-8111-111111111111';
let value={mode:'managed',accounts:[{opaqueId:id,label:'Personal',email:null,plan:null}],selected:null,epoch:0,legacy:'absent',added:null};
let selects=0;
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let pending='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>{pending+=chunk;while(pending.includes('\n')){const at=pending.indexOf('\n');const c=JSON.parse(pending.slice(0,at));pending=pending.slice(at+1);
 if(c.type==='shutdown'){send({type:'shutdown-complete'});process.exit(0);}
 if(c.type==='codex-select'){selects++;if(selects!==1)throw Error('replayed selection');value={...value,selected:id,epoch:1};const captured=value;setTimeout(()=>send({type:'native-result',requestId:c.requestId,command:c.type,value:captured}),200);}
 else if(c.type==='codex-signout-all'){if(c.expectedEpoch!==1)throw Error('stale deletion');value={...value,accounts:[],selected:null,epoch:2};send({type:'native-result',requestId:c.requestId,command:c.type,value});}
 else if(c.type==='codex-accounts-status'){send({type:'native-result',requestId:c.requestId,command:c.type,value});}
}});
"#;
    let (_directory, backend) = staged_account_fixture(fixture);
    let command = Command::CodexSelect {
        account_id: CodexAccountId::new(ID.into()).unwrap(),
        expected_epoch: 0,
    };
    assert_eq!(
        backend.request(command.clone(), Duration::from_millis(20)),
        Err(Error::RequestTimeout)
    );
    assert_eq!(
        backend.request(command, Duration::from_secs(1)),
        Err(Error::RequestFailed)
    );
    assert_eq!(
        backend.request(
            Command::CodexSignOutAll { expected_epoch: 1 },
            Duration::from_secs(1)
        ),
        Err(Error::RequestFailed)
    );
    let PublicReply::CodexAccounts(status) = backend
        .request(Command::CodexAccountsStatus, Duration::from_secs(1))
        .unwrap()
    else {
        panic!("account metadata expected");
    };
    assert_eq!(status.epoch, Some(1));
    assert_eq!(status.selected.unwrap().as_str(), ID);
    let PublicReply::CodexAccounts(signed_out) = backend
        .request(
            Command::CodexSignOutAll { expected_epoch: 1 },
            Duration::from_secs(1),
        )
        .unwrap()
    else {
        panic!("account metadata expected");
    };
    assert!(signed_out.accounts.is_empty());
    assert!(backend.stop().unwrap().exited);
}

#[test]
fn selection_reply_cannot_silently_substitute_a_different_account() {
    let fixture = r#"
const send=v=>process.stdout.write('DSH_TAURI:'+JSON.stringify(v)+'\n');
const ids=['account-11111111-1111-4111-8111-111111111111','account-22222222-2222-4222-9222-222222222222'];
send({type:'ready',url:'http://127.0.0.1:49876/?token=PUBLIC_FAKE_LAUNCH',version:'0.2.1-alpha.1',profile:'desktop'});
let pending='';process.stdin.setEncoding('utf8');process.stdin.on('data',chunk=>{pending+=chunk;while(pending.includes('\n')){const at=pending.indexOf('\n');const c=JSON.parse(pending.slice(0,at));pending=pending.slice(at+1);
 if(c.type==='shutdown'){send({type:'shutdown-complete'});process.exit(0);}
 send({type:'native-result',requestId:c.requestId,command:c.type,value:{mode:'managed',accounts:ids.map(opaqueId=>({opaqueId,label:'Personal',email:null,plan:null})),selected:ids[1],epoch:1,legacy:'absent',added:null}});
}});
"#;
    let (_directory, backend) = staged_account_fixture(fixture);
    let command = Command::CodexSelect {
        account_id: CodexAccountId::new(ID.into()).unwrap(),
        expected_epoch: 0,
    };
    assert_eq!(
        backend.request(command.clone(), Duration::from_secs(1)),
        Err(Error::Protocol)
    );
    assert_eq!(
        backend.request(command, Duration::from_secs(1)),
        Err(Error::RequestFailed)
    );
    assert!(backend.stop().unwrap().exited);
}

const ID: &str = "account-11111111-1111-4111-8111-111111111111";
const OTHER: &str = "account-22222222-2222-4222-9222-222222222222";
fn sample() -> Value {
    json!({"mode":"managed", "accounts":[{"opaqueId":ID,"label":"PUBLIC_PRIVATE_LABEL","email":null,"plan":null}],
        "selected":ID,"epoch":3,"legacy":"unadopted","added":{"accountId":ID,"epoch":1,"attempt":7}})
}
fn reply(command: &str, value: Value) -> Result<crate::protocol::Event, Error> {
    let frame = format!(
        "DSH_TAURI:{}",
        json!({"type":"native-result","requestId":"1","command":command,"value":value})
    );
    crate::protocol::parse(frame.as_bytes(), "0.2.1-alpha.1")
}

#[test]
fn account_ids_are_private_branded_lowercase_random_uuid_v4_not_issuer_ids() {
    let id = CodexAccountId::new(ID.into()).unwrap();
    assert_eq!(id.as_str(), ID);
    assert_eq!(format!("{id:?}"), "CodexAccountId([REDACTED])");
    for raw in [
        "issuer-id".into(),
        "PUBLIC_FAKE_TOKEN".into(),
        ID.to_uppercase(),
        ID.replace("-4111-", "-1111-"),
        ID.replace("-8111-", "-7111-"),
        ID.to_owned() + "\n",
    ] {
        assert_eq!(CodexAccountId::new(raw), Err(Error::InvalidCommand));
    }
}

#[test]
fn labels_bound_unicode_scalars_bytes_and_controls_and_redact_debug() {
    let label = CodexAccountLabel::new("PUBLIC_PRIVATE_LABEL".into()).unwrap();
    assert_eq!(label.as_str(), "PUBLIC_PRIVATE_LABEL");
    assert_eq!(format!("{label:?}"), "CodexAccountLabel([REDACTED])");
    assert!(CodexAccountLabel::new("😀".repeat(128)).is_ok());
    let padded = CodexAccountLabel::new(" PUBLIC label ".into()).unwrap();
    assert_eq!(padded.as_str(), " PUBLIC label ");
    let mut stored = sample();
    stored["accounts"][0]["label"] = json!(" PUBLIC label ");
    assert_eq!(
        crate::codex::accounts_metadata(&stored).unwrap().accounts[0]
            .label
            .as_str(),
        " PUBLIC label "
    );
    for raw in [
        "".into(),
        " ".into(),
        "\u{a0}".into(),
        "x\ny".into(),
        "x\u{202e}y".into(),
        "x\u{1343f}y".into(),
        "x\u{e007f}y".into(),
        "x\u{2028}y".into(),
        "😀".repeat(129),
    ] {
        assert_eq!(CodexAccountLabel::new(raw), Err(Error::InvalidCommand));
    }
}

#[test]
fn managed_accounts_are_non_copy_closed_metadata_with_exact_add_receipt_and_redacted_debug() {
    let parsed = crate::codex::accounts_metadata(&sample()).unwrap();
    assert_eq!(parsed.mode, CodexAccountMode::Managed);
    assert_eq!(parsed.epoch, Some(3));
    assert_eq!(parsed.selected.as_ref().unwrap().as_str(), ID);
    assert_eq!(parsed.added.as_ref().unwrap().epoch, 1);
    assert_eq!(parsed.added.as_ref().unwrap().attempt, 7);
    let debug = format!("{parsed:?}");
    assert!(!debug.contains(ID));
    assert!(!debug.contains("PUBLIC_PRIVATE_LABEL"));
    for command in [
        "codex-accounts-status",
        "codex-select",
        "codex-signout",
        "codex-signout-all",
    ] {
        let crate::protocol::Event::Reply {
            reply: Ok(PublicReply::CodexAccounts(value)),
            ..
        } = reply(command, sample()).unwrap()
        else {
            panic!("typed managed accounts expected");
        };
        assert!(crate::protocol::matches_reply(
            command,
            &PublicReply::CodexAccounts(value)
        ));
        assert!(!crate::protocol::matches_reply(
            command,
            &PublicReply::CodexCallbackAccepted(true)
        ));
    }
}

#[test]
fn managed_parser_rejects_extra_fields_fabricated_identity_and_invalid_receipt_or_selection() {
    let mut invalid = Vec::new();
    let mut token = sample();
    token["token"] = json!("PUBLIC_FAKE_TOKEN");
    invalid.push(token);
    for (field, value) in [
        ("token", json!("PUBLIC_FAKE_TOKEN")),
        ("email", json!("fake@example.test")),
        ("plan", json!("Pro")),
        ("opaqueId", json!("issuer-id")),
        ("label", json!(" ")),
    ] {
        let mut row = sample();
        row["accounts"][0][field] = value;
        invalid.push(row);
    }
    for (field, value) in [
        ("mode", json!("unknown")),
        ("selected", json!(OTHER)),
        ("epoch", Value::Null),
        ("epoch", json!(MAXIMUM_PLUS_ONE)),
        ("legacy", json!("adopted")),
    ] {
        let mut row = sample();
        row[field] = value;
        invalid.push(row);
    }
    for (field, value) in [
        ("token", json!("PUBLIC_FAKE_TOKEN")),
        ("accountId", json!(OTHER)),
        ("epoch", json!(false)),
        ("epoch", json!(4)),
        ("attempt", json!(0)),
        ("attempt", json!(MAXIMUM_PLUS_ONE)),
    ] {
        let mut row = sample();
        row["added"][field] = value;
        invalid.push(row);
    }
    let mut duplicate = sample();
    duplicate["accounts"]
        .as_array_mut()
        .unwrap()
        .push(sample()["accounts"][0].clone());
    invalid.push(duplicate);
    let mut oversized = sample();
    oversized["accounts"] = json!(vec![sample()["accounts"][0].clone(); 129]);
    invalid.push(oversized);
    for value in invalid {
        assert_eq!(
            crate::codex::accounts_metadata(&value),
            Err(Error::Protocol)
        );
    }
}
const MAXIMUM_PLUS_ONE: u64 = 9_007_199_254_740_992;

#[test]
fn zero_epoch_first_add_receipt_and_complete_account_bounds_are_enforced() {
    let mut fresh = sample();
    fresh["epoch"] = json!(0);
    fresh["added"]["epoch"] = json!(0);
    assert_eq!(
        crate::codex::accounts_metadata(&fresh)
            .unwrap()
            .added
            .unwrap()
            .epoch,
        0
    );
    for epoch in [
        json!(false),
        json!(true),
        json!(0.5),
        json!(MAXIMUM_PLUS_ONE),
    ] {
        let mut invalid = fresh.clone();
        invalid["epoch"] = epoch;
        assert_eq!(
            crate::codex::accounts_metadata(&invalid),
            Err(Error::Protocol)
        );
    }
    let rows: Vec<_> = (0..128).map(|index| json!({"opaqueId":format!("account-{index:08x}-1111-4111-8111-111111111111"),"label":"Personal","email":null,"plan":null})).collect();
    let mut bounded = json!({"mode":"managed","accounts":rows,"selected":null,"epoch":0,"legacy":"absent","added":null});
    assert_eq!(
        crate::codex::accounts_metadata(&bounded)
            .unwrap()
            .accounts
            .len(),
        128
    );
    for row in bounded["accounts"].as_array_mut().unwrap() {
        row["label"] = json!("😀".repeat(128));
    }
    assert_eq!(
        crate::codex::accounts_metadata(&bounded),
        Err(Error::Protocol)
    );
}

#[test]
fn private_commands_encode_only_branded_identity_label_and_explicit_safe_epoch() {
    let id = CodexAccountId::new(ID.into()).unwrap();
    let label = CodexAccountLabel::new("Personal".into()).unwrap();
    let commands = [
        (
            Command::CodexAccountsStatus,
            json!({"type":"codex-accounts-status","requestId":"1"}),
        ),
        (
            Command::CodexAdd {
                label,
                expected_epoch: 0,
            },
            json!({"type":"codex-add","requestId":"1","label":"Personal","expectedEpoch":0}),
        ),
        (
            Command::CodexSelect {
                account_id: id.clone(),
                expected_epoch: 1,
            },
            json!({"type":"codex-select","requestId":"1","accountId":ID,"expectedEpoch":1}),
        ),
        (
            Command::CodexSignOut {
                account_id: id,
                expected_epoch: 2,
            },
            json!({"type":"codex-signout","requestId":"1","accountId":ID,"expectedEpoch":2}),
        ),
        (
            Command::CodexSignOutAll { expected_epoch: 3 },
            json!({"type":"codex-signout-all","requestId":"1","expectedEpoch":3}),
        ),
    ];
    for (command, packet) in commands {
        assert_eq!(command.encode("1").unwrap(), packet);
    }
    assert_eq!(
        Command::CodexSignOutAll {
            expected_epoch: MAXIMUM_PLUS_ONE
        }
        .encode("1"),
        Err(Error::InvalidCommand)
    );
}

#[test]
fn legacy_accounts_result_has_no_invented_identity_or_epoch() {
    let value = json!({"mode":"legacy","accounts":[],"selected":null,"epoch":null,"legacy":"absent","added":null});
    let parsed = crate::codex::accounts_metadata(&value).unwrap();
    assert_eq!(parsed.mode, CodexAccountMode::Legacy);
    assert_eq!(parsed.epoch, None);
    let mut invalid = value;
    invalid["epoch"] = json!(0);
    assert_eq!(
        crate::codex::accounts_metadata(&invalid),
        Err(Error::Protocol)
    );
}

#[test]
fn codex_epoch_conflict_is_an_allowlisted_error_not_a_provider_diagnostic() {
    let frame = b"DSH_TAURI:{\"type\":\"request-error\",\"requestId\":\"1\",\"code\":\"CODEX_EPOCH_CONFLICT\",\"message\":\"PUBLIC_FAKE_TOKEN\"}";
    assert!(matches!(
        crate::protocol::parse(frame, "0.2.1-alpha.1"),
        Ok(crate::protocol::Event::Reply {
            reply: Err(Error::CodexEpochConflict),
            ..
        })
    ));
}
