import test from 'node:test';
import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, readFile, symlink, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createCodexAppServer, CodexAppServerError } from './codex-app-server.mjs';

// PUBLIC synthetic subprocesses only: no Codex binary, OAuth, provider or Cargo.
const evidenceParent = resolve(dirname(fileURLToPath(import.meta.url)), '../evidence');
await mkdir(evidenceParent, { recursive: true, mode: 0o700 });
const evidence = await mkdtemp(join(evidenceParent, 'codex-app-server-client-'));
const traces = [];

const fixtureSource = mode => `#!${process.execPath}
import { appendFileSync, writeFileSync } from 'node:fs';
import { createInterface } from 'node:readline';
const mode = ${JSON.stringify(mode)};
const trace = process.env.CODEX_HOME + '/PUBLIC-requests.jsonl';
writeFileSync(process.env.CODEX_HOME + '/PUBLIC-spawned', 'PUBLIC');
let initialized = false, experimental = false, n = 0, held = null, refreshId = null;
const send = value => process.stdout.write(JSON.stringify(value) + '\\n');
const result = (id, value) => send({id, result:value});
const notice = params => send({method:'account/updated',params});
const normal = request => result(request.id,{PUBLIC:true,method:request.method,params:request.params,n,initialized});
const lines = createInterface({input:process.stdin});
if (mode === 'stubborn') { process.on('SIGTERM',()=>{}); setInterval(()=>{},1000); }
if (mode === 'unread') setInterval(()=>{},1000);
lines.on('close',()=>{ if (mode !== 'stubborn') process.exit(0); });
lines.on('line',line=>{
  const request = JSON.parse(line);
  appendFileSync(trace, JSON.stringify(request)+'\\n');
  if (request.method === 'initialize') {
    experimental = request.params.capabilities.experimentalApi === true;
    if (mode === 'no-init') return;
    if (mode === 'init-eof') { process.stdout.end(); return; }
    if (mode === 'init-partial') { process.stdout.write('{"PUBLIC":'); process.stdout.end(); return; }
    if (mode === 'init-bad-utf8') { process.stdout.write(Buffer.from([0xc3,0x28,10])); return; }
    if (mode === 'init-oversize') { process.stdout.write('x'.repeat(8193)); return; }
    const response = {codexHome:process.env.CODEX_HOME,platformFamily:'unix',platformOs:'linux',
      userAgent:request.params.clientInfo.name+'/0.160.1 (PUBLIC Linux)'};
    if (mode === 'wrong-version') response.userAgent = request.params.clientInfo.name+'/0.160.0 (PUBLIC)';
    if (mode === 'wrong-home') response.codexHome = '/PUBLIC-not-owned';
    if (mode === 'wrong-platform') response.platformOs = 'PUBLIC-unsupported';
    if (mode === 'early-refresh') {
      notice({PUBLICStartup:true});
      send({id:'PUBLIC-early',method:'account/chatgptAuthTokens/refresh',params:{reason:'unauthorized'}});
    }
    result(request.id,response); return;
  }
  if (request.method === 'initialized') {
    initialized = true;
    if (mode === 'unread') { lines.pause(); process.stdin.pause(); notice({PUBLICUnread:true}); }
    return;
  }
  if (!request.method) {
    notice({PUBLICReply:request});
    if (mode === 'refresh-replay-completed') send({id:refreshId,method:'account/chatgptAuthTokens/refresh',params:{reason:'unauthorized'}});
    if (held) { normal(held); held = null; }
    return;
  }
  n++;
  if (request.method === 'account/login/start' && request.params.type === 'chatgptAuthTokens' && !experimental)
    return send({id:request.id,error:{code:-32600,message:'PUBLIC requires experimentalApi capability'}});
  if (mode === 'environment') return result(request.id,{env:process.env,cwd:process.cwd(),argv:process.argv.slice(2),initialized});
  if (mode === 'notifications') {
    send({method:'PUBLIC/unknown',params:{secret:'PUBLIC-do-not-dispatch'}});
    notice({PUBLIC:true,n});
    send({method:'account/login/completed',params:{loginId:'PUBLIC-login',success:true,error:null}});
    send({method:'account/rateLimits/updated',params:{rateLimits:{PUBLIC:true}}});
    return normal(request);
  }
  if (mode === 'rpc-error') return send({id:request.id,error:{code:-32000,message:'PUBLIC_SECRET_DIAGNOSTIC',data:{token:'PUBLIC_SECRET_DIAGNOSTIC'}}});
  if (mode === 'hang') { notice({PUBLICReceipt:n}); return; }
  if (mode === 'late-replies') {
    const count=n;
    setTimeout(()=>{result(request.id,{PUBLIC:true,n:count});result(request.id,{PUBLIC:true,n:999});},count===1?120:60);
    return;
  }
  if (mode === 'eof') { process.stdout.end(); return; }
  if (mode === 'partial-eof') { process.stdout.write('{"PUBLIC":'); process.stdout.end(); return; }
  if (mode === 'exit') { process.exit(7); return; }
  if (mode === 'oversize') { process.stdout.write('x'.repeat(8193)+'\\n'); return; }
  if (mode === 'bad-utf8') { process.stdout.write(Buffer.from([0x22,0xc3,0x28,0x22,10])); return; }
  if (mode === 'invalid-json') { process.stdout.write('PUBLIC-not-json\\n'); return; }
  if (mode === 'unknown-reply') return result('native-9000000',{PUBLIC:true});
  if (mode === 'ambiguous-reply') return send({id:request.id,result:{},error:{code:-1,message:'PUBLIC'}});
  if (mode === 'numeric-reply') return result(4,{PUBLIC:true});
  if (mode === 'utf8-fragments') {
    const buf=Buffer.from(JSON.stringify({id:request.id,result:{PUBLIC:'hello 🌹 café'}})+'\\n');
    const at=buf.indexOf(Buffer.from('🌹'))+1;
    process.stdout.write(buf.subarray(0,at)); setTimeout(()=>process.stdout.write(buf.subarray(at)),5); return;
  }
  if (mode.startsWith('refresh') || mode === 'unknown-server-request') {
    held=request;
    refreshId=mode==='refresh-number'?0:mode==='refresh-uncooperative'?'PUBLIC-refresh-'+n:'PUBLIC-refresh-1';
    const packet={id:refreshId,method:mode==='unknown-server-request'?'PUBLIC/unknown':'account/chatgptAuthTokens/refresh',
      params:{reason:mode==='refresh-invalid'?'PUBLIC-invalid':'unauthorized',previousAccountId:'PUBLIC-account'}};
    send(packet);
    if (mode==='refresh-replay-active') send(packet);
    if (mode==='refresh-limit') send({...packet,id:'PUBLIC-refresh-2'});
    if (mode==='refresh-history-limit') send({...packet,id:'PUBLIC-refresh-2',method:'PUBLIC/unknown'});
    return;
  }
  if (mode === 'natural-close') { normal(request); setImmediate(()=>process.exit(0)); return; }
  normal(request);
});
`;

