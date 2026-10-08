// Pure loopback wire assertions; no Harness, profile, model or real credentials.
import http from 'node:http';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const mode = process.argv[2];
const sessionId = 'session / opaque:%?&=💾', name = 'PUBLIC report é.txt';
let cookie, origin, count = 0, failed = false;
const server = http.createServer(async (req, res) => {
  try {
    if (req.method === 'GET' && req.url === '/?token=PUBLIC_UPLOAD_LAUNCH') {
      assert.equal(req.headers.cookie, undefined);
      res.writeHead(303, { location: './', 'set-cookie': `${cookie}; Path=/; HttpOnly; SameSite=Strict; Max-Age=3600; Expires=${new Date(Date.now()+3600000).toUTCString()}` }); res.end(); return;
    }
    assert.notEqual(mode, 'no-upload', 'expired worker must not send a request');
    const url = new URL(req.url, origin);
    assert.equal(req.method, 'POST'); assert.equal(url.pathname, '/api/session/uploadFileBinary');
    assert.equal(url.searchParams.get('sessionId'), sessionId);
    assert.equal(url.searchParams.get('agentId'), null);
    assert.equal(url.searchParams.get('name'), mode === 'empty' ? null : name);
    assert.equal(req.headers.cookie, cookie); assert.equal(req.headers.origin, origin);
    assert.equal(req.headers['content-type'], 'application/octet-stream');
    assert.equal(req.headers['transfer-encoding'], undefined);
    const chunks = []; for await (const chunk of req) chunks.push(chunk);
    const bytes = Buffer.concat(chunks);
    const expected = mode === 'empty' ? Buffer.alloc(0) : mode === 'max' ? Buffer.alloc(4*1024*1024, 0xab) : Buffer.from([0,255,128,10]);
    assert.deepEqual(bytes, expected); assert.equal(Number(req.headers['content-length']), expected.length);
    const index = ++count; process.stdout.write(`upload:${index}\n`);
    res.on('close', () => process.stdout.write(`closed:${index}\n`));
    if (mode === 'http') { res.writeHead(401); res.end('PRIVATE_ERROR'); return; }
    if (mode === 'redirect') { res.writeHead(303, { location: `${origin}/escape?token=PRIVATE_REDIRECT` }); res.end(); return; }
    if (mode === 'wrong-type') { res.writeHead(200, {'content-type':'text/html'}); res.end('PRIVATE_HTML'); return; }
    if (mode === 'oversize') { res.writeHead(200, {'content-type':'application/json','content-length':16385}); res.end('x'.repeat(16385)); return; }
    res.writeHead(200, {'content-type':'application/json'});
    if (mode === 'chunked-over') { res.write(' '.repeat(8192)); res.end(' '.repeat(8193)); return; }
    if (mode === 'hold-all' || (mode === 'hold-first' && index === 1)) { res.write('{"ok":'); return; }
    let value = { ok: true, value: { receiptId: `PUBLIC_RECEIPT_${index}`, file: { attachmentId: `sha256:${'a'.repeat(64)}`, name: 'PUBLIC clean.bin', bytes: expected.length } } };
    if (mode === 'wrong-bytes') value.value.file.bytes++;
    if (mode === 'partial') delete value.value.receiptId;
    if (mode === 'extra') value.value.file.token = 'PRIVATE_EXTRA';
    if (mode === 'remote') value = {ok:false,error:{code:'gateway/internal',message:'PRIVATE_REMOTE_ERROR',details:{token:'PRIVATE_ERROR_TOKEN'}}};
    if (mode === 'bad-error') value = {ok:false,error:{}};
    if (mode === 'extra-error') value = {ok:false,error:{code:'gateway/internal',message:'PRIVATE_ERROR',details:{},extra:'PRIVATE_EXTRA'}};
    if (mode === 'too-many-items') value = {ok:false,error:{code:'gateway/internal',message:'PRIVATE_ERROR',details:{items:Array(129).fill(null)}}};
    if (mode === 'exact-result') { res.end(JSON.stringify(value).padEnd(16384, ' ')); return; }
    if (mode === 'invalid-json') { res.end('{broken PRIVATE_JSON'); return; }
    res.end(JSON.stringify(value));
  } catch (_error) { failed = true; res.destroy(); }
});
server.listen(0, '127.0.0.1', () => {
  const authority = `127.0.0.1:${server.address().port}`; origin = `http://${authority}`;
  cookie = `dsh-auth-${createHash('sha256').update(authority).digest('base64url')}=PUBLIC_UPLOAD_COOKIE`;
  process.stdout.write(`${origin}/?token=PUBLIC_UPLOAD_LAUNCH\n`);
});
process.stdin.resume(); process.stdin.once('data', () => {
  server.closeAllConnections(); server.close(() => process.exit(failed ? 1 : 0));
});
