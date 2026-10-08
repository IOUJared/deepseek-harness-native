import test from 'node:test';
import assert from 'node:assert/strict';
import { discoverSessionAgent, parseForkCommand } from './fork-supervisor.mjs';
function fixture(rows, resolve = id => rows.find(agent => agent.id === id)) {
  const calls = [];
  return { calls, ctx: { get(name) { calls.push(name); assert.equal(name, 'agents'); return { list() { return rows; }, get: resolve }; } } };
}
test('discovery returns the registered object identity, not the queried identity', () => {
  const f = fixture([{ id: 'actual-agent', session: { id: 'selected-session' } }]);
  // Semantic adapter case only: actual alpha registry requires equal Agent/session bytes.
  assert.deepEqual(discoverSessionAgent(f.ctx, 'selected-session'), { sessionId: 'selected-session', agentId: 'actual-agent' });
  assert.deepEqual(f.calls, ['agents']);
});
test('absence is explicit without session preparation, cancellation or other services', () => {
  const f = fixture([{ id: 'other', session: { id: 'other' } }]);
  assert.deepEqual(discoverSessionAgent(f.ctx, 'missing'), { sessionId: 'missing', agentId: null });
  assert.deepEqual(f.calls, ['agents']);
});
test('ambiguous or stale registry observations do not guess a target', () => {
  const rows = [{ id: 'a', session: { id: 'selected' } }, { id: 'b', session: { id: 'selected' } }];
  assert.throws(() => discoverSessionAgent(fixture(rows).ctx, 'selected'), /unavailable/);
  assert.throws(() => discoverSessionAgent(fixture(rows.slice(0, 1), () => ({ ...rows[0] })).ctx, 'selected'), /unavailable/);
});
test('wire parser admits only the closed read and complete UTF8 identifier budget', () => {
  const command = { type: 'discover-session-agent', requestId: 'read-1', sessionId: 'é'.repeat(256) };
  assert.deepEqual(parseForkCommand(JSON.stringify(command)), command);
  for (const id of ['', 'a b', 'a\n', 'a\u0085', '\ud800', 'é'.repeat(257)]) {
    assert.throws(() => parseForkCommand(JSON.stringify({ ...command, sessionId: id })));
  }
  assert.throws(() => parseForkCommand(JSON.stringify({ ...command, agentId: 'guessed' })));
  assert.throws(() => parseForkCommand(JSON.stringify({ ...command, method: 'resume' })));
});