async function makeFixture(mode, overrides = {}) {
  const root = await mkdtemp(join(evidence, `${mode}-`));
  const codexHome = join(root, 'home');
  const nativeCwd = join(root, 'cwd');
  await mkdir(codexHome, { mode: 0o700 });
  await mkdir(nativeCwd, { mode: 0o700 });
  const absoluteCliPath = join(root, 'PUBLIC-codex-fixture.mjs');
  await writeFile(absoluteCliPath, fixtureSource(mode), { mode: 0o700, flag: 'wx' });
  traces.push(root);
  const options = {
    absoluteCliPath, codexHome, nativeCwd, expectedCliVersion: '0.160.1', credentialStorage: 'ephemeral',
    enableExperimentalChatgptTokens: false,
    clientInfo: { name: 'native_public_fixture', version: 'PUBLIC-test', title: 'PUBLIC fixture' },
    maxFrameBytes: 8192, maxPendingRequests: 8, maxPendingBytes: 65536,
    maxServerRequests: 4, maxServerRequestIds: 16, maxRequestIdBytes: 128,
    startupTimeoutMs: 2000, requestTimeoutMs: 2000, writeTimeoutMs: 1000,
    serverRequestTimeoutMs: 1000, stopGraceMs: 100, stopTermMs: 100, stopKillMs: 1000,
    onNotification() {}, ...overrides,
  };
  const client = createCodexAppServer(options);
  return { client, options, root, codexHome, nativeCwd, absoluteCliPath };
}

async function owned(t, mode, overrides) {
  const fixture = await makeFixture(mode, overrides);
  t.after(async () => {
    await fixture.client.stop();
    assert.equal(fixture.client.lifecycle.pendingRequests, 0);
    assert.equal(fixture.client.lifecycle.serverRequests, 0);
    assert.equal(fixture.client.lifecycle.pendingBytes, 0);
  });
  return fixture;
}
const ownedExperimental = (t, mode, overrides = {}) => owned(t, mode, { ...overrides, enableExperimentalChatgptTokens: true });
const start = client => client.start({ explicitUserAction: true });
const rejectsCode = (promise, code, indeterminate) => assert.rejects(promise, cause => {
  assert.ok(cause instanceof CodexAppServerError);
  assert.equal(cause.code, code);
  if (indeterminate !== undefined) assert.equal(cause.indeterminate, indeterminate);
  assert.ok(!JSON.stringify(cause).includes('PUBLIC_SECRET_DIAGNOSTIC'));
  assert.ok(!cause.message.includes('PUBLIC_SECRET_DIAGNOSTIC'));
  return true;
});
async function requests(home) {
  const trace = await readFile(join(home, 'PUBLIC-requests.jsonl'), 'utf8');
  return trace.trim().split('\n').map(line => JSON.parse(line));
}

// A written frame is logged only when the fixture's reader consumes it, and `writeFrame`
// resolves at the child's stdin, not at its log. A count taken straight after the handshake can
// therefore miss `initialized` on a loaded machine (measured 10/10 failures under CPU load
// before this wait). The wait bounds that drain; the assertion stays exact, so a frame that
// never arrives still fails the test.
async function settledRequests(home, expected) {
  let rows = await requests(home);
  for (let attempt = 0; rows.length < expected && attempt < 400; attempt++) {
    await new Promise(resolve => setTimeout(resolve, 5));
    rows = await requests(home);
  }
  return rows;
}

