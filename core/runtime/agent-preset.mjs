/** Fixed native agent-preset roster owner: reads the Host's own selection roster.
 *  No credential, socket or arbitrary Host payload ever crosses this boundary. */
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const boundedText = (value, limit) => typeof value === 'string'
  && value.length > 0
  && [...value].length <= limit
  && Buffer.byteLength(value) <= 4096
  && !/[\p{Cc}\p{Cs}\p{Cf}\p{Zl}\p{Zp}]/u.test(value);
/** Preset ids are the Host's own keys: bounded, printable, never control characters. */
export const validPresetId = value => boundedText(value, 128);
const MAX_PRESETS = 64;
const boundedOption = (value, limit) => (boundedText(value, limit) ? value : undefined);
/** One `AgentPresetRow`, reduced to the fields a client renders. A `broken` row is a
 *  declared preset whose activation failed, and the reference keeps it visible. */
const boundedRow = value => {
  if (!object(value) || !validPresetId(value.id)) return undefined;
  const row = { id: value.id, isDefault: value.isDefault === true };
  const name = boundedOption(value.name, 256);
  if (name !== undefined) row.name = name;
  const description = boundedOption(value.description, 1024);
  if (description !== undefined) row.description = description;
  const broken = boundedOption(value.broken, 512);
  if (broken !== undefined) row.broken = broken;
  return Buffer.byteLength(JSON.stringify(row)) <= 4096 ? row : undefined;
};
/** `AgentPresetRoster` as the Host declares it: `{ presets: AgentPresetRow[] }`
 *  (`api-catalog.ts:4605-4611`). */
export const boundedRoster = value => {
  if (!object(value) || !Array.isArray(value.presets)) return undefined;
  const presets = value.presets.slice(0, MAX_PRESETS).map(boundedRow);
  if (presets.length === 0 || presets.some(preset => preset === undefined)) return undefined;
  if (!presets.some(preset => preset.isDefault)) return undefined;
  return { presets };
};
const failure = (code, message) => ({ ok: false, error: { code, message } });
const unavailable = () => failure('unavailable', 'agent presets are unavailable');
/** One bridge per Core process; `ctx.get` resolves the Host's own registry. */
export const createAgentPresetBridge = ({ ctx }) => ({
  handle: async command => {
    const service = ctx.get('agentPresets');
    if (!object(service) || typeof service.remoteExportList !== 'function') return unavailable();
    if (!object(command) || command.type !== 'agent-presets') return unavailable();
    try {
      const roster = boundedRoster(await service.remoteExportList());
      return roster === undefined ? failure('invalid', 'agent roster is malformed') : { ok: true, roster };
    } catch {
      return unavailable();
    }
  },
});
