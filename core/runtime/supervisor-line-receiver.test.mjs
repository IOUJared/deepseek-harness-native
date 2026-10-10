import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { StringDecoder } from 'node:string_decoder';
import { test } from 'node:test';
import { lineReceiver, interceptDiagnostics, MAX_COMMAND_BYTES } from './supervisor.mjs';

// Exact retained pre-optimization implementation: independent output/partition oracle.
function original(onLine, onOversize, limit = MAX_COMMAND_BYTES) {
  const decoder = new StringDecoder('utf8');
  let pending = '';
  let dropping = false;
  const feedText = text => {
    for (const part of text.split(/(?<=\n)/)) {
      if (!part) continue;
      const ended = part.endsWith('\n');
      if (!dropping) {
        if (Buffer.byteLength(pending) + Buffer.byteLength(part) > limit) {
          pending = '';
          dropping = true;
          onOversize();
        } else pending += part;
      }
      if (ended) {
        if (!dropping) onLine(pending.replace(/\r?\n$/, ''));
        pending = '';
        dropping = false;
      }
    }
  };
  return {
    write(chunk) { feedText(decoder.write(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk))); },
    end() {
      feedText(decoder.end());
      // Commands require newline framing; diagnostics may flush their final bounded line.
      return !dropping && pending ? pending : undefined;
    },
  };
}

function partitions(bytes, width = 1, seed) {
  const chunks = [];
  for (let at = 0; at < bytes.length;) {
    if (seed !== undefined) seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
    const next = Math.min(bytes.length, at + (seed === undefined ? width : 1 + seed % width));
    chunks.push(bytes.subarray(at, next));
    at = next;
  }
  return chunks;
}
function collect(factory, chunks, limit, mode = 'normal') {
  const events = [];
  let lineCalls = 0, oversizeCalls = 0, receiver;
  receiver = factory(line => {
    events.push(['line', line]);
    lineCalls++;
    if (mode === 'throw-line' && lineCalls === 1) throw new Error('PUBLIC line callback');
    if (mode === 'reenter-line' && lineCalls === 1) receiver.write('B\n');
    return false;
  }, () => {
    events.push(['oversize']);
    oversizeCalls++;
    if (mode === 'throw-oversize' && oversizeCalls === 1) throw new Error('PUBLIC oversize callback');
    if (mode === 'reenter-oversize' && oversizeCalls === 1) receiver.write('\nB\n');
    return { ignored: true };
  }, limit);
  for (const chunk of chunks) {
    try { assert.equal(receiver.write(chunk), undefined); }
    catch (cause) { events.push(['throw', cause.message]); }
  }
  const tails = [];
  for (let i = 0; i < 2; i++) {
    try { tails.push(receiver.end()); } catch (cause) { tails.push(['throw', cause.message]); }
  }
  return { events, tails };
}

test('whole, one-byte and deterministic random byte partitions match retained production oracle', () => {
  const inputs = [Buffer.alloc(0), ...['\n', '\r\n', 'A\nB\n', 'A\r\nB\rtail', '🌹é秘密\nOK\r\n尾', 'unframed tail', '\n\nend'].map(value => Buffer.from(value)),
    Buffer.from([0xff, 0xfe, 0x0a, 0xf0, 0x9f, 0x8c, 0xb9, 0x0a, 0xe2, 0x82]),
    Buffer.from([0xed, 0xa0, 0x80, 0x0a, 0xe2, 0x28, 0xa1, 0x0a, 0xf4, 0x90, 0x80, 0x80]),
  ];
  for (const bytes of inputs) for (const limit of [0, 1, 2, 3, 4, 5, 8, 12, 64]) {
    for (const chunks of [[bytes], partitions(bytes), ...[1, 7, 31, 123456789].map(seed => partitions(bytes, 9, seed))]) {
      assert.deepEqual(collect(lineReceiver, chunks, limit), collect(original, chunks, limit), `limit=${limit}, bytes=${bytes.toString('hex')}`);
    }
  }
});