// Only the two source files are durable product edits; this generated evidence is ignored.
test('construction and account methods stay inert until explicit action; start coalesces and stop is idempotent', async t => {
  const { client, codexHome } = await owned(t, 'normal');
  assert.equal(client.lifecycle.state, 'idle');
  await rejectsCode(client.readAccount({ refreshToken: false }), 'NOT_READY');
  await rejectsCode(client.start(), 'EXPLICIT_ACTION_REQUIRED');
  await rejectsCode(client.start({ explicitUserAction: false }), 'EXPLICIT_ACTION_REQUIRED');
  await assert.rejects(readFile(join(codexHome, 'PUBLIC-spawned')), { code: 'ENOENT' });
  const first = start(client);
  assert.equal(first, start(client));
  const handshake = await first;
  assert.equal(handshake.codexHome, codexHome);
  assert.equal(client.lifecycle.state, 'ready');
  assert.equal((await client.readAccount({ refreshToken: false })).initialized, true);
  await client.readRateLimits();
  await client.startLogin({ type: 'chatgpt', appBrand: 'codex', codexStreamlinedLogin: true, useHostedLoginSuccessPage: false });
  await client.cancelLogin('PUBLIC-login');
  await client.logout();
  const trace = await requests(codexHome);
  assert.deepEqual(trace.map(row => row.method), ['initialize', 'initialized', 'account/read', 'account/rateLimits/read', 'account/login/start', 'account/login/cancel', 'account/logout']);
  assert.deepEqual(trace[0].params.capabilities, { experimentalApi: false, explicitGatewayOauth: true, requestAttestation: false });
  assert.deepEqual(trace[3].params, { supportsLunaReserve: false, excludeResetCreditDetails: true });
  assert.equal(new Set(trace.filter(row => row.id).map(row => row.id)).size, trace.filter(row => row.id).length);
  assert.equal(client.request, undefined);
  assert.equal(client.handle, undefined);
  const stopped = client.stop();
  assert.equal(stopped, client.stop());
  assert.deepEqual(await stopped, { exitObserved: true, closeObserved: true, exitCode: 0, signal: null, forced: false, timedOut: false });
  await rejectsCode(client.logout(), 'NOT_READY');
  await rejectsCode(start(client), 'UNAVAILABLE');
});

test('explicit environment has no parent credentials, browser or Node configuration', async t => {
  const originals = Object.fromEntries(['OPENAI_API_KEY', 'CODEX_ACCESS_TOKEN', 'NODE_OPTIONS', 'BROWSER', 'HTTP_PROXY'].map(key => [key, process.env[key]]));
  for (const key of Object.keys(originals)) process.env[key] = 'PUBLIC_SENTINEL_NOT_INHERITED';
  t.after(() => { for (const [key, value] of Object.entries(originals)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; } });
  const { client, codexHome, nativeCwd } = await owned(t, 'environment');
  await start(client);
  const result = await client.readAccount({ refreshToken: false });
  assert.deepEqual(result.env, { PATH: '/usr/bin:/bin', HOME: codexHome, CODEX_HOME: codexHome,
    XDG_CONFIG_HOME: codexHome, XDG_DATA_HOME: codexHome, XDG_CACHE_HOME: codexHome });
  assert.equal(result.cwd, nativeCwd);
  assert.deepEqual(result.argv, ['-c', 'cli_auth_credentials_store="ephemeral"', 'app-server', '--listen', 'stdio://']);
});

test('required tunables, pinned version, absolute normalized paths and callbacks fail upfront', async t => {
  const { options } = await owned(t, 'normal');
  for (const field of ['maxFrameBytes', 'maxPendingRequests', 'maxPendingBytes', 'maxServerRequests', 'maxServerRequestIds', 'maxRequestIdBytes',
    'startupTimeoutMs', 'requestTimeoutMs', 'writeTimeoutMs', 'serverRequestTimeoutMs', 'stopGraceMs', 'stopTermMs', 'stopKillMs']) {
    for (const value of [undefined, 0, -1, 1.5, NaN, Infinity]) assert.throws(() => createCodexAppServer({ ...options, [field]: value }), { code: 'INVALID_OPTIONS' });
  }
  for (const patch of [{ absoluteCliPath: 'codex' }, { codexHome: '/a/../b' }, { nativeCwd: '/a\nb' }, { expectedCliVersion: '0.160.0' }, { credentialStorage: undefined }, { credentialStorage: 'auto' }, { credentialStorage: 'keyring' },
    { enableExperimentalChatgptTokens: undefined }, { enableExperimentalChatgptTokens: null },
    { enableExperimentalChatgptTokens: 'true' }, { enableExperimentalChatgptTokens: 1 },
    { enableExperimentalChatgptTokens: true, serverRequestTimeoutMs: 10000 },
    { env: process.env }, { logger() {} }, { clientInfo: { name: 'bad\nname', version: 'PUBLIC' } }, { onNotification: null },
    { onAuthTokensRefresh: true }, { maxFrameBytes: 511 }, { maxFrameBytes: 16777217 }, { maxPendingBytes: 16385 }, { maxServerRequests: 17 }]) {
    assert.throws(() => createCodexAppServer({ ...options, ...patch }), { code: 'INVALID_OPTIONS' });
  }
});

