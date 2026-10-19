// Local fake public BFF only: no Harness import, composition, model, or account mutation.
import http from 'node:http';
import crypto from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { WebSocketServer } = require('ws');
const mode = process.argv[2] ?? 'normal';
let origin, cookieName;
let nextSelfReplyEvent = () => {};
let selfReplyEventNumber = 0;
let pluginRevision = 3, pluginCount = 4, pluginMutations = 0, pluginDescriptions = 0;
let agentPresetDefault = null, agentPresetWrites = 0;
const claims = new Map();
let claimOpens = 0, claimCancels = 0, claimReplyRequests = 0, claimRejects = 0, claimDisconnects = 0, claimLateFramesSent = 0;
const claimMethods = Object.create(null);
function endClaims() {
  for (const [id, ws] of claims) if (ws.readyState === 1) ws.send(JSON.stringify({ type: 'end', streamId: id }));
  claims.clear();
}
const pluginNamespace = () => ({ ns: 'probe', revision: pluginRevision, autoGenerate: true, applies: 'live', schema: { uid: 1, refs: { 1: { type: 'object', meta: {}, dict: { count: 2, apiKey: 3, provider: 4 } }, 2: { type: 'number', meta: { min: 1, max: 8, step: 1 } }, 3: { type: 'string', meta: { role: 'secret' } }, 4: { type: 'string', meta: { role: 'credential-ref' } } } }, value: { count: pluginCount, apiKey: 'PRIVATE_PLUGIN_SENTINEL', provider: 'PRIVATE_REFERENCE_SENTINEL' }, base: {}, user: { count: pluginCount }, secrets: [{ path: ['apiKey'], set: true }] });
const agentPresetNamespace = () => ({ ns: 'agent-preset-registry', revision: 0, autoGenerate: false, applies: 'live',
  schema: { uid: 4115, refs: { 4114: { type: 'string', meta: {} }, 4115: { type: 'object', meta: { default: {} }, dict: { selectedDefault: 4114 } } } },
  value: agentPresetDefault === null ? {} : { selectedDefault: agentPresetDefault }, base: {},
  user: agentPresetDefault === null ? {} : { selectedDefault: agentPresetDefault }, secrets: [] });
