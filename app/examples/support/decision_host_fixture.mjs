// Fixture-only Loader plugin; real Agent/turn/services/Remote Events, never a fabricated wire frame.
import { randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import { createRequire } from 'node:module';
import { watch, lstatSync, existsSync } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
export const name = 'native-decision-host-fixture';
export const inject = ['agents', 'agentLoop', 'approval', 'userQuestions'];
const directory = dirname(fileURLToPath(import.meta.url));
const control = join(directory, 'control');
const PUBLIC_NOTE = 'PUBLIC_NATIVE_DECISION_FIXTURE_NOTE';

function marker(name) {
  const path = join(control, name);
  if (!existsSync(path)) return false;
  const info = lstatSync(path);
  if (!info.isFile() || info.isSymbolicLink() || info.size > 16 || (info.mode & 0o777) !== 0o600) throw new Error('fixture-marker-invalid');
  return true;
}
function waitMarker(name, signal) {
  return new Promise((resolve, reject) => {
    let closed = false;
    const stop = error => {
      if (closed) return;
      closed = true; watcher.close(); signal.removeEventListener('abort', abort);
      error ? reject(error) : resolve();
    };
    const abort = () => stop(new Error('fixture-aborted'));
    const check = () => { try { if (marker(name)) stop(); } catch { stop(new Error('fixture-marker-invalid')); } };
    const watcher = watch(control, (_event, file) => { if (file?.toString() === name) check(); });
    watcher.on('error', () => stop(new Error('fixture-watch-failed')));
    signal.addEventListener('abort', abort, {once:true});
    if (signal.aborted) abort(); else check(); // Reconcile after registering: no lost prewritten marker.
  });
}
const rawReceipt = (name, value) => writeFile(join(control, name), JSON.stringify(value), {flag:'wx', mode:0o600});
async function packageValue(runtime, name) {
  const cliRequire = createRequire(join(runtime, 'package.json'));
  const require = createRequire(cliRequire.resolve('@deepseek-ai/dsh-base/package.json'));
  const { readFile } = await import('node:fs/promises');
  const manifest = JSON.parse(await readFile(require.resolve(`${name}/package.json`), 'utf8'));
  if (manifest.version !== '0.2.1-alpha.1') throw new Error('fixture-runtime-version');
  return import(pathToFileURL(require.resolve(name)).href);
}

export function apply(ctx, config) {
  const lifetime = new AbortController();
  let handle;
  let subject;
  let receiving = true;
  const receipt = (name, value) => {
    if (!receiving || lifetime.signal.aborted) throw new Error('fixture-stopping');
    return rawReceipt(name, value);
  };
  let auditId;
  const counts = {turnStart:0,turnEnd:0,steps:0,requestHeaders:0,toolCalls:0,toolResults:0,assistantMessages:0,approvalAsked:0,approvalDecided:0};
  let turnEnd;
  const ended = new Promise(resolve => { turnEnd = resolve; });
  ctx.on('session/event', (session, event) => {
    if (!receiving || session !== subject?.session) return;
    switch (event.type) {
      case 'turn/start': counts.turnStart++; break;
      case 'turn/end': counts.turnEnd++; turnEnd(); break;
      case 'step/start': counts.steps++; break;
      case 'request/header': counts.requestHeaders++; break;
      case 'tool/call': counts.toolCalls++; break;
      case 'tool/result': counts.toolResults++; break;
      case 'assistant/message': counts.assistantMessages++; break;
      case 'approval/asked': counts.approvalAsked++; auditId = event.data.id; break;
      case 'approval/decided': if (event.data.id === auditId && event.data.outcome === 'allowed-once') counts.approvalDecided++; break;
    }
  });
  const deadline = setTimeout(() => lifetime.abort(new Error('fixture-deadline')), 30000);
  const work = (async () => {
    const [{SessionId}, {createUserMessage}] = await Promise.all([
      packageValue(config.runtime, '@deepseek-ai/dsh-session'), packageValue(config.runtime, '@deepseek-ai/dsh-llm'),
    ]);
    let first = true;
    handle = await ctx.agents.create({sessionId:SessionId(randomUUID()), meta:{cwd:config.workspace}, signal:lifetime.signal,
      setup(agentCtx, agent) {
        subject = agent;
        agentCtx.on('agent/pre-step', async ({agent:current, signal}, next) => {
          if (current !== agent) return next();
          if (!first) return {kind:'reject'}; // Every unexpected later wake still cannot admit a model step.
          first = false;
          const shared = AbortSignal.any([signal, lifetime.signal]);
          const rootRegistered = ctx.agents.get(agent.id) === agent && ctx.agents.roots().includes(agent);
          const outcome = await ctx.approval.request({agent,toolName:'PUBLIC_fixture_no_operation',reason:'Fixture-only live decision; no command or tool will execute.',signal:shared});
          await receipt('phase-1.json', {phase:1,actualAgentId:agent.id,rootRegistered,settled:outcome === 'allowed-once',approvalOutcome:outcome});
          await waitMarker('go-2', shared);
          const answer = await ctx.userQuestions.ask({agent,signal:shared,questions:[
            {id:'choice',question:'Select both PUBLIC fixture choices.',multiSelect:true,options:[{label:'One'},{label:'Two'}]},
            {id:'note',question:'Enter the fixed PUBLIC fixture note.'},
            {id:'skip',question:'Explicitly skip this PUBLIC fixture question.',options:[{label:'Not selected'}]},
          ]});
          const expectedAnswerMatched = isDeepStrictEqual(answer, {answers:[{id:'choice',selected:['One','Two']},{id:'note',selected:[],custom:PUBLIC_NOTE},{id:'skip',selected:[]}]});
          await receipt('phase-2.json', {phase:2,actualAgentId:agent.id,settled:expectedAnswerMatched,expectedAnswerMatched,answerCount:answer.answers.length});
          await waitMarker('go-3', shared);
          let cancelCode;
          try { await ctx.userQuestions.ask({agent,signal:shared,questions:[{id:'cancel',question:'Cancel only this PUBLIC question, not its Agent turn.'}]}); }
          catch (error) { cancelCode = error?.name === 'UserQuestionError' && error?.code === 'ASK_CANCELLED' ? 'ASK_CANCELLED' : 'unexpected'; }
          await receipt('phase-3.json', {phase:3,actualAgentId:agent.id,settled:cancelCode === 'ASK_CANCELLED',code:cancelCode ?? 'unexpected'});
          await waitMarker('go-4', shared);
          const phase = new AbortController();
          const signal4 = AbortSignal.any([shared, phase.signal]);
          const abortTask = waitMarker('abort-4', signal4).then(() => phase.abort(new Error('PUBLIC fixture Host abortion')), () => {});
          let abortCode;
          try { await ctx.userQuestions.ask({agent,signal:signal4,questions:[{id:'abort',question:'Host will abort this PUBLIC question; do not answer.'}]}); }
          catch (error) { abortCode = error?.name === 'UserQuestionError' && error?.code === 'ASK_ABORTED' ? 'ASK_ABORTED' : 'unexpected'; }
          finally { phase.abort(); await abortTask; }
          await receipt('phase-4.json', {phase:4,actualAgentId:agent.id,settled:abortCode === 'ASK_ABORTED',code:abortCode ?? 'unexpected'});
          return {kind:'reject'};
        }, {prepend:true});
      },
    });
    await receipt('ready.json', {actualAgentId:handle.agent.id,rootRegistered:ctx.agents.get(handle.agent.id) === handle.agent && ctx.agents.roots().includes(handle.agent)});
    await waitMarker('go-1', lifetime.signal);
    handle.agent.followup(createUserMessage({source:{kind:'user'},content:[{type:'text',text:'PUBLIC fixture wake; reject before any model step.'}]}));
    await Promise.race([ended, new Promise((_,reject) => {
      const fail = () => reject(new Error('fixture-aborted'));
      if (lifetime.signal.aborted) fail(); else lifetime.signal.addEventListener('abort',fail,{once:true});
    })]);
    await handle.agent.whenIdle();
    await handle.dispose();
    const disposed = ctx.agents.get(subject.id) !== subject;
    await receipt('complete.json', {disposed,turnClosed:counts.turnStart === 1 && counts.turnEnd === 1,counts,approvalAuditPair:counts.approvalAsked === 1 && counts.approvalDecided === 1,zeroAdmittedModelSteps:counts.steps === 0 && counts.requestHeaders === 0 && counts.toolCalls === 0 && counts.toolResults === 0 && counts.assistantMessages === 0});
  })().catch(async () => {
    try { await rawReceipt('failed.json', {code:'fixture-host-failed'}); } catch { /* Fixed failure only; file may already exist or intake may be stopping. */ }
  });
  ctx.effect(() => async () => {
    receiving = false; // Unloading must never publish successful late receipts.
    clearTimeout(deadline); lifetime.abort(new Error('fixture-disposed'));
    if (handle) handle.agent.cancel({kind:'hook',reason:'fixture teardown'});
    await work;
    if (handle) await handle.dispose();
  });
}