test('launch validates current-user private canonical directories and regular executable', async t => {
  const fixtures = [];
  for (const condition of ['home-mode', 'cwd-mode', 'home-link', 'missing', 'executable-directory', 'not-executable']) {
    const fixture = await owned(t, 'normal');
    let options = { ...fixture.options };
    if (condition === 'home-mode') await chmod(fixture.codexHome, 0o755);
    if (condition === 'cwd-mode') await chmod(fixture.nativeCwd, 0o755);
    if (condition === 'home-link') { const path = join(fixture.root, 'PUBLIC-symlink'); await symlink(fixture.codexHome, path); options.codexHome = path; }
    if (condition === 'missing') options.absoluteCliPath = join(fixture.root, 'PUBLIC-missing');
    if (condition === 'executable-directory') options.absoluteCliPath = fixture.nativeCwd;
    if (condition === 'not-executable') await chmod(fixture.absoluteCliPath, 0o600);
    const client = createCodexAppServer(options);
    fixtures.push(client);
    await assert.rejects(start(client), cause => cause instanceof CodexAppServerError && cause.code.startsWith('INVALID_'));
    await assert.rejects(readFile(join(fixture.codexHome, 'PUBLIC-spawned')), { code: 'ENOENT' });
  }
  for (const client of fixtures) await client.stop();
});

for (const [mode, code] of [['no-init', 'STARTUP_TIMEOUT'], ['wrong-version', 'INITIALIZE_MISMATCH'], ['wrong-home', 'INITIALIZE_MISMATCH'],
  ['wrong-platform', 'INITIALIZE_MISMATCH'], ['init-eof', 'EOF'], ['init-partial', 'PARTIAL_EOF'], ['init-bad-utf8', 'INVALID_FRAME'], ['init-oversize', 'FRAME_LIMIT']]) {
  test(`initialize ${mode} fails closed and reaps the fake child`, async t => {
    const { client } = await owned(t, mode, { startupTimeoutMs: 100 });
    await rejectsCode(start(client), code);
    const stopped = await client.stop();
    assert.equal(stopped.closeObserved, true);
    assert.equal(client.lifecycle.state, 'stopped');
    await rejectsCode(client.readRateLimits(), 'NOT_READY');
  });
}

test('stop during async launch validation never allows a later spawn', async t => {
  const { client, codexHome } = await owned(t, 'normal');
  const starting = start(client);
  const failure = assert.rejects(starting, cause => cause instanceof CodexAppServerError);
  const result = await client.stop();
  await failure;
  assert.equal(result.closeObserved, false);
  await assert.rejects(readFile(join(codexHome, 'PUBLIC-spawned')), { code: 'ENOENT' });
});

test('only public login variants and exact params are admitted without generic dispatch', async t => {
  const { client, codexHome } = await owned(t, 'normal');
  await start(client);
  for (const value of [{ type: 'apiKey', apiKey: 'PUBLIC-synthetic-key' }, { type: 'chatgptDeviceCode' }, { type: 'chatgpt' }]) await client.startLogin(value);
  const before = (await requests(codexHome)).length;
  for (const value of [{ type: 'chatgptAuthTokens', accessToken: 'PUBLIC', chatgptAccountId: 'PUBLIC' }, { type: 'amazonBedrock', apiKey: 'PUBLIC', region: 'PUBLIC' },
    { type: 'apiKey', apiKey: '' }, { type: 'chatgpt', appBrand: 'PUBLIC' }, { type: 'chatgpt', codexStreamlinedLogin: 1 }, { type: 'chatgptDeviceCode', extra: true }]) {
    await assert.rejects(client.startLogin(value), cause => cause instanceof CodexAppServerError);
  }
  await rejectsCode(client.readAccount({}), 'INVALID_PARAMS');
  await rejectsCode(client.readAccount({ refreshToken: false, ambient: true }), 'INVALID_PARAMS');
  await rejectsCode(client.cancelLogin(''), 'INVALID_PARAMS');
  await rejectsCode(client.logout({ timeoutMs: 99 }), 'INVALID_PARAMS');
  assert.equal((await requests(codexHome)).length, before);
});

test('known notifications remain private; unrelated notifications and owner throws are contained', async t => {
  const notices = [];
  const { client } = await owned(t, 'notifications', { onNotification(notice) { notices.push(notice); if (notices.length === 1) throw new Error('PUBLIC_SECRET_DIAGNOSTIC'); } });
  await start(client);
  assert.equal((await client.readAccount({ refreshToken: false })).PUBLIC, true);
  assert.deepEqual(notices.map(row => row.method), ['account/updated', 'account/login/completed', 'account/rateLimits/updated']);
});

test('accidental async notification callbacks do not accumulate or leak rejection diagnostics', async t => {
  const { client } = await owned(t, 'notifications', { onNotification() { return Promise.reject(new Error('PUBLIC_SECRET_DIAGNOSTIC')); } });
  await start(client);
  await rejectsCode(client.readAccount({ refreshToken: false }), 'ASYNC_NOTIFICATION_CALLBACK');
  assert.equal((await client.stop()).closeObserved, true);
});

test('RPC rejection does not forward raw message, data, cause or stderr', async t => {
  const { client } = await owned(t, 'rpc-error');
  await start(client);
  await rejectsCode(client.logout(), 'RPC_REJECTED', true);
  assert.equal(client.lifecycle.state, 'ready');
});

test('fragmented multibyte UTF-8 survives JSON-lines framing', async t => {
  const { client } = await owned(t, 'utf8-fragments');
  await start(client);
  assert.deepEqual(await client.readAccount({ refreshToken: false }), { PUBLIC: 'hello 🌹 café' });
});

test('late and duplicate replies cannot settle a newer ID after timeout', async t => {
  const { client } = await owned(t, 'late-replies', { requestTimeoutMs: 90 });
  await start(client);
  await rejectsCode(client.readAccount({ refreshToken: false }), 'REQUEST_TIMEOUT', false);
  assert.deepEqual(await client.readAccount({ refreshToken: false }), { PUBLIC: true, n: 2 });
  assert.equal(client.lifecycle.state, 'ready');
});