const cookieValue = 'v1.fixture.signature';
const terminalInfo = () => ({ id: 'PUBLIC-terminal-1', title: 'PUBLIC terminal', shell: { path: '/bin/PUBLIC-shell', args: ['-l'], name: 'PUBLIC shell' }, cwd: '/PUBLIC/cwd', cols: 80, rows: 24, state: 'running', exitCode: null });
const terminalAgent = 'session / terminal opaque:%';
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
    case 'skills/list': {
      // The reference's remote takes one `SkillListRequest` object, `{ sessionId }`.
      const expected = { sessionId: 'session / opaque:%' };
      if (request.method !== 'POST'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'skills/list', payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      value = { skills: [
        { name: 'impeccable-rust', description: 'Risk-driven Rust review', modelInvocable: true },
        { name: 'code-review', description: 'Review changes', whenToUse: 'before a commit' },
      ] };
      if (mode === 'skills-blank-name') value.skills[0].name = '';
      break;
    }
    case 'commands/list': {
      // The Host takes one flat named argument (`agentId`), never a wrapped request.
      const expected = { agentId: 'session / opaque:%' };
      if (request.method !== 'POST'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'commands/list', payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      value = [
        { name: 'plan', description: 'Explore and design before executing' },
        { definitionId: 'PUBLIC-def', name: 'compact', description: 'Condense history' },
      ];
      if (mode === 'commands-slash-name') value[0].name = '/plan';
      if (mode === 'commands-blank-name') value[0].name = '';
      // The real descriptor advertises an optional free-form input hint and may
      // grow further fields; neither may cost the composer its menu.
      if (mode === 'commands-extra-fields') {
        value[0].input = { hint: 'what to plan' };
        value[1].futureField = { anything: true };
      }
      break;
    }
    case 'commands/execute': {
      const expected = { agentId: 'session / opaque:%', line: '/plan', submittedAttachments: [] };
      if (request.method !== 'POST'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'commands/execute', payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      value = { commandId: 'PUBLIC-command', result: { kind: 'success', text: 'Plan mode on' } };
      if (mode === 'command-error') value = { commandId: 'PUBLIC-command', result: { kind: 'error', text: 'unknown command' } };
      if (mode === 'command-blank-error') value = { commandId: 'PUBLIC-command', result: { kind: 'error', text: '  ' } };
      if (mode === 'command-null') value = undefined;
      break;
    }
    case 'session/modelCatalog': value = { default: { provider: 'example', model: 'model' }, routableProviders: [], groups: [], failures: [] }; break;
    case 'session/selectModel': value = { selected: { provider: args.request.provider, model: args.request.model } }; break;
    case 'session/rename': {
      const expected = { request: { sessionId: 'session / rename opaque:%', title: 'PUBLIC renamed title' } };
      // Full envelope equality rejects extra agent/stopActivity authority and any title drift.
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'session/rename', payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      value = { title: args.request.title, seq: 5 };
      if (mode === 'rename-missing') value = {};
      if (mode === 'rename-wrong-type') value = { title: 5, seq: 'five' };
      if (mode === 'rename-extra') value.agentId = 'agent-fixture';
      if (mode === 'rename-negative-seq') value.seq = -1;
      break;
    }
    case 'session/fork': {
      // Omission of atSeq is itself part of the contract: the envelope must carry
      // exactly {sessionId} unless the caller asked for an explicit inclusive cut.
      const expected = mode === 'fork-at'
        ? { request: { sessionId: 'session / fork opaque:%', atSeq: 4 } }
        : { request: { sessionId: 'session / fork opaque:%' } };
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'session/fork', payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      value = { sessionId: 'session-fork-child' };
      if (mode === 'fork-missing') value = {};
      if (mode === 'fork-empty') value = { sessionId: '' };
      if (mode === 'fork-wrong-type') value = { sessionId: 5 };
      if (mode === 'fork-same') value = { sessionId: 'session / fork opaque:%' };
      if (mode === 'fork-extra') value.agentId = 'agent-fixture';
      break;
    }
    case 'session/page': value = { records: [], hasMore: false }; break;
    case 'session/projections': value = null; break;
    case 'directoryPicker/list': {
      // `path` is optional, but the Host's boundary rejects a present null. Root
      // requests must carry exactly `{}`; child requests exactly one string path.
      const root = isDeepStrictEqual(args, {});
      const child = isDeepStrictEqual(args, { path: '/PUBLIC/root' });
      if (!root && !child) { response.writeHead(400); response.end(); return; }
      value = {
        path: '/PUBLIC/root', home: '/PUBLIC/home',
        crumbs: [{ name: '/', path: '/', hidden: false }],
        entries: [{ name: 'project', path: '/PUBLIC/root/project', hidden: false }],
        truncated: false,
      };
      break;
    }
    case 'directoryPicker/pick': {
      // The chooser takes no arguments and replies with one absolute path or null
      // on cancel; every other shape is a malformed reply, not a cancel.
      if (!isDeepStrictEqual(args, {})) { response.writeHead(400); response.end(); return; }
      value = '/PUBLIC/picked';
      if (mode === 'picker-cancel') value = null;
      if (mode === 'picker-missing') value = {};
      if (mode === 'picker-empty') value = '';
      if (mode === 'picker-number') value = 7;
      if (mode === 'picker-oversize') value = `/${'x'.repeat(4096)}`;
      break;
    }
    case 'messageFeedback/put': {
      // Absent note/category are omitted keys, never explicit nulls: the Host's
      // boundary schema refuses null for either optional string.
      const bare = { sessionId: 'session / feedback opaque:%', messageId: 'PUBLIC-message', rating: 'positive', ifVersion: null };
      const noted = { ...bare, messageId: 'PUBLIC-message-note', note: 'PUBLIC note', category: 'task-result' };
      const expected = args.request?.messageId === 'PUBLIC-message-note' ? noted : bare;
      if (!isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'messageFeedback/put', payload: { args: { request: expected } } })) {
        response.writeHead(400); response.end(); return;
      }
      value = {
        ok: true,
        value: {
          messageId: expected.messageId, rating: 'positive', version: 'v-fixture',
          createdAt: 1, updatedAt: 2,
          ...(expected.note === undefined ? {} : { note: expected.note }),
          ...(expected.category === undefined ? {} : { category: expected.category }),
        },
      };
      break;
    }
    case 'schedule/update': {
      // The native body wraps the whole mutation in `request`; the fixture only checks the
      // identity and that a mutation field is present, then answers the mode's outcome.
      const request = args.request;
      if (!request || request.id !== 'PUBLIC-schedule' || (!request.title && !request.prompt && !request.change)) { response.writeHead(400); response.end(); return; }
      if (mode === 'schedule-update-ended') value = { id: 'PUBLIC-schedule', updated: false, code: 'schedule_ended' };
      else if (mode === 'schedule-update-conflict') value = { id: 'PUBLIC-schedule', updated: false, code: 'schedule_conflict' };
      else if (mode === 'schedule-update-refused') value = { id: 'PUBLIC-schedule', code: 'schedule_refused', message: 'PRIVATE_SCHEDULE_SENTINEL' };
      else value = { id: 'PUBLIC-schedule', updated: true, record: { id: 'PUBLIC-schedule', title: 'PUBLIC title', prompt: 'PUBLIC prompt' } };
      break;
    }
    case 'schedule/history': {
      // The first page omits `before` entirely; later pages carry the opaque
      // cursor string. A null cursor fails the Host boundary and this check.
      const base = { sessionId: 'session / schedule opaque:%', id: 'PUBLIC-schedule', limit: 20 };
      const first = isDeepStrictEqual(args, { request: base });
      const next = isDeepStrictEqual(args, { request: { ...base, before: 'PUBLIC-before' } });
      if (!first && !next) { response.writeHead(400); response.end(); return; }
      value = {
        id: 'PUBLIC-schedule', records: [],
        earlierRecordsUnavailable: false, earlierRecordsPruned: false,
        retention: { days: 7, records: 50 },
      };
      break;
    }
    case 'agentPresets/select': {
      // The descriptor wires the lookup parameter as `agentId` and the preset as
      // `agentPreset`; both are flat. A wrapped `request` body is refused.
      const expected = { agentId: 'session / preset opaque:%', agentPreset: 'cordis' };
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'agentPresets/select', payload: { args: expected } })) {
        response.writeHead(400); response.end(); return;
      }
      value = 'cordis';
      break;
    }
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
    case 'settings/update':
      // The reference's own write: the namespace and its one string field, no expectedRevision,
      // and nothing else in the body. Anything else is a fixture failure, not a Host behaviour.
      agentPresetWrites++;
      if (!isDeepStrictEqual(args, { ns: 'agent-preset-registry', patch: { selectedDefault: 'ptc' } })) { response.writeHead(400); response.end(); return; }
      if (!['agent-preset-refused', 'agent-preset-conflict'].includes(mode)) agentPresetDefault = 'ptc';
      value = agentPresetNamespace();
      if (mode === 'agent-preset-wrong-ns') value = { ...value, ns: 'probe' };
      if (mode === 'agent-preset-malformed') value = { ...value, schema: 'PRIVATE_SETTINGS_SENTINEL', revision: 'not-a-number' };
      break;
    case 'settings/mutate':
      pluginMutations++;
      if (!isDeepStrictEqual(args, { ns: 'probe', ops: [{ op: 'set', path: ['count'], value: 5 }], expectedRevision: 3 })) { response.writeHead(400); response.end(); return; }
      if (!['plugin-conflict', 'plugin-rejected'].includes(mode)) { pluginCount = 5; pluginRevision = 4; }
      value = pluginNamespace(); break;
    case 'account/getState': value = { status: 'signed-out', links: { usageUrl: 'https://example.invalid/usage', topUpUrl: 'https://example.invalid/topup' }, attempt: null }; break;
    case 'terminal/list':
      if (!isDeepStrictEqual(args, { sessionId: terminalAgent })) { response.writeHead(400); response.end(); return; }
      value = [terminalInfo()];
      if (mode === 'terminal-list-empty') value = [];
      if (mode === 'terminal-list-duplicate') value = [terminalInfo(), terminalInfo()];
      if (mode === 'terminal-list-oversize') value = Array.from({ length: 4097 }, () => terminalInfo());
      if (mode === 'terminal-list-bad-info') value = [{ ...terminalInfo(), cols: 1 }];
      break;
    case 'terminal/shells':
      if (!isDeepStrictEqual(args, { agentId: terminalAgent })) { response.writeHead(400); response.end(); return; }
      value = [{ path: '/bin/PUBLIC-shell', args: ['-l'], name: 'PUBLIC shell' }];
      if (mode === 'terminal-shells-bad') value = [{ path: '', args: [], name: 'PUBLIC shell' }];
      if (mode === 'terminal-shells-oversize') value = Array.from({ length: 257 }, (_, i) => ({ path: `/bin/s${i}`, args: [], name: `s${i}` }));
      break;
    case 'terminal/environment':
      if (!isDeepStrictEqual(args, { agentId: terminalAgent })) { response.writeHead(400); response.end(); return; }
      value = { cwd: '/PUBLIC/cwd', maxInputBytes: 65536, maxCols: 500, maxRows: 200, scrollback: 1000 };
      if (mode === 'terminal-environment-bad') value.maxCols = 1;
      break;
    case 'terminal/create': {
      const createRequest = { id: 'PUBLIC-terminal-1', cols: 80, rows: 24 };
      if (mode === 'terminal-create-shell') createRequest.shellPath = '/bin/PUBLIC-shell';
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'terminal/create', payload: { args: { agentId: terminalAgent, request: createRequest } } })) {
        response.writeHead(400); response.end(); return;
      }
      value = terminalInfo();
      if (mode === 'terminal-create-missing') value = {};
      if (mode === 'terminal-create-wrong-id') value = { ...terminalInfo(), id: 'PUBLIC-other-terminal' };
      if (mode === 'terminal-create-wrong-type') value = { ...terminalInfo(), cols: 'eighty' };
      if (mode === 'terminal-create-extra') value = { ...terminalInfo(), agent: {} };
      if (mode === 'terminal-create-running-exit') value = { ...terminalInfo(), exitCode: 5 };
      break;
    }
    case 'terminal/write':
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'terminal/write', payload: { args: { agentId: terminalAgent, id: 'PUBLIC-terminal-1', attachmentId: 'PUBLIC-attachment-1', data: 'PUBLIC input\n' } } })) {
        response.writeHead(400); response.end(); return;
      }
      value = undefined; break;
    case 'terminal/resize':
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'terminal/resize', payload: { args: { agentId: terminalAgent, id: 'PUBLIC-terminal-1', attachmentId: 'PUBLIC-attachment-1', cols: 120, rows: 40 } } })) {
        response.writeHead(400); response.end(); return;
      }
      value = undefined; break;
    case 'terminal/close':
      if (request.method !== 'POST' || request.headers['content-type'] !== 'application/json'
          || !isDeepStrictEqual(envelope, { type: 'client-request', rpcId: envelope.rpcId, method: 'terminal/close', payload: { args: { agentId: terminalAgent, id: 'PUBLIC-terminal-1' } } })) {
        response.writeHead(400); response.end(); return;
      }
      value = undefined; break;
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
  if (envelope.method === 'settings/update' && ['agent-preset-refused', 'agent-preset-conflict'].includes(mode)) output.result = { ok: false, error: { code: mode === 'agent-preset-conflict' ? 'settings/conflict' : 'settings/rejected', message: 'PRIVATE_SETTINGS_SENTINEL', details: {} } };
  if (envelope.method === 'settings/mutate' && ['plugin-conflict', 'plugin-rejected', 'plugin-postcommit'].includes(mode)) output.result = { ok: false, error: { code: mode === 'plugin-conflict' ? 'settings/conflict' : mode === 'plugin-rejected' ? 'settings/rejected' : 'gateway/internal', message: 'PRIVATE_PLUGIN_SENTINEL', details: {} } };
  if (mode === 'commands-context-missing' && envelope.method === 'commands/list') output.result = { ok: false, error: { code: 'gateway/context-not-found', message: 'PRIVATE_CONTEXT_SENTINEL', details: {} } };
  if (mode === 'fork-unavailable') output.result = { ok: false, error: { code: 'session/fork-unavailable', message: 'PRIVATE_FORK_SENTINEL', details: {} } };
  if (mode === 'fork-attach-failed') output.result = { ok: false, error: { code: 'session/workspace-attach-failed', message: 'PRIVATE_FORK_SENTINEL', details: {} } };
  if (mode === 'failure') output.result = { ok: false, error: { code: 'fixture/rejected', message: 'SECRET_SENTINEL_opaque_url_cookie', details: {} } };
  if (mode === 'terminal-unavailable' && envelope.method.startsWith('terminal/')) output.result = { ok: false, error: { code: 'terminal/unavailable', message: 'PRIVATE_TERMINAL_SENTINEL', details: {} } };
  if (mode === 'terminal-control-unavailable' && ['terminal/write', 'terminal/resize'].includes(envelope.method)) output.result = { ok: false, error: { code: 'terminal/control-unavailable', message: 'PRIVATE_TERMINAL_SENTINEL', details: { reason: 'read-only' } } };
  if (mode === 'terminal-limit-reached' && envelope.method === 'terminal/create') output.result = { ok: false, error: { code: 'terminal/limit-reached', message: 'PRIVATE_TERMINAL_SENTINEL', details: { limit: 8 } } };
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
      case 'job/follow': {
        // The descriptor accepts exactly sessionId/jobId/from; a resume cursor named
        // `offset` is an unexpected key and must not open the stream.
        const expected = { type: 'open', streamId: id, endpoint: 'job/follow', payload: { args: { request: { sessionId: 'session / job opaque:%', jobId: 'PUBLIC-job', from: 5 } } } };
        if (!isDeepStrictEqual(message, expected)) {
          ws.send(JSON.stringify({ type: 'error', streamId: id, error: { code: 'fixture/arguments', message: 'PRIVATE_JOB_SENTINEL', details: {} } }));
          break;
        }
        send({ type: 'opened', job: { id: 'PUBLIC-job', kind: 'bash', label: 'PUBLIC job', status: 'running' }, from: 5 });
        break;
      }
      case 'terminal/follow': {
        const expected = { type: 'open', streamId: id, endpoint: 'terminal/follow', payload: { args: { agentId: terminalAgent, id: 'PUBLIC-terminal-1', attachmentId: 'PUBLIC-attachment-1' } } };
        if (!isDeepStrictEqual(message, expected)) { ws.close(1008); return; }
        if (mode === 'terminal-follow-bad-opening') { send({ type: 'output', sequence: 1, data: 'PUBLIC gap' }); break; }
        if (mode === 'terminal-follow-state-first') { send({ type: 'state', info: terminalInfo() }); break; }
        if (mode === 'terminal-follow-missing-sequence') { send({ type: 'snapshot', screen: 'PUBLIC screen', info: terminalInfo() }); break; }
        send({ type: 'snapshot', sequence: 0, screen: 'PUBLIC screen', info: { ...terminalInfo(), controllerId: 'PUBLIC-attachment-1' } });
        if (mode === 'terminal-follow-gap') { send({ type: 'output', sequence: 2, data: 'PUBLIC gap' }); break; }
        if (mode === 'terminal-follow-extra-tag') { send({ type: 'retained' }); break; }
        send({ type: 'output', sequence: 1, data: 'PUBLIC first\n' });
        send({ type: 'state', info: { ...terminalInfo(), cols: 120, rows: 40, controllerId: 'PUBLIC-attachment-1' } });
        send({ type: 'output', sequence: 2, data: 'PUBLIC second\n' });
        break;
      }
      case 'userQuestions/attachWait': {        if (!mode.startsWith('claim-')) { send({ remainingMs: 1234 }); break; }
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
