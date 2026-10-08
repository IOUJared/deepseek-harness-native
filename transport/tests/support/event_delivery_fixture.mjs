// Owned local fake BFF only; no Harness boot, model, profile, credential or external request.
import http from 'node:http';
import crypto from 'node:crypto';
import path from 'node:path';
import { createRequire } from 'node:module';
const require = createRequire(path.resolve(import.meta.dirname, '../../../../deepseek-harness-linux/packages/api/gateway/package.json'));
const { WebSocketServer } = require('ws');
const mode = process.argv[2];
let origin, cookieName, root, rootId, held, reused = false, posts = 0, lists = 0;
function emit(value) { if (root?.readyState === 1) root.send(JSON.stringify({ type: 'item', streamId: rootId, value })); }
function question() { return mode === 'question'; }
function waterfall() { emit({ type: 'waterfall', event: question() ? 'user-questions/request' : 'approval/request', eventId: 'delivery-event', agentId: 'agent-fixture', request: question() ? { questions: [{ id:'q', question:'PUBLIC fixture', options:[{label:'yes'}] }] } : { toolName: reused ? 'new-tool' : 'old-tool' } }); }
function respond(response, envelope, value) { response.end(JSON.stringify({ type:'server-response', rpcId:envelope.rpcId, result:{ok:true, ...(value === undefined ? {} : {value})} })); }
function reuse() {
  if (reused) return;
  emit({type:'cancel',eventId:'delivery-event'});
  reused = true;
  waterfall();
  if (held) { const pending = held; held = undefined; respond(pending.response, pending.envelope, pending.value); }
}
const server = http.createServer(async (request,response) => {
  if (request.method === 'GET' && request.url === '/?token=fixture') {
    response.writeHead(303,{location:'./','set-cookie':`${cookieName}=v1.fixture.signature; HttpOnly; Path=/; SameSite=Strict; Max-Age=3600; Expires=${new Date(Date.now()+3600000).toUTCString()}`}); response.end(); return;
  }
  if (request.headers.origin !== origin || request.headers.cookie !== `${cookieName}=v1.fixture.signature`) { response.writeHead(403); response.end(); return; }
  const chunks=[]; for await (const chunk of request) chunks.push(chunk);
  let envelope; try { envelope=JSON.parse(Buffer.concat(chunks).toString()); } catch { response.writeHead(400);response.end();return; }
  if (request.method !== 'POST' || envelope.type !== 'client-request' || request.url !== `/api/${envelope.method}` || !envelope.payload?.args) { response.writeHead(400);response.end();return; }
  response.setHeader('content-type','application/json');
  const args=envelope.payload.args;
  if (envelope.method === 'session/list') {
    lists++;
    const value={items:[{sessionId:'fixture-session',agentAvailable:false,updatedAt:0,running:false,blank:true,cwd:JSON.stringify({posts,reused})}]};
    if (mode === 'queued' && lists === 1) { held={response,envelope,value};emit({type:'emit',event:'probe/slot-held',args:[]});return; }
    respond(response,envelope,value);return;
  }
  if (envelope.method === '$events/result') {
    if (args.clientId !== 'delivery-client' || args.eventId !== 'delivery-event') { response.writeHead(400); response.end(); return; }
    posts++;
    if (mode === 'ack' && !reused) { held={response,envelope};emit({type:'emit',event:'probe/reply-held',args:[]});return; }
    if (mode === 'failure' && posts === 1) { response.end(JSON.stringify({type:'server-response',rpcId:envelope.rpcId,result:{ok:false,error:{code:'fixture/uncertain',message:'PRIVATE_SENTINEL',details:{}}}}));return; }
    if (question() && (args.outcome?.kind !== 'rejected' || args.outcome.error?.code !== 'ASK_CANCELLED')) { response.writeHead(400);response.end();return; }
    respond(response,envelope);return;
  }
  response.writeHead(404);response.end();
});
const wss = new WebSocketServer({noServer:true});
server.on('upgrade',(request,socket,head)=>{
  if (request.url !== '/api/remote.mux' || request.headers.origin !== origin || request.headers.cookie !== `${cookieName}=v1.fixture.signature`) {socket.destroy();return;}
  wss.handleUpgrade(request,socket,head,ws=>wss.emit('connection',ws));
});
wss.on('connection',ws=>ws.on('message',raw=>{
  const message=JSON.parse(raw.toString());
  if (message.type === 'cancel') return;
  if (message.type !== 'open') {ws.close(1008);return;}
  if (message.endpoint === '$events') {
    root=ws;rootId=message.streamId;
    emit({type:'ready',clientId:'delivery-client',host:{home:'/PUBLIC/fixture'}});
    waterfall();return;
  }
  if (message.endpoint === 'workspace/follow') {
    if (['queued','ack','dequeue'].includes(mode)) reuse();
    ws.send(JSON.stringify({type:'item',streamId:message.streamId,value:{type:'baseline',value:{items:[],archivedSessionIds:[],pinnedSessionIds:[]}}}));return;
  }
  ws.close(1008);
}));
server.listen(0,'127.0.0.1',()=>{
  origin=`http://127.0.0.1:${server.address().port}`;
  cookieName=`dsh-auth-${crypto.createHash('sha256').update(origin.slice(7)).digest('base64url')}`;
  process.stdout.write(`${origin}/?token=fixture\n`);
});