test('abort abandons only its RPC, never claims mutation rollback or sends cancel', async t => {
  let receipt;
  const received = new Promise(resolve => { receipt = resolve; });
  const { client, codexHome } = await owned(t, 'hang', { onNotification: receipt });
  await start(client);
  const controller = new AbortController();
  const mutation = client.startLogin({ type: 'chatgpt' }, { signal: controller.signal });
  const assertion = rejectsCode(mutation, 'ABORTED', true);
  await received;
  controller.abort();
  await assertion;
  const pre = new AbortController(); pre.abort();
  await rejectsCode(client.logout({ signal: pre.signal }), 'ABORTED', false);
  assert.deepEqual((await settledRequests(codexHome, 3)).map(row => row.method), ['initialize', 'initialized', 'account/login/start']);
  assert.equal(client.lifecycle.pendingRequests, 0);
  assert.equal(client.lifecycle.state, 'ready');
});

test('mutation timeout and stop dispose every pending operation as indeterminate', async t => {
  const { client } = await owned(t, 'hang', { requestTimeoutMs: 50 });
  await start(client);
  await rejectsCode(client.readAccount({ refreshToken: true }), 'REQUEST_TIMEOUT', true);
  const pending = client.logout();
  const assertion = rejectsCode(pending, 'STOPPED', true);
  await client.stop();
  await assertion;
});

for (const [mode, code] of [['eof', 'EOF'], ['partial-eof', 'PARTIAL_EOF'], ['oversize', 'FRAME_LIMIT'], ['bad-utf8', 'INVALID_FRAME'],
  ['invalid-json', 'INVALID_FRAME'], ['unknown-reply', 'UNKNOWN_REPLY'], ['ambiguous-reply', 'INVALID_FRAME'], ['numeric-reply', 'INVALID_FRAME']]) {
  test(`ready ${mode} poisons and settles all pending RPCs`, async t => {
    const { client } = await owned(t, mode);
    await start(client);
    const one = client.logout(); const two = client.readRateLimits();
    await Promise.all([rejectsCode(one, code, true), rejectsCode(two, code, false)]);
    assert.equal((await client.stop()).closeObserved, true);
    await rejectsCode(client.logout(), 'NOT_READY');
  });
}

test('early process exit and normal exit retain distinct exit/close observation', async t => {
  const exited = await owned(t, 'exit');
  await start(exited.client);
  await assert.rejects(exited.client.logout(), cause => cause instanceof CodexAppServerError && ['EOF', 'PROCESS_EXIT'].includes(cause.code));
  const outcome = await exited.client.stop();
  assert.equal(outcome.exitObserved, true); assert.equal(outcome.closeObserved, true); assert.equal(outcome.exitCode, 7);
  assert.equal(outcome.forced, false);
  const normal = await owned(t, 'natural-close');
  await start(normal.client);
  await normal.client.logout();
  assert.equal((await normal.client.stop()).closeObserved, true);
});

test('pending count and outbound frame limits reject without transmission', async t => {
  let receipt;
  const received = new Promise(resolve => { receipt = resolve; });
  const { client, codexHome } = await owned(t, 'hang', { maxPendingRequests: 1, onNotification: receipt });
  await start(client);
  const pending = client.readRateLimits();
  const assertion = rejectsCode(pending, 'STOPPED');
  await received;
  await rejectsCode(client.logout(), 'PENDING_LIMIT', false);
  await client.stop(); await assertion;
  assert.equal((await settledRequests(codexHome, 3)).length, 3);
  const bounded = await owned(t, 'normal'); await start(bounded.client);
  await rejectsCode(bounded.client.startLogin({ type: 'apiKey', apiKey: 'PUBLIC'.repeat(2000) }), 'FRAME_LIMIT', false);
  assert.equal((await requests(bounded.codexHome)).filter(row => row.method === 'account/login/start').length, 0);
});

test('total pending serialized-byte reservations are bounded independently of request count', async t => {
  let receipt;
  const received = new Promise(resolve => { receipt = resolve; });
  const { client } = await owned(t, 'hang', { maxFrameBytes: 1024, maxPendingBytes: 2050, onNotification: receipt });
  await start(client);
  const first = client.startLogin({ type: 'apiKey', apiKey: 'PUBLIC'.repeat(150) });
  const assertion = rejectsCode(first, 'STOPPED', true);
  await received;
  assert.ok(client.lifecycle.pendingBytes > 900);
  await rejectsCode(client.startLogin({ type: 'apiKey', apiKey: 'PUBLIC'.repeat(150) }), 'PENDING_LIMIT', false);
  await client.stop(); await assertion;
});

for (const mode of ['refresh-string', 'refresh-number']) {
  test(`public refresh callback uses exact ${mode} correlation and returns only typed token fields`, async t => {
    const calls = [], replies = [];
    const { client } = await ownedExperimental(t, mode, {
      onNotification(notice) { replies.push(notice.params.PUBLICReply); },
      onAuthTokensRefresh(request) { calls.push(request); return { accessToken: 'PUBLIC-synthetic-access', chatgptAccountId: 'PUBLIC-account', chatgptPlanType: 'PUBLIC-plan' }; },
    });
    await start(client); await client.readAccount({ refreshToken: false });
    assert.equal(calls.length, 1);
    assert.equal(calls[0].requestId, mode === 'refresh-number' ? 0 : 'PUBLIC-refresh-1');
    assert.deepEqual(calls[0].params, { reason: 'unauthorized', previousAccountId: 'PUBLIC-account' });
    assert.equal(calls[0].signal.aborted, true);
    assert.deepEqual(replies, [{ id: calls[0].requestId, result: { accessToken: 'PUBLIC-synthetic-access', chatgptAccountId: 'PUBLIC-account', chatgptPlanType: 'PUBLIC-plan' } }]);
  });
}

