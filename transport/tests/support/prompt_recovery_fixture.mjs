// PUBLIC localhost-only wire fixture. No Harness, provider, OAuth or model calls.
import http from 'node:http';
import crypto from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
const mode=process.argv[2]??'normal';
let origin,cookieName;
const held=[];
let cancels=0;
const inbox=['PUBLIC queued input'];
const reply=(response,envelope,value)=>{response.writeHead(200,{'content-type':'application/json'});response.end(JSON.stringify({type:'server-response',rpcId:envelope.rpcId,result:{ok:true,value}}));};
const server=http.createServer(async(request,response)=>{
  if(request.method==='GET'&&request.url==='/?token=PUBLIC-fixture'){
    response.writeHead(303,{location:'./','set-cookie':`${cookieName}=v1.PUBLIC.signature; HttpOnly; Path=/; SameSite=Strict; Max-Age=3600; Expires=${new Date(Date.now()+3600000).toUTCString()}`});response.end();return;
  }
  if(request.method!=='POST'||request.headers.host!==origin.slice(7)||request.headers.origin!==origin||request.headers.cookie!==`${cookieName}=v1.PUBLIC.signature`){response.writeHead(403);response.end();return;}
  const chunks=[];for await(const chunk of request)chunks.push(chunk);
  let envelope;try{envelope=JSON.parse(Buffer.concat(chunks));}catch{response.writeHead(400);response.end();return;}
  if(envelope.type!=='client-request'||request.url!==`/api/${envelope.method}`){response.writeHead(400);response.end();return;}
  if(envelope.method==='session/list'){
    if(!isDeepStrictEqual(envelope.payload.args,{_request:{}})){response.writeHead(400);response.end();return;}
    if(mode==='held'&&cancels===0){held.push([response,envelope]);if(held.length===4)process.stdout.write('HELD4\n');return;}
    reply(response,envelope,{items:[{agentAvailable:true,sessionId:'PUBLIC session',updatedAt:0,running:false,blank:false,cwd:`retained:${inbox.length};cancels:${cancels}`} ]});return;
  }
  if(envelope.method==='session/cancel'){
    if(!isDeepStrictEqual(envelope.payload.args,{request:{sessionId:'PUBLIC session'}})){response.writeHead(400);response.end();return;}
    cancels++; // Corresponds to BFF agent.cancel(...,{keepInbox:true}); never clears queued input.
    reply(response,envelope,{accepted:true});
    for(const[r,e]of held.splice(0))reply(r,e,{items:[{agentAvailable:true,sessionId:'PUBLIC session',updatedAt:0,running:false,blank:false,cwd:`retained:${inbox.length};cancels:${cancels}`} ]});return;
  }
  if(envelope.method==='session/promptAdmission'){
    if(!isDeepStrictEqual(envelope.payload.args,{request:{sessionId:'PUBLIC session',requestId:'PUBLIC original request'}})){response.writeHead(400);response.end();return;}
    const values={normal:{state:'accepted',evidence:'journal',phase:'recorded',messageId:'PUBLIC opaque message / id',seq:0},unknown:{state:'unknown'},admitting:{state:'admitting'},rejected:{state:'rejected',code:'session/agent-busy'},internal:{state:'rejected',code:'gateway/internal'},secret:{state:'unknown',grant:'PRIVATE sentinel'},unsafe:{state:'accepted',evidence:'journal',phase:'recorded',seq:9007199254740992},invalid:{state:'accepted',evidence:'future',phase:'recorded'}};
    if(mode==='missing'){reply(response,envelope,{PRIVATE:'not a receipt'});return;}
    reply(response,envelope,values[mode]??values.normal);return;
  }
  response.writeHead(400);response.end();
});
server.listen(0,'127.0.0.1',()=>{origin=`http://127.0.0.1:${server.address().port}`;cookieName=`dsh-auth-${crypto.createHash('sha256').update(origin.slice(7)).digest('base64url')}`;process.stdout.write(`${origin}/?token=PUBLIC-fixture\n`);});
process.on('SIGTERM',()=>{for(const[response]of held.splice(0))response.destroy();server.close(()=>process.exit(0));});
