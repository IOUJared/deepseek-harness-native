// Pure Node loopback fixture. No real Harness, profiles, GUI, or external requests.
import http from 'node:http';
import { createHash } from 'node:crypto';
import { crc32, deflateRawSync } from 'node:zlib';
import assert from 'node:assert/strict';

const mode = process.argv[2] ?? 'stored';
const sessionId = 'session / opaque:%?&=💾';
const expectedPath = `/api/session.export?${new URLSearchParams({ sessionId, includeDescendants: 'false' })}`;
const privateBody = 'PRIVATE_HOST_PATH /home/private/secret PRIVATE_TOKEN PRIVATE_LOG';
const content = Buffer.from(mode === 'public-normal'
  ? 'PUBLIC_ROOT_LOG_WITH_ATTACHMENT_REFERENCE\n'
  : 'PRIVATE_ROOT_LOG_WITH_ATTACHMENT_REFERENCE\n');
// Deliberately small fixture generator; it is not an extractor or archive validator.
function zip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name);
    const streamed = entry.streamed ?? false;
    const data = streamed ? deflateRawSync(content, { level: entry.level ?? 6 }) : content;
    const flags = (streamed ? 8 : 0) | (name.length !== entry.name.length ? 2048 : 0);
    const method = streamed ? 8 : 0;
    const crc = crc32(content);
    const size = entry.advertisedSize ?? content.length;
    const local = Buffer.alloc(30 + name.length);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(flags, 6);
    local.writeUInt16LE(method, 8);
    if (!streamed) {
      local.writeUInt32LE(crc, 14);
      local.writeUInt32LE(data.length, 18);
      local.writeUInt32LE(size, 22);
    }
    local.writeUInt16LE(name.length, 26);
    name.copy(local, 30);
    const descriptor = Buffer.alloc(streamed ? 16 : 0);
    if (streamed) {
      descriptor.writeUInt32LE(0x08074b50, 0);
      descriptor.writeUInt32LE(crc, 4);
      descriptor.writeUInt32LE(data.length, 8);
      descriptor.writeUInt32LE(size, 12);
    }
    const central = Buffer.alloc(46 + name.length);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(flags, 8);
    central.writeUInt16LE(method, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(size, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(entry.attributes ?? 0, 38);
    central.writeUInt32LE(offset, 42);
    name.copy(central, 46);
    locals.push(local, data, descriptor);
    centrals.push(central);
    offset += local.length + data.length + descriptor.length;
  }
  const directory = Buffer.concat(centrals);
  const footer = Buffer.alloc(22);
  footer.writeUInt32LE(0x06054b50, 0);
  footer.writeUInt16LE(entries.length, 8);
  footer.writeUInt16LE(entries.length, 10);
  footer.writeUInt32LE(directory.length, 12);
  footer.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, footer]);
}
let entries = [{ name: 'session.v4.jsonl', streamed: mode.startsWith('deflate') || mode === 'bad-descriptor' || mode.startsWith('local-nonzero-') }];
if (mode === 'deflate-level0') entries[0].level = 0;
if (mode === 'multi-entry') entries.push({ name: 'files/test00.bin' }, { name: 'media/fixture.png' });
if (mode === 'unicode-name') entries.push({ name: 'files/fixture/レポート💾.bin', streamed: true });
const unsafeNames = {
  'absolute-name': '/media/fixture.png',
  'backslash-name': 'files/fixture\\private.bin',
  'control-name': 'files/fixture/\u0000private.bin',
  'del-name': 'files/fixture/\u007fprivate.bin',
  'unicode-control-name': 'files/fixture/\u0085private.bin',
  'dot-name': 'files/./private.bin',
  'dotdot-name': 'media/../private.png',
  'empty-component': 'files//private.bin',
  'directory-name': 'media/fixture/',
  'descendant-name': 'subagents/child/session.v4.jsonl',
  'unsupported-name': 'private.bin',
  'duplicate-root': 'session.v4.jsonl',
};
if (mode in unsafeNames) entries.push({ name: unsafeNames[mode] });
if (mode === 'duplicate-attachment') entries.push({ name: 'media/fixture.png' }, { name: 'media/fixture.png' });
if (mode === 'wrong-root-generation') entries[0].name = 'session.v2.jsonl';
if (mode === 'missing-root') entries[0].name = 'media/fixture.png';
if (mode === 'directory-attrs') entries.push({ name: 'media/fixture.png', attributes: 0x10 });
if (mode === 'volume-attrs') entries.push({ name: 'media/fixture.png', attributes: 0x08 });
if (mode === 'symlink-attrs') entries.push({ name: 'media/fixture.png', attributes: (0o120777 << 16) >>> 0 });
if (mode === 'special-attrs') entries.push({ name: 'media/fixture.png', attributes: (0o020600 << 16) >>> 0 });
if (mode === 'regular-attrs') entries.push({ name: 'media/fixture.png', attributes: (0o100600 << 16) >>> 0 });
if (mode === 'advertised-entry-exact' || mode === 'advertised-entry-over') {
  entries[0].streamed = true;
  entries[0].advertisedSize = 64 * 1024 * 1024 + Number(mode.endsWith('over'));
}
if (mode === 'advertised-total-exact' || mode === 'advertised-total-over') {
  entries = Array.from({ length: mode.endsWith('over') ? 5 : 4 }, (_, i) => ({ name: i === 0 ? 'session.v4.jsonl' : `media/fixture${i}.png`, streamed: true, advertisedSize: 64 * 1024 * 1024 }));
}
let archive = zip(entries);
const footerOffset = archive.length - 22;
const directoryOffset = archive.readUInt32LE(footerOffset + 16);
const directoryLength = archive.readUInt32LE(footerOffset + 12);
const compressedLength = archive.readUInt32LE(directoryOffset + 20);
const dataOffset = 30 + Buffer.byteLength(entries[0].name);
switch (mode) {
  case 'bad-footer': archive[footerOffset] = 0; break;
  case 'bad-count': archive.writeUInt16LE(2, footerOffset + 8); archive.writeUInt16LE(2, footerOffset + 10); break;
  case 'bad-directory-size': archive.writeUInt32LE(directoryLength + 1, footerOffset + 12); break;
  case 'bad-directory-offset': archive.writeUInt32LE(0xfffffff0, footerOffset + 16); break;
  case 'bad-local-offset': archive.writeUInt32LE(1, directoryOffset + 42); break;
  case 'bad-local-name': archive[30] ^= 1; break;
  case 'bad-compressed-size': archive.writeUInt32LE(compressedLength + 1, directoryOffset + 20); break;
  case 'bad-descriptor': archive[dataOffset + compressedLength + 4] ^= 1; break;
  case 'local-nonzero-crc': archive.writeUInt32LE(1, 14); break;
  case 'local-nonzero-compressed': archive.writeUInt32LE(1, 18); break;
  case 'local-nonzero-uncompressed': archive.writeUInt32LE(1, 22); break;
  case 'bad-central-header': archive[directoryOffset] = 0; break;
  case 'multi-disk': archive.writeUInt16LE(1, footerOffset + 4); break;
  case 'zip64': archive.writeUInt32LE(0xffffffff, directoryOffset + 20); break;
  case 'encrypted': archive.writeUInt16LE(1, directoryOffset + 8); break;
  case 'bad-method': archive.writeUInt16LE(99, directoryOffset + 10); break;
  case 'bad-version': archive.writeUInt16LE(45, directoryOffset + 6); break;
  case 'bad-local-version': archive.writeUInt16LE(10, 4); break;
  case 'extra-field': archive.writeUInt16LE(1, directoryOffset + 30); break;
  case 'local-extra-field': archive.writeUInt16LE(1, 28); break;
  case 'different-times': archive.writeUInt16LE(1, 10); archive.writeUInt16LE(2, directoryOffset + 12); break;
  case 'invalid-utf8': archive[30] = 0xff; archive[directoryOffset + 46] = 0xff; break;
  case 'unverified-crc': archive.writeUInt32LE(0, 14); archive.writeUInt32LE(0, directoryOffset + 16); break;
  case 'deflate-unverified-content': archive[dataOffset] = 0xff; break;
  case 'prefixed': archive = Buffer.concat([Buffer.from('JUNK'), archive]); break;
  case 'trailing': archive = Buffer.concat([archive, Buffer.from('JUNK')]); break;
  case 'truncated-zip': archive = archive.subarray(0, archive.length - 7); break;
  case 'pk-only': archive = Buffer.from('PK\x03\x04'); break;
  case 'html-as-zip': archive = Buffer.from(`<html>${privateBody}</html>`); break;
  case 'zip-comment': {
    const comment = Buffer.from('opaque PK\x05\x06 comment');
    archive.writeUInt16LE(comment.length, footerOffset + 20);
    archive = Buffer.concat([archive, comment]);
    break;
  }
}
let exportCount = 0;
let rpcCount = 0;
let origin;
let cookie;
const sockets = new Set();
const timers = new Set();
function later(fn, delay) {
  const timer = setTimeout(() => { timers.delete(timer); fn(); }, delay);
  timers.add(timer);
}
const server = http.createServer(async (req, res) => {
  try {
    if (req.url === '/?token=fixture-private-token') {
      assert.equal(req.method, 'GET');
      assert.equal(req.headers.cookie, undefined);
      res.writeHead(303, { location: './', 'set-cookie': `${cookie}; Path=/; HttpOnly; SameSite=Strict; Max-Age=3600; Expires=${new Date(Date.now() + 3600000).toUTCString()}` });
      res.end();
      return;
    }
    assert.equal(req.headers.origin, origin);
    assert.equal(req.headers.cookie, cookie);
    for (const header of ['authorization', 'proxy-authorization', 'x-api-key', 'x-dsh-token', 'x-dsh-auth', 'referer']) assert.equal(req.headers[header], undefined);
    if (req.url === '/api/session/list') {
      assert.equal(req.method, 'POST');
      let bytes = '';
      for await (const chunk of req) bytes += chunk;
      const request = JSON.parse(bytes);
      assert.equal(request.method, 'session/list');
      const index = ++rpcCount;
      console.log(`rpc:${index}`);
      const reply = () => {
        res.writeHead(200, { 'content-type': 'application/json' });
        res.end(JSON.stringify({ type: 'server-response', rpcId: request.rpcId, result: { ok: true, value: { items: [] } } }));
      };
      if (mode === 'rpc-delay') later(reply, 90);
      else if (mode !== 'rpc-hold') reply();
      return;
    }
    // Any redirected fetch, extra authority, model/stop/delete, or other route fails.
    assert.equal(req.url, expectedPath);
    assert.equal(req.method, 'GET');
    for (const header of ['content-type', 'content-length', 'transfer-encoding']) assert.equal(req.headers[header], undefined);
    const index = ++exportCount;
    console.log(`export:${index}`);
    res.on('close', () => console.log(`closed:${index}`));
    const status = mode.startsWith('status-') ? Number(mode.slice(7)) : 200;
    if (status !== 200) {
      res.writeHead(status, { 'content-type': 'text/plain', 'x-private-token': 'PRIVATE_RESPONSE_TOKEN' });
      res.end(privateBody);
      return;
    }
    if (mode === 'redirect') {
      res.writeHead(302, { location: `${origin}/FORBIDDEN_REDIRECT`, 'content-type': 'application/zip' });
      res.end(privateBody);
      return;
    }
    const headers = { 'content-type': mode === 'html-type' ? 'text/html' : mode === 'json-type' ? 'application/json' : mode === 'type-params' ? 'Application/ZIP; irrelevant="value"' : 'application/zip', 'content-disposition': 'attachment; filename="../../PRIVATE_NAME.zip"', 'x-private-token': 'PRIVATE_RESPONSE_TOKEN' };
    if (mode === 'missing-type') delete headers['content-type'];
    if (mode === 'duplicate-type') headers['content-type'] = ['application/zip', 'text/html'];
    if (mode === 'encoded') headers['content-encoding'] = 'gzip';
    if (mode === 'ceiling-known') headers['content-length'] = 32 * 1024 * 1024 + 1;
    else if (mode === 'truncated-http') { headers['content-length'] = archive.length + 4; headers.connection = 'close'; }
    else if (!['chunked', 'deflate-chunked', 'chunked-over', 'ceiling-chunked', 'slow', 'queue-budget', 'hold-first', 'hold-all'].includes(mode)) headers['content-length'] = archive.length;
    res.writeHead(200, headers);
    if (mode === 'ceiling-known') { res.end('short'); return; }
    if (mode === 'ceiling-chunked') {
      const chunk = Buffer.alloc(64 * 1024);
      let sent = 0;
      function pump() {
        while (!res.destroyed && sent <= 32 * 1024 * 1024) {
          sent += chunk.length;
          if (!res.write(chunk)) { res.once('drain', pump); return; }
        }
        if (!res.destroyed) res.end();
      }
      pump(); return;
    }
    if (mode === 'hold-all' || (mode === 'hold-first' && index === 1)) { res.write(archive.subarray(0, 20)); return; }
    if (mode === 'slow' || mode === 'queue-budget') {
      res.write(archive.subarray(0, 20));
      later(() => res.end(archive.subarray(20)), mode === 'slow' ? 500 : 200);
      return;
    }
    if (mode === 'chunked-over') { res.write(Buffer.alloc(32)); later(() => res.end(Buffer.alloc(33)), 10); return; }
    if (mode === 'chunked' || mode === 'deflate-chunked') { res.write(archive.subarray(0, 17)); later(() => res.end(archive.subarray(17)), 10); return; }
    res.end(archive);
  } catch {
    // Fixed fixture failure, never print request credentials or private body.
    process.exitCode = 1;
    res.writeHead(418, { 'content-type': 'text/plain' });
    res.end('fixture wire assertion failed');
  }
});
server.on('connection', socket => { sockets.add(socket); socket.once('close', () => sockets.delete(socket)); });
server.listen(0, '127.0.0.1', () => {
  const authority = `127.0.0.1:${server.address().port}`;
  origin = `http://${authority}`;
  cookie = `dsh-auth-${createHash('sha256').update(authority).digest('base64url')}=fixture-private-cookie`;
  console.log(`${origin}/?token=fixture-private-token`);
  console.log(`length:${archive.length}`);
});
process.stdin.resume();
process.stdin.once('data', () => {
  // Timed-out/cancelled queued exports must never have reached the HTTP route.
  if ((mode === 'rpc-delay' && (exportCount !== 1 || rpcCount !== 3))
      || (mode === 'hold-all' && exportCount !== 1)
      || (mode === 'rpc-hold' && exportCount !== 0)) process.exitCode = 1;
  for (const timer of timers) clearTimeout(timer);
  for (const socket of sockets) socket.destroy();
  server.close(() => process.exit(process.exitCode ?? 0));
});