test('unowned server methods never reach token callback', async t => {
  let calls = 0, reply;
  const { client } = await ownedExperimental(t, 'unknown-server-request', {
    onAuthTokensRefresh() { calls++; }, onNotification(notice) { reply = notice.params.PUBLICReply; },
  });
  await start(client); await client.readAccount({ refreshToken: false });
  assert.equal(calls, 0);
  assert.deepEqual(reply, { id: 'PUBLIC-refresh-1', error: { code: -32601, message: 'Request unavailable' } });
});

test('refresh failure diagnostics are sanitized; absent/invalid callback results never disclose tokens', async t => {
  for (const callback of [undefined, () => { throw new Error('PUBLIC_SECRET_DIAGNOSTIC'); }, () => ({ accessToken: 'PUBLIC-secret', chatgptAccountId: 'PUBLIC', arbitrary: true })]) {
    let reply;
    const { client } = await ownedExperimental(t, 'refresh-string', { onAuthTokensRefresh: callback, onNotification(notice) { reply = notice.params.PUBLICReply; } });
    await start(client); await client.readAccount({ refreshToken: false });
    assert.equal(reply.error.code, callback ? -32603 : -32601);
    assert.equal(reply.error.message, 'Request unavailable');
    assert.ok(!JSON.stringify(reply).includes('PUBLIC-secret'));
  }
});

test('refresh timeout aborts private callback and discards late completion exactly once', async t => {
  let finish, captured, replies = 0;
  const { client, codexHome } = await ownedExperimental(t, 'refresh-string', {
    serverRequestTimeoutMs: 30,
    onNotification() { replies++; },
    onAuthTokensRefresh(request) { captured = request; return new Promise(resolve => { finish = resolve; }); },
  });
  await start(client); await client.readAccount({ refreshToken: false });
  assert.equal(captured.signal.aborted, true);
  finish({ accessToken: 'PUBLIC-late', chatgptAccountId: 'PUBLIC-account' });
  await Promise.resolve(); await Promise.resolve();
  assert.equal(replies, 1);
  const responses = (await requests(codexHome)).filter(row => !row.method);
  assert.equal(responses.length, 1); assert.equal(responses[0].error.code, -32000);
});

for (const [mode, code, limits] of [['refresh-replay-active', 'SERVER_REQUEST_REPLAY', {}], ['refresh-replay-completed', 'SERVER_REQUEST_REPLAY', {}],
  ['refresh-invalid', 'INVALID_SERVER_REQUEST', {}], ['refresh-limit', 'SERVER_REQUEST_LIMIT', { maxServerRequests: 1 }],
  ['refresh-history-limit', 'SERVER_REQUEST_LIMIT', { maxServerRequestIds: 1, maxServerRequests: 1 }]]) {
  test(`${mode} fails closed without repeated callback dispatch`, async t => {
    let calls = 0;
    const { client } = await ownedExperimental(t, mode, { ...limits, onAuthTokensRefresh() {
      calls++;
      if (mode === 'refresh-replay-completed') return { accessToken: 'PUBLIC', chatgptAccountId: 'PUBLIC' };
      return new Promise(() => {});
    } });
    await start(client);
    await rejectsCode(client.readAccount({ refreshToken: false }), code);
    assert.equal((await client.stop()).closeObserved, true);
    assert.ok(calls <= 1);
  });
}

test('stop closes stdio, sends TERM/KILL only to unreaped owned child and observes close', async t => {
  const { client } = await owned(t, 'stubborn');
  await start(client);
  const outcome = await client.stop();
  assert.equal(outcome.forced, true); assert.equal(outcome.timedOut, true);
  assert.equal(outcome.signal, 'SIGKILL'); assert.equal(outcome.exitObserved, true); assert.equal(outcome.closeObserved, true);
  assert.equal(await client.stop(), outcome);
});

test('credential storage is an explicit CLI-only choice, never keyring/auto', async t => {
  const { client } = await owned(t, 'environment', { credentialStorage: 'private-file' });
  await start(client);
  const value = await client.readAccount({ refreshToken: false });
  assert.deepEqual(value.argv, ['-c', 'cli_auth_credentials_store="file"', 'app-server', '--listen', 'stdio://']);
});

test('startup notifications and token requests cannot release private owner state before verified readiness', async t => {
  let calls = 0;
  const notices = [];
  const { client, codexHome } = await ownedExperimental(t, 'early-refresh', {
    onNotification(value) { notices.push(value); },
    onAuthTokensRefresh() { calls++; return { accessToken: 'PUBLIC-never', chatgptAccountId: 'PUBLIC-never' }; },
  });
  await start(client); await client.readAccount({ refreshToken: false });
  assert.equal(calls, 0);
  assert.equal(notices.filter(value => value.params.PUBLICStartup).length, 0);
  const response = (await requests(codexHome)).find(value => value.id === 'PUBLIC-early');
  assert.deepEqual(response, { id: 'PUBLIC-early', error: { code: -32000, message: 'Request unavailable' } });
});

