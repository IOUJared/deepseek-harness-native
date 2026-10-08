// Local fake public BFF only: no Harness import, composition, model, or account mutation.
import http from 'node:http';
import crypto from 'node:crypto';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import { createRequire } from 'node:module';
const require = createRequire(path.resolve(import.meta.dirname, '../../../deepseek-harness-linux/packages/api/gateway/package.json'));
const { WebSocketServer } = require('ws');
const mode = process.argv[2] ?? 'normal';
let origin, cookieName;
let nextSelfReplyEvent = () => {};
let selfReplyEventNumber = 0;
let pluginRevision = 3, pluginCount = 4, pluginMutations = 0, pluginDescriptions = 0;
const claims = new Map();
let claimOpens = 0, claimCancels = 0, claimReplyRequests = 0, claimRejects = 0, claimDisconnects = 0, claimLateFramesSent = 0;
const claimMethods = Object.create(null);
function endClaims() {
  for (const [id, ws] of claims) if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'end', streamId: id }));
  claims.clear();
}
const pluginNamespace = () => ({ ns: 'probe', revision: pluginRevision, autoGenerate: true, applies: 'live', schema: { uid: 1, refs: { 1: { type: 'object', meta: {}, dict: { count: 2, apiKey: 3, provider: 4 } }, 2: { type: 'number', meta: { min: 1, max: 8, step: 1 } }, 3: { type: 'string', meta: { role: 'secret' } }, 4: { type: 'string', meta: { role: 'credential-ref' } } } }, value: { count: pluginCount, apiKey: 'PRIVATE_PLUGIN_SENTINEL', provider: 'PRIVATE_REFERENCE_SENTINEL' }, base: {}, user: { count: pluginCount }, secrets: [{ path: ['apiKey'], set: true }] });
const cookieValue = 'v1.fixture.signature';
const server = http.createServer(async (request, response) => {
  if (request.method === 'GET' && request.url === '/?token=fixture-only') {
    if (mode === 'redirect-start') { response.writeHead(302, { location: 'http://127.0.0.1:1/leak' }); response.end(); return; }
    const name = mode === 'bad-name' ? 'dsh-auth-wrong-authority' : cookieName;
    const attributes = [mode === 'no-http-only' ? '' : 'HttpOnly', mode === 'bad-scope' ? 'Path=/other' : 'Path=/', mode === 'no-samesite' ? 'SameSite=Lax' : 'SameSite=Strict', 'Max-Age=3600', `Expires=${new Date(Date.now() + 3_600_000).toUTCString()}`];
    if (mode === 'domain-cookie') attributes.push('Domain=127.0.0.1');
    if (mode === 'secure-cookie') attributes.push('Secure');
    response.writeHead(303, { location: mode === 'bad-location' ? '/foreign' : './', 'set-cookie': `${name}=${cookieValue}; ${attributes.join('; ')}` }); response.end(); return;
  }
  if (request.headers.host !== origin.slice(7) || request.headers.origin !== origin || request.headers.cookie !== `${cookieName}=${cookieValue}`) {
    response.writeHead(403); response.end(); return;
  }
  if (mode === 'redirect-rpc') { response.writeHead(307, { location: 'http://127.0.0.1:1/leak' }); response.end(); return; }
  const chunks = []; for await (const chunk of request) chunks.push(chunk);
  let envelope; try { envelope = JSON.parse(Buffer.concat(chunks).toString()); } catch { response.writeHead(400); response.end(); return; }
  if (envelope.type !== 'client-request' || request.url !== `/api/${envelope.method}` || !envelope.payload?.args) { response.writeHead(400); response.end(); return; }
  const args = envelope.payload.args;
  if (mode.startsWith('claim-')) claimMethods[envelope.method] = (claimMethods[envelope.method] ?? 0) + 1;
  let value;
  switch (envelope.method) {
    case 'session/list':
      if (JSON.stringify(args) !== '{"_request":{}}') { response.writeHead(400); response.end(); return; }
      value = { items: [{ agentAvailable: false, sessionId: 'session / opaque:%', updatedAt: 1000, running: false, blank: true }] };
      if (mode.startsWith('claim-')) value.items[0].cwd = JSON.stringify({ opens: claimOpens, cancels: claimCancels, active: claims.size, replyRequests: claimReplyRequests, rejects: claimRejects, disconnects: claimDisconnects, methods: claimMethods, lateFramesSent: claimLateFramesSent });
      break;
    case 'session/create': value = { sessionId: args.request.sessionId ?? 'session-new', ...(args.request.agentPreset ? { agentPreset: args.request.agentPreset } : {}) }; break;
    case 'session/prompt': case 'session/cancel':
      if (mode.startsWith('claim-') && envelope.method === 'session/cancel') endClaims(); // Fixture-only explicit end barrier, not claim behavior.
      value = { accepted: true }; break;
    case 'session/modelCatalog': value = { default: { provider: 'example', model: 'model' }, routableProviders: [], groups: [], failures: [] }; break;
    case 'session/selectModel': value = { selected: { provider: args.request.provider, model: args.request.model } }; break;
    case 'session/page': value = { records: [], hasMore: false }; break;
    case 'session/projections': value = null; break;
    case 'workspace/pinSession': case 'workspace/unpinSession':
    case 'workspace/archiveSession': case 'workspace/unarchiveSession': {
      const expected = { request: { sessionId: 'session / registry opaque:%' } };
      // Full envelope equality prevents extra stopActivity, agent or signal authority.
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: envelope.method, payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      const pin = envelope.method.endsWith('pinSession');
      const field = pin ? 'pinnedSessionIds' : 'archivedSessionIds';
      const ids = envelope.method === 'workspace/pinSession' || envelope.method === 'workspace/archiveSession'
        ? ['session / registry opaque:%', 'session-z-global', 'session-a-global']
        : ['session-z-global', 'session-a-global'];
      value = { [field]: ids };
      if (mode === 'registry-array') value = [ids];
      if (mode === 'registry-missing') value = {};
      if (mode === 'registry-empty-id') value = { [field]: [''] };
      if (mode === 'registry-wrong-list') value = { [field]: 'not-a-list' };
      if (mode === 'registry-extra') value.agent = {};
      break;
    }
    case 'pluginInventory/list':
      if (!isDeepStrictEqual(args, {})) { response.writeHead(400); response.end(); return; }
      value = { entries: [{ entryId: 'include:probe', moduleName: ['plugin-missing', 'plugin-changed-value', 'plugin-custom-page'].includes(mode) ? `probe-mutations-${pluginMutations}` : 'probe', enabled: true, fiberPhase: 'active', meta: { error: 'PRIVATE_PLUGIN_SENTINEL' } }] }; break;
    case 'settings/describe':
      if (!isDeepStrictEqual(args, {})) { response.writeHead(400); response.end(); return; }
      pluginDescriptions++;
      value = { writable: mode !== 'plugin-readonly', hasDocument: true, namespaces: [pluginNamespace()] };
      if (mode === 'plugin-missing') delete value.namespaces[0].value.count;
      if (mode === 'plugin-custom-page') value.namespaces[0].autoGenerate = false;
      if (mode === 'plugin-changed-value' && pluginDescriptions > 1) value.namespaces[0].value.count = 6;
      break;
    case 'settings/mutate':
      pluginMutations++;
      if (!isDeepStrictEqual(args, { ns: 'probe', ops: [{ op: 'set', path: ['count'], value: 5 }], expectedRevision: 3 })) { response.writeHead(400); response.end(); return; }
      if (!['plugin-conflict', 'plugin-rejected'].includes(mode)) { pluginCount = 5; pluginRevision = 4; }
      value = pluginNamespace(); break;
    case 'account/getState': value = { status: 'signed-out', links: { usageUrl: 'https://example.invalid/usage', topUpUrl: 'https://example.invalid/topup' }, attempt: null }; break;
    case 'userQuestions/answer':
      if (args.agentId !== 'agent-fixture' || args.callId !== 'call-1' || 'request' in args) { response.writeHead(400); response.end(); return; }
      value = true; break;
    case '$events/result':
      if (mode.startsWith('claim-')) {
        if (args.clientId !== 'fixture-client' || args.eventId !== 'fixture-question') { response.writeHead(400); response.end(); return; }
        claimReplyRequests++;
        if (args.outcome?.kind === 'rejected') claimRejects++;
        if (mode === 'claim-reply-failure') { response.writeHead(503); response.end(); return; }
        if (mode === 'claim-end-first') endClaims();
      }
      if (mode === 'reply-failure') { response.writeHead(503); response.end(); return; }
      if (mode === 'question-events' && !isDeepStrictEqual(args, {clientId:'fixture-client',eventId:'fixture-question',outcome:{kind:'rejected',error:{name:'UserQuestionError',message:'the user cancelled ask_user_question',code:'ASK_CANCELLED'}}})) { response.writeHead(400); response.end(); return; }
      value = undefined; break;
    default: response.writeHead(404); response.end(); return;
  }
  const output = { type: 'server-response', rpcId: mode === 'wrong-rpc' ? 'wrong' : envelope.rpcId, result: { ok: true, ...(value === undefined ? {} : { value }) } };
  if (mode === 'missing-available') delete output.result.value.items[0].agentAvailable;
  if (envelope.method === 'settings/mutate' && ['plugin-conflict', 'plugin-rejected', 'plugin-postcommit'].includes(mode)) output.result = { ok: false, error: { code: mode === 'plugin-conflict' ? 'settings/conflict' : mode === 'plugin-rejected' ? 'settings/rejected' : 'gateway/internal', message: 'PRIVATE_PLUGIN_SENTINEL', details: {} } };
  if (mode === 'failure') output.result = { ok: false, error: { code: 'fixture/rejected', message: 'SECRET_SENTINEL_opaque_url_cookie', details: {} } };
  if (mode === 'oversize-length') { response.writeHead(200, { 'content-type': 'application/json', 'content-length': '2000000' }); response.end('x'); return; }
  response.writeHead(200, { 'content-type': mode === 'bad-media' ? 'text/html' : 'application/json' });
  if (mode === 'slow') { setTimeout(() => response.end(JSON.stringify(output)), 2000); return; }
  if (mode === 'bad-json') { response.end('{bad SECRET_SENTINEL_opaque_url_cookie'); return; }
  if (mode === 'oversize-chunked') { response.write('['); response.end(' '.repeat(8192) + ']'); return; }
  if (mode === 'claim-end-first' && envelope.method === '$events/result') {
    setTimeout(() => response.end(JSON.stringify(output)), 100); return;
  }
  response.end(JSON.stringify(output));
  if (mode === 'self-reply-series' && envelope.method === '$events/result') {
    // Like real Gateway: remove our delivery, acknowledge RPC, never self-Cancel.
    setTimeout(nextSelfReplyEvent, 5);
  }
});
const wss = new WebSocketServer({ noServer: true, maxPayload: 256 * 1024 });
server.on('upgrade', (request, socket, head) => {
  if (request.url !== '/api/remote.mux' || request.headers.host !== origin.slice(7) || request.headers.origin !== origin || request.headers.cookie !== `${cookieName}=${cookieValue}`) { socket.destroy(); return; }
  if (mode === 'slow-upgrade') {
    setTimeout(() => { if (!socket.destroyed) wss.handleUpgrade(request, socket, head, ws => wss.emit('connection', ws)); }, 2000);
    return;
  }
  wss.handleUpgrade(request, socket, head, ws => wss.emit('connection', ws));
});
wss.on('connection', ws => {
  ws.on('close', () => { for (const [id, owner] of claims) if (owner === ws) { claims.delete(id); claimDisconnects++; } });
  let pinged = false;
  ws.on('pong', () => { pinged = true; });
  ws.on('message', bytes => {
    const message = JSON.parse(bytes.toString());
    if (message.type === 'cancel') {
      if (mode.startsWith('claim-') && claims.delete(message.streamId)) claimCancels++;
      if (mode === 'late-item') ws.send(JSON.stringify({ type: 'item', streamId: message.streamId, value: { type: 'event', event: { seq: 1, type: 'turn/end', time: 1000, data: {} } } }));
      return;
    }
    if (message.type !== 'open') return;
    const id = message.streamId;
    const send = value => ws.send(JSON.stringify({ type: 'item', streamId: id, value }));
    if (mode === 'bad-frame') { ws.send('{broken SECRET_SENTINEL_opaque_url_cookie'); return; }
    if (mode === 'binary') { ws.send(Buffer.from('binary')); return; }
    if (mode === 'wrong-stream') { ws.send(JSON.stringify({ type: 'item', streamId: 'unknown', value: {} })); return; }
    if (mode === 'ws-oversize') { send({ value: 'x'.repeat(8192) }); return; }
    if (mode === 'remote-stream-error') { ws.send(JSON.stringify({ type: 'error', streamId: id, error: { code: 'fixture/fail', message: 'SECRET_SENTINEL_opaque_url_cookie', details: {} } })); return; }
    switch (message.endpoint) {
      case 'workspace/follow':
        send({ type: 'baseline', value: { items: [], archivedSessionIds: [], pinnedSessionIds: [] } });
        if (mode === 'queued-budget') send({ type: 'upsert', workspace: { workspaceId: 'workspace-fixture', path: '/fixture', title: 'x'.repeat(480), sessionIds: [], createdAt: '2026-10-06T00:00:00.000Z', updatedAt: '2026-10-06T00:00:00.000Z' } });
        break;
      case 'session/control': send({ type: 'baseline', value: { projections: {} } }); break;
      case 'session/follow':
        if ('cursor' in message.payload.args.request) { ws.close(1008); return; }
        send({ type: 'snapshot', header: { version: 4, id: 'session / opaque:%', createdAt: 1000, isSeeded: false }, cursor: -1, records: [], hasMore: false, projections: { asOfSeq: -1, values: {} } });
        send({ type: 'event', event: { seq: mode === 'sequence-gap' ? 2 : 0, type: 'turn/start', time: 1000, data: { turn: 1 } } }); break;
      case '$events':
        send({ type: 'ready', clientId: 'fixture-client', host: { home: '/fixture' } });
        if (mode.startsWith('claim-')) send({ type: 'waterfall', event: 'user-questions/request', eventId: 'fixture-question', agentId: 'agent-fixture', request: { questions: [{ id: 'q', question: 'PUBLIC fixture question', options: [{ label: 'yes' }] }], wait: { callId: 'call-1', timed: true } } });
        else if (mode === 'question-events') send({ type: 'waterfall', event: 'user-questions/request', eventId: 'fixture-question', agentId: 'agent-fixture', request: { questions: [] } });
        else if (mode === 'self-reply-series') {
          nextSelfReplyEvent = () => { if (ws.readyState === 1) send({ type: 'waterfall', event: 'approval/request', eventId: `self-${++selfReplyEventNumber}`, agentId: 'agent-fixture', request: { toolName: 'fixture' } }); };
          nextSelfReplyEvent();
        } else {
          const request = mode === 'event-array' ? ['fixture', null, null, null] : mode === 'event-agent' ? {toolName:'fixture',agent:{}} : mode === 'event-signal' ? {toolName:'fixture',signal:{aborted:true}} : {toolName:'fixture'};
          send({ type: 'waterfall', event: 'approval/request', eventId: 'fixture-event', agentId: 'agent-fixture', request });
        }
        if (mode === 'event-flood') {
          for (let i = 0; i < 5; i++) send({ type: 'waterfall', event: 'approval/request', eventId: `overflow-${i}`, agentId: 'agent-fixture', request: { toolName: 'fixture' } });
        }
        break;
      case 'userQuestions/attachWait': {
        if (!mode.startsWith('claim-')) { send({ remainingMs: 1234 }); break; }
        if (!isDeepStrictEqual(message, { type: 'open', streamId: id, endpoint: 'userQuestions/attachWait', payload: { args: { agentId: 'agent-fixture', callId: 'call-1' } } })) {
          ws.send(JSON.stringify({ type: 'error', streamId: id, error: { code: 'fixture/authority', message: 'PRIVATE_AUTHORITY_SENTINEL', details: {} } })); break;
        }
        claimOpens++;
        if (mode === 'claim-end-before') { ws.send(JSON.stringify({ type: 'end', streamId: id })); break; }
        if (mode === 'claim-error-before') { ws.send(JSON.stringify({ type: 'error', streamId: id, error: { code: 'fixture/unavailable', message: 'PRIVATE_ERROR_SENTINEL', details: {} } })); break; }
        claims.set(id, ws);
        const bad = { 'claim-array': [1234], 'claim-empty': {}, 'claim-extra': { remainingMs: 1234, deadline: 1 }, 'claim-negative': { remainingMs: -1 }, 'claim-fraction': { remainingMs: 0.5 }, 'claim-string': { remainingMs: '1234' }, 'claim-null': { remainingMs: null }, 'claim-overflow': { remainingMs: 1e30 } };
        if (mode in bad) { send(bad[mode]); break; }
        if (mode === 'claim-slow-first') { setTimeout(() => { if (ws.readyState === 1) { send({ remainingMs: 1234 }); claimLateFramesSent++; } }, 200); break; }
        send({ remainingMs: mode === 'claim-zero' ? 0 : 1234 });
        if (mode === 'claim-duplicate') send({ remainingMs: 1233 });
        if (mode === 'claim-immediate-end') endClaims();
        if (mode === 'claim-error-after') { claims.delete(id); ws.send(JSON.stringify({ type: 'error', streamId: id, error: { code: 'fixture/ended', message: 'PRIVATE_ERROR_SENTINEL', details: {} } })); }
        break;
      }
      default: ws.close(1008); return;
    }
    if (mode === 'ping') {
      ws.ping('fixture');
      setTimeout(() => { if (pinged && ws.readyState === 1) { send({ type: 'upsert', workspace: { workspaceId: 'workspace-fixture', path: '/fixture', title: 'Pong observed', sessionIds: [], createdAt: '2026-10-06T00:00:00.000Z', updatedAt: '2026-10-06T00:00:00.000Z' } }); } }, 50);
    }
  });
});
server.listen(0, '127.0.0.1', () => {
  origin = `http://127.0.0.1:${server.address().port}`;
  cookieName = `dsh-auth-${crypto.createHash('sha256').update(origin.slice(7)).digest('base64url')}`;
  process.stdout.write(`${origin}/?token=fixture-only\n`); // private pipe consumed by test, never test log
});
process.on('SIGTERM', () => { for (const socket of wss.clients) socket.terminate(); server.close(() => process.exit(0)); });