test('limits count decoded bytes including LF/CRLF, not original invalid byte count', () => {
  assert.deepEqual(collect(lineReceiver, ['abc\n'], 4), { events: [['line', 'abc']], tails: [undefined, undefined] });
  assert.deepEqual(collect(lineReceiver, ['abc\r\n'], 4), { events: [['oversize']], tails: [undefined, undefined] });
  assert.deepEqual(collect(lineReceiver, [Buffer.from([0xff]), '\n'], 4).events, [['line', '\ufffd']]);
  assert.deepEqual(collect(lineReceiver, [Buffer.from([0xff]), '\n'], 3).events, [['oversize']]);
  const emoji = Buffer.from('🌹\n');
  assert.deepEqual(collect(lineReceiver, partitions(emoji), 5).events, [['line', '🌹']]);
  assert.deepEqual(collect(lineReceiver, partitions(emoji), 4).events, [['oversize']]);
});

test('oversized complete line drops once and valid following line recovers across partitions', () => {
  const bytes = Buffer.from('SECRET-should-not-leak\r\nOK\n');
  for (const chunks of [[bytes], partitions(bytes), partitions(bytes, 5, 42)]) {
    const result = collect(lineReceiver, chunks, 8);
    assert.deepEqual(result, collect(original, chunks, 8));
    assert.deepEqual(result.events, [['oversize'], ['line', 'OK']]);
  }
});

test('end preserves bounded unframed tails and decoder replacement-byte overflow', () => {
  assert.deepEqual(collect(lineReceiver, ['tail'], 4), { events: [], tails: ['tail', 'tail'] });
  const incomplete = Buffer.from([0xf0, 0x9f]);
  assert.deepEqual(collect(lineReceiver, [incomplete], 3), { events: [], tails: ['\ufffd', '\ufffd'] });
  assert.deepEqual(collect(lineReceiver, [incomplete], 2), { events: [['oversize']], tails: [undefined, undefined] });
  assert.deepEqual(collect(lineReceiver, ['too long'], 4), { events: [['oversize']], tails: [undefined, undefined] });
});

test('strings, Uint8Arrays, callback returns, throws and reentrant callbacks retain original outcomes', () => {
  for (const mode of ['normal', 'throw-line', 'throw-oversize', 'reenter-line', 'reenter-oversize']) {
    for (const chunks of [['A\n', 'second\n', '\nB\n'], [new Uint8Array(Buffer.from('A\n')), 'too-long\n', '\nOK\n']]) {
      assert.deepEqual(collect(lineReceiver, chunks, 4, mode), collect(original, chunks, 4, mode), mode);
    }
  }
});

test('64KiB exact/default boundary and over-limit recovery match for fragmented ASCII and Unicode', () => {
  const inputs = [Buffer.from('x'.repeat(MAX_COMMAND_BYTES - 1) + '\nOK\n'),
    Buffer.from('x'.repeat(MAX_COMMAND_BYTES) + '\nOK\n'),
    Buffer.from('🌹'.repeat(16383) + 'abc\nOK\n')];
  for (const bytes of inputs) for (const chunks of [[bytes], partitions(bytes), partitions(bytes, 257), partitions(bytes, 1024, 19)]) {
    assert.deepEqual(collect(lineReceiver, chunks, undefined), collect(original, chunks, undefined));
  }
});

test('diagnostics still redact complete split secrets before truncation and suppress oversized lines', () => {
  const stream = () => Object.assign(new EventEmitter(), { writableLength: 0, writes: [], write(chunk) { this.writes.push(String(chunk)); return true; } });
  const output = stream(), diagnostic = stream();
  const secret = 'PUBLIC-sensitive-secret-🌹';
  const adapter = interceptDiagnostics(output, diagnostic, { PUBLIC_SECRET: secret });
  try {
    for (const chunk of partitions(Buffer.from(`token=${secret}\n`))) output.write(chunk);
    output.write('x'.repeat(4097) + '\nOK\n');
    output.write(`Bearer ${secret}`);
  } finally { adapter.restore(); }
  const emitted = diagnostic.writes.join('');
  assert.ok(!emitted.includes(secret));
  assert.ok(emitted.includes('[REDACTED]'));
  assert.ok(emitted.includes('oversized diagnostic suppressed'));
  assert.ok(emitted.includes('[harness] OK\n'));
  assert.deepEqual(output.writes, []); // No raw diagnostic text enters protocol stdout.
});