test('callback ignoring abort keeps its capacity reservation until settlement, preventing unbounded abandoned work', async t => {
  let calls = 0;
  const { client } = await ownedExperimental(t, 'refresh-uncooperative', { maxServerRequests: 1, serverRequestTimeoutMs: 20,
    onAuthTokensRefresh() { calls++; return new Promise(() => {}); },
  });
  await start(client); await client.readAccount({ refreshToken: false });
  assert.equal(client.lifecycle.serverRequests, 1);
  assert.ok(client.lifecycle.pendingBytes > 0);
  await rejectsCode(client.readAccount({ refreshToken: false }), 'SERVER_REQUEST_LIMIT');
  assert.equal(calls, 1);
  assert.equal((await client.stop()).closeObserved, true);
});

test('blocked stdio writes have a bounded fault deadline and never replay a partial mutation', async t => {
  let unread;
  const ready = new Promise(resolve => { unread = resolve; });
  const { client } = await owned(t, 'unread', {
    maxFrameBytes: 1048576, maxPendingBytes: 8388608, writeTimeoutMs: 40,
    onNotification() { unread(); },
  });
  await start(client); await ready;
  const requests = Array.from({ length: 3 }, () => client.startLogin({ type: 'apiKey', apiKey: 'PUBLIC'.repeat(100000) }));
  await Promise.all(requests.map(request => rejectsCode(request, 'WRITE_TIMEOUT', true)));
  assert.equal((await client.stop()).closeObserved, true);
});

test('frame preflight accounts for escaped and multibyte bytes before whole-record serialization', async t => {
  const { client } = await owned(t, 'normal', { maxFrameBytes: 1024, maxPendingBytes: 8192 });
  await start(client);
  for (const value of ['PUBLIC'.repeat(1000000), '🌹'.repeat(300), '\\"'.repeat(600)]) {
    await rejectsCode(client.startLogin({ type: 'apiKey', apiKey: value }), 'FRAME_LIMIT', false);
  }
  assert.equal(client.lifecycle.state, 'ready');
});

test('startup cancellation is prompt, fail-closed and independent of server acknowledgment', async t => {
  const { client } = await owned(t, 'no-init');
  const controller = new AbortController();
  const starting = client.start({ explicitUserAction: true, signal: controller.signal });
  const assertion = rejectsCode(starting, 'ABORTED', false);
  controller.abort();
  await assertion;
  assert.equal(client.lifecycle.state, 'stopped');
});

test('experimental auth requires an exact boolean and callback deadline strictly below server 10s', async t => {
  const { options } = await owned(t, 'normal');
  for (const value of [undefined, null, 0, 1, 'false', 'true', {}, []]) {
    assert.throws(() => createCodexAppServer({ ...options, enableExperimentalChatgptTokens: value }), { code: 'INVALID_OPTIONS' });
  }
  for (const timeout of [10000, 10001, 2147483647]) {
    assert.throws(() => createCodexAppServer({ ...options, enableExperimentalChatgptTokens: true, serverRequestTimeoutMs: timeout }), { code: 'INVALID_OPTIONS' });
  }
  const enabled = createCodexAppServer({ ...options, enableExperimentalChatgptTokens: true, serverRequestTimeoutMs: 9999 });
  const disabled = createCodexAppServer({ ...options, enableExperimentalChatgptTokens: false, serverRequestTimeoutMs: 10000 });
  assert.equal(enabled.lifecycle.state, 'idle'); assert.equal(disabled.lifecycle.state, 'idle');
  await enabled.stop(); await disabled.stop();
});

test('explicit false freezes nonexperimental admission and denies token login/refresh without owner access', async t => {
  let calls = 0, response;
  const { client, options, codexHome } = await owned(t, 'refresh-string', {
    enableExperimentalChatgptTokens: false,
    onAuthTokensRefresh() { calls++; return { accessToken: 'PUBLIC-never', chatgptAccountId: 'PUBLIC-never' }; },
    onNotification(notice) { response = notice.params.PUBLICReply; },
  });
  const tokens = { type: 'chatgptAuthTokens', accessToken: 'PUBLIC-never', chatgptAccountId: 'PUBLIC-never' };
  await rejectsCode(client.startLogin(tokens), 'EXPERIMENTAL_AUTH_DISABLED', false);
  options.enableExperimentalChatgptTokens = true;
  await start(client);
  await rejectsCode(client.startLogin(tokens), 'EXPERIMENTAL_AUTH_DISABLED', false);
  await rejectsCode(client.startLogin({ ...tokens, enableExperimentalChatgptTokens: true }), 'EXPERIMENTAL_AUTH_DISABLED', false);
  await client.readAccount({ refreshToken: false });
  assert.equal(calls, 0);
  assert.deepEqual(response, { id: 'PUBLIC-refresh-1', error: { code: -32601, message: 'Request unavailable' } });
  const trace = await requests(codexHome);
  assert.equal(trace[0].params.capabilities.experimentalApi, false);
  assert.equal(trace.filter(request => request.method === 'account/login/start').length, 0);
  assert.ok(!JSON.stringify(trace).includes('PUBLIC-never'));
});

