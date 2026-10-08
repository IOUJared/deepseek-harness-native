// Fixed test-only Loader contribution: real Agent/inbox admission, reject every owned model step.
import { createHash, randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { existsSync, lstatSync, watch } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const name = 'native-file-prompt-host-fixture';
export const inject = ['agents', 'agentLoop'];
const control = join(dirname(fileURLToPath(import.meta.url)), 'control');
const PUBLIC_TEXT = 'PUBLIC native file prompt; reject before any model step.';
function digest(bytes, multiplier, offset) {
  const hash = createHash('sha256');
  const chunk = Buffer.alloc(65536);
  for (let start = 0; start < bytes; start += chunk.length) {
    const size = Math.min(chunk.length, bytes - start);
    for (let index = 0; index < size; index++) chunk[index] = ((start + index) * multiplier + offset) % 256;
    hash.update(chunk.subarray(0, size));
  }
  return `sha256:${hash.digest('hex')}`;
}
const expected = [
  { name: 'PUBLIC owned file.bin', bytes: 5003, attachmentId: digest(5003, 43, 19), text: true },
  { name: 'PUBLIC empty.bin', bytes: 0, attachmentId: digest(0, 0, 0), text: false },
  { name: 'PUBLIC ceiling.bin', bytes: 4 * 1024 * 1024, attachmentId: digest(4 * 1024 * 1024, 13, 7), text: true },
].map((file, index) => ({ ...file, requestId: `PUBLIC_native_file_prompt_${index + 1}` }));

function marker() {
  const path = join(control, 'go-close');
  if (!existsSync(path)) return false;
  const info = lstatSync(path);
  if (!info.isFile() || info.isSymbolicLink() || info.size > 16 || (info.mode & 0o7777) !== 0o600) throw new Error('fixture-marker-invalid');
  return true;
}
function waitClose(signal) {
  return new Promise((resolve, reject) => {
    let closed = false;
    const stop = error => {
      if (closed) return;
      closed = true;
      watcher.close();
      signal.removeEventListener('abort', abort);
      error ? reject(error) : resolve();
    };
    const abort = () => stop(new Error('fixture-aborted'));
    const check = () => { try { if (marker()) stop(); } catch { stop(new Error('fixture-marker-invalid')); } };
    const watcher = watch(control, (_event, file) => { if (file?.toString() === 'go-close') check(); });
    watcher.on('error', () => stop(new Error('fixture-watch-failed')));
    signal.addEventListener('abort', abort, { once: true });
    if (signal.aborted) abort(); else check();
  });
}
async function sessionPackage(runtime) {
  const cliRequire = createRequire(join(runtime, 'package.json'));
  const baseRequire = createRequire(cliRequire.resolve('@deepseek-ai/dsh-base/package.json'));
  const manifest = JSON.parse(await readFile(baseRequire.resolve('@deepseek-ai/dsh-session/package.json'), 'utf8'));
  if (manifest.version !== '0.2.1-alpha.1') throw new Error('fixture-runtime-version');
  return import(pathToFileURL(baseRequire.resolve('@deepseek-ai/dsh-session')).href);
}
function rawReceipt(name, value) {
  const bytes = JSON.stringify(value);
  if (Buffer.byteLength(bytes) > 16384) throw new Error('fixture-receipt-bound');
  return writeFile(join(control, name), bytes, { flag: 'wx', mode: 0o600 });
}

// appRequests: true accepts three native UI Sends; omission/false keeps the fixed headless IDs.
export function apply(ctx, config) {
  if (config.appRequests !== undefined && typeof config.appRequests !== 'boolean') throw new Error('fixture-app-requests-invalid');
  const appRequests = config.appRequests === true;
  let appIdentity;
  const lifetime = new AbortController();
  let receiving = true;
  let failed = false;
  let failureWrite = Promise.resolve();
  let writes = Promise.resolve();
  let handle;
  let subject;
  let activeTurn = 0;
  const counts = { turnStart: 0, turnEnd: 0, blockedTurns: 0, inboxInserted: 0, inboxClaims: 0, preSteps: 0,
    steps: 0, stepEnds: 0, requestHeaders: 0, toolCalls: 0, toolResults: 0, assistantMessages: 0, userMessages: 0 };
  const states = expected.map(() => ({ requestId: undefined, inserted: false, removed: false,
    claimed: false, preStep: false, started: false, ended: false, published: false }));
  const rootRegistered = () => ctx.agents.get(subject.id) === subject && ctx.agents.roots().includes(subject);
  const fail = () => {
    if (failed || !receiving) return;
    failed = true;
    lifetime.abort(new Error('fixture-failed'));
    if (subject && ctx.agents.get(subject.id) === subject) {
      try { subject.cancel({ kind: 'hook', reason: 'fixed file prompt fixture failure' }); }
      catch { /* The fixed failure stays latched; owned handle disposal still drains teardown. */ }
    }
    failureWrite = Promise.resolve().then(() => rawReceipt('failed.json', { code: 'fixture-host-failed' })).catch(() => {
      // A static failure remains failed even when the private report cannot be written.
    });
  };
  const publish = (name, value) => {
    if (!receiving || lifetime.signal.aborted) throw new Error('fixture-stopping');
    writes = writes.then(() => {
      if (!receiving || lifetime.signal.aborted) throw new Error('fixture-stopping');
      return rawReceipt(name, value);
    });
    void writes.catch(fail);
    return writes;
  };
  const zeroModelWork = () => ['steps', 'stepEnds', 'requestHeaders', 'toolCalls', 'toolResults', 'assistantMessages', 'userMessages']
    .every(key => counts[key] === 0);
  function nativeRequest(requestId, index) {
    if (typeof requestId !== 'string' || requestId.length > 256) throw new Error('fixture-request-invalid');
    // Full-match comparison also refuses the trailing newline accepted by a JavaScript `$` anchor.
    const match = /^native-([1-9][0-9]*)-(0|[1-9][0-9]*)-(0|[1-9][0-9]*)-([1-3])$/.exec(requestId);
    if (!match || match[0] !== requestId || match[4] !== String(index + 1)
      || (appIdentity && (match[1] !== appIdentity.pid || match[3] !== appIdentity.generation))) throw new Error('fixture-request-invalid');
    return { pid: match[1], generation: match[3] };
  }
  function messageIndex(message, firstReceipt = false) {
    if (message.source?.kind !== 'user' || message.role !== 'user') throw new Error('fixture-message-invalid');
    const requestId = message.source.rpcId;
    const parts = message.content;
    if (!Array.isArray(parts)) throw new Error('fixture-content-invalid');
    const part = parts.at(-1);
    const matches = expected.map((file, index) => ({ file, index })).filter(({ file }) => appRequests
      ? part?.type === 'file' && part.attachment?.attachmentId === file.attachmentId
        && part.attachment.name === file.name && part.attachment.bytes === file.bytes
      : requestId === file.requestId);
    if (matches.length !== 1) throw new Error('fixture-message-invalid');
    const { file, index } = matches[0];
    if (parts.length !== (file.text ? 2 : 1)) throw new Error('fixture-content-invalid');
    if (file.text && (parts[0].type !== 'text' || parts[0].text !== PUBLIC_TEXT)) throw new Error('fixture-text-invalid');
    if (part.type !== 'file' || part.receiptId !== undefined || part.attachment?.attachmentId !== file.attachmentId
      || part.attachment.name !== file.name || part.attachment.bytes !== file.bytes) throw new Error('fixture-file-invalid');
    if (appRequests) nativeRequest(requestId, index);
    if (!firstReceipt && (!states[index].inserted || states[index].requestId !== requestId)) throw new Error('fixture-request-unbound');
    return index;
  }
  function maybePhase(index) {
    const state = states[index];
    if (state.published || !state.inserted || !state.removed || !state.claimed || !state.preStep || !state.started || !state.ended) return;
    if (failed || !rootRegistered() || !zeroModelWork()
      || counts.turnStart !== index + 1 || counts.turnEnd !== index + 1 || counts.blockedTurns !== index + 1
      || counts.inboxInserted !== index + 1 || counts.inboxClaims !== index + 1 || counts.preSteps !== index + 1) throw new Error('fixture-phase-invalid');
    state.published = true;
    void publish(`phase-${index + 1}.json`, { phase: index + 1, requestId: state.requestId,
      file: { ...state.file },
      rootRegistered: true, durableInboxFileMatched: true, claimedFileMatched: true,
      blockedTurn: true, zeroAdmittedModelSteps: true, counts: { ...counts } });
  }
  ctx.on('session/event', (session, event) => {
    if (!receiving || failed || session !== subject?.session) return;
    try {
      switch (event.type) {
        case 'agent/inbox/spliced': {
          const { inserted, target, start, removedCount, outcome } = event.data;
          if (inserted.length) {
            if (inserted.length !== 1 || target !== 'next-turn' || start !== 0 || removedCount !== undefined || outcome !== undefined) throw new Error('fixture-insertion-invalid');
            const index = messageIndex(inserted[0], true);
            const state = states[index];
            const requestId = inserted[0].source.rpcId;
            if (state.inserted || state.requestId !== undefined || index !== counts.inboxInserted
              || (index > 0 && !states[index - 1].published)
              || states.some(other => other.requestId === requestId)) throw new Error('fixture-insertion-order');
            if (appRequests) appIdentity ??= nativeRequest(requestId, index);
            // Only this first committed insertion may bind the state's exact observed request ID.
            state.requestId = requestId;
            const attachment = inserted[0].content.at(-1).attachment;
            state.file = { attachmentId: attachment.attachmentId, name: attachment.name, bytes: attachment.bytes };
            state.inserted = true;
            counts.inboxInserted++;
            maybePhase(index);
          } else if (removedCount !== undefined) {
            const index = activeTurn - 1;
            if (target !== 'next-turn' || start !== 0 || removedCount !== 1 || outcome !== undefined || index < 0 || index >= 3
              || !states[index].inserted || !states[index].started || states[index].removed || states[index].claimed) throw new Error('fixture-claim-invalid');
            states[index].removed = true;
            maybePhase(index);
          }
          break;
        }
        case 'turn/start': {
          const index = event.data.turn - 1;
          if (index < 0 || index >= 3 || !states[index].inserted || states[index].started
            || event.data.turn !== counts.turnStart + 1 || counts.turnEnd !== index
            || (index > 0 && !states[index - 1].published)) throw new Error('fixture-turn-invalid');
          activeTurn = event.data.turn;
          states[index].started = true;
          counts.turnStart++;
          break;
        }
        case 'turn/end': {
          const index = event.data.turn - 1;
          if (index < 0 || index >= 3 || !states[index].started || !states[index].claimed || !states[index].preStep
            || states[index].ended || event.data.turn !== activeTurn || index !== counts.turnEnd
            || event.data.reason.kind !== 'blocked') throw new Error('fixture-turn-end-invalid');
          states[index].ended = true;
          counts.turnEnd++;
          counts.blockedTurns++;
          maybePhase(index);
          break;
        }
        case 'step/start': counts.steps++; fail(); break;
        case 'step/end': counts.stepEnds++; fail(); break;
        case 'request/header': counts.requestHeaders++; fail(); break;
        case 'tool/call': counts.toolCalls++; fail(); break;
        case 'tool/result': counts.toolResults++; fail(); break;
        case 'assistant/message': counts.assistantMessages++; fail(); break;
        case 'user/message': counts.userMessages++; fail(); break;
      }
    } catch { fail(); } // Emit observers cannot make a committed append fail; latch a separate fixture failure.
  });
  const deadline = setTimeout(fail, 18000);
  const work = (async () => {
    const { SessionId } = await sessionPackage(config.runtime);
    handle = await ctx.agents.create({ sessionId: SessionId(randomUUID()), meta: { cwd: config.workspace }, signal: lifetime.signal,
      setup(agentCtx, agent) {
        subject = agent;
        agentCtx.on('agent/inbox/claimed', ({ agent: current, message, turn }) => {
          if (current !== agent || !receiving || failed) return;
          try {
            const index = messageIndex(message);
            const state = states[index];
            if (turn !== index + 1 || turn !== activeTurn || !state.started || !state.removed
              || state.claimed || state.preStep || state.ended || index !== counts.inboxClaims) throw new Error('fixture-claim-order');
            state.claimed = true;
            counts.inboxClaims++;
            maybePhase(index);
          } catch { fail(); }
        });
        agentCtx.on('agent/pre-step', async ({ agent: current, messages, turn, step }, next) => {
          if (current !== agent) return next();
          try {
            if (failed || !receiving || messages.length !== 1 || step !== 1 || !rootRegistered()) throw new Error('fixture-pre-step-invalid');
            const index = messageIndex(messages[0]);
            if (turn !== index + 1 || turn !== activeTurn || !states[index].started || !states[index].removed
              || !states[index].claimed || states[index].preStep || states[index].ended
              || index !== counts.preSteps) throw new Error('fixture-pre-step-order');
            states[index].preStep = true;
            counts.preSteps++;
            maybePhase(index);
          } catch { fail(); }
          return { kind: 'reject' }; // Every owned wake is refused, including unexpected or malformed fixture input.
        }, { prepend: true });
      },
    });
    const header = subject.session.header;
    if (!rootRegistered() || header.id !== subject.id || header.version !== 4 || header.origin !== undefined) throw new Error('fixture-root-invalid');
    await publish('ready.json', { actualAgentId: subject.id, rootRegistered: true, headerVersion: 4, ordinary: true });
    await waitClose(lifetime.signal);
    await subject.whenIdle();
    await writes;
    if (failed || !states.every(state => state.published) || counts.turnStart !== 3 || counts.turnEnd !== 3
      || counts.blockedTurns !== 3 || !zeroModelWork()) throw new Error('fixture-completion-invalid');
    await handle.dispose();
    if (ctx.agents.get(subject.id) === subject) throw new Error('fixture-disposal-invalid');
    await publish('complete.json', { status: 'passed', rootDisposed: true, turnClosed: true,
      zeroAdmittedModelSteps: true, counts: { ...counts },
      scope: 'actual file prompt durable inbox admission and owned pre-step rejection; not user-message history retirement' });
    clearTimeout(deadline);
  })().catch(fail);
  ctx.effect(() => async () => {
    receiving = false;
    clearTimeout(deadline);
    lifetime.abort(new Error('fixture-disposed'));
    if (subject && ctx.agents.get(subject.id) === subject) {
      try { subject.cancel({ kind: 'hook', reason: 'fixed file prompt fixture teardown' }); }
      catch { /* The owned handle remains the teardown authority even if cancellation fails. */ }
    }
    await work;
    if (handle) await handle.dispose();
    try { await writes; } catch { /* Closing intentionally prevents any queued successful late receipt. */ }
    await failureWrite;
  });
}