test('explicit Beta true negotiates experimental capability and sends exact private external-token fields', async t => {
  const { client, codexHome } = await ownedExperimental(t, 'normal');
  await start(client);
  const tokens = { type: 'chatgptAuthTokens', accessToken: 'PUBLIC-synthetic-access', chatgptAccountId: 'PUBLIC-account', chatgptPlanType: 'PUBLIC-plan' };
  const result = await client.startLogin(tokens);
  assert.deepEqual(result.params, tokens);
  assert.equal(result.initialized, true);
  for (const plan of [null, '']) await client.startLogin({ ...tokens, chatgptPlanType: plan });
  const { chatgptPlanType: _plan, ...withoutPlan } = tokens;
  await client.startLogin(withoutPlan);
  const trace = await requests(codexHome);
  assert.deepEqual(trace[0].params.capabilities, { experimentalApi: true, explicitGatewayOauth: true, requestAttestation: false });
  assert.deepEqual(trace[2], { id: 'native-2', method: 'account/login/start', params: tokens });
  assert.deepEqual(Object.keys(trace[2].params), ['type', 'accessToken', 'chatgptAccountId', 'chatgptPlanType']);
  assert.ok(!Object.hasOwn(trace.at(-1).params, 'chatgptPlanType'));
  assert.deepEqual(Object.keys(client), ['start', 'stop', 'lifecycle', 'readAccount', 'readRateLimits', 'startLogin', 'cancelLogin', 'logout']);
});

test('Beta token field validation cannot open arbitrary experimental methods or accept credential extras', async t => {
  const { client, codexHome } = await ownedExperimental(t, 'normal');
  await start(client);
  const tokens = { type: 'chatgptAuthTokens', accessToken: 'PUBLIC-synthetic-access', chatgptAccountId: 'PUBLIC-account' };
  for (const patch of [{ accessToken: undefined }, { accessToken: null }, { accessToken: '' }, { accessToken: 7 },
    { accessToken: 'PUBLIC\ninvalid' }, { chatgptAccountId: undefined }, { chatgptAccountId: null }, { chatgptAccountId: '' },
    { chatgptPlanType: undefined }, { chatgptPlanType: 7 }, { chatgptPlanType: {} }, { chatgptPlanType: 'PUBLIC\ninvalid' },
    { refreshToken: 'PUBLIC-extra' }, { idToken: 'PUBLIC-extra' }, { credentialStorage: 'file' }, { experimentalApi: true }]) {
    await rejectsCode(client.startLogin({ ...tokens, ...patch }), 'INVALID_PARAMS', false);
  }
  for (const type of ['process/spawn', 'attestation/generate', 'experimentalFeature/enablement/set', 'amazonBedrock']) {
    await rejectsCode(client.startLogin({ type }), 'UNSUPPORTED_LOGIN_TYPE', false);
  }
  await rejectsCode(client.readAccount({ refreshToken: false, workspaceRouting: true }), 'INVALID_PARAMS');
  await rejectsCode(client.startLogin(tokens, { experimentalApi: true }), 'INVALID_PARAMS');
  assert.equal((await settledRequests(codexHome, 2)).length, 2);
  assert.equal(client.request, undefined); assert.equal(client.dispatch, undefined);
});

test('Beta token RPC enforces complete UTF8/escaped frame bounds before any transmission', async t => {
  const { client, codexHome } = await ownedExperimental(t, 'normal', { maxFrameBytes: 1024, maxPendingBytes: 8192 });
  await start(client);
  const tokens = { type: 'chatgptAuthTokens', accessToken: 'PUBLIC-access', chatgptAccountId: 'PUBLIC-account' };
  for (const patch of [{ accessToken: 'PUBLIC'.repeat(1000000) }, { accessToken: '🌹'.repeat(300) },
    { accessToken: '\\"'.repeat(600) }, { chatgptAccountId: '🌹'.repeat(300) },
    { accessToken: 'PUBLIC'.repeat(100), chatgptAccountId: 'PUBLIC'.repeat(100) }, { chatgptPlanType: 'PUBLIC'.repeat(200) }]) {
    await rejectsCode(client.startLogin({ ...tokens, ...patch }), 'FRAME_LIMIT', false);
  }
  assert.equal((await settledRequests(codexHome, 2)).length, 2);
  assert.equal(client.lifecycle.state, 'ready');
});

test('Beta token mutation timeout stays private and indeterminate, with no automatic retry', async t => {
  const { client, codexHome } = await ownedExperimental(t, 'hang', { requestTimeoutMs: 30 });
  await start(client);
  await rejectsCode(client.startLogin({ type: 'chatgptAuthTokens', accessToken: 'PUBLIC-synthetic-access', chatgptAccountId: 'PUBLIC-account' }), 'REQUEST_TIMEOUT', true);
  assert.equal((await requests(codexHome)).filter(request => request.method === 'account/login/start').length, 1);
  assert.equal(client.lifecycle.pendingRequests, 0);
});

test.after(async () => {
  await writeFile(join(evidence, 'PUBLIC-summary.json'), JSON.stringify({ evidence, fixtureRoots: traces,
    limits: { maxFrameBytes: 8192, maxPendingRequests: 8, maxPendingBytes: 65536, maxServerRequests: 4,
      maxServerRequestIds: 16, maxRequestIdBytes: 128, startupTimeoutMs: 2000, requestTimeoutMs: 2000,
      writeTimeoutMs: 1000, serverRequestTimeoutMs: 1000, stopGraceMs: 100, stopTermMs: 100, stopKillMs: 1000 },
    defaultExperimentalChatgptTokens: false, experimentalCasesExplicitlyEnabled: true,
    realCodexLaunched: false, realProviderCalls: false, cargoRun: false }, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
  console.log(`PUBLIC Codex fixture evidence: ${evidence}`);
});
