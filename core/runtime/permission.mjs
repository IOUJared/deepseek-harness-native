/** Fixed native permission-preset owner: reads the Host catalog, switches a preset.
 *  No credential, socket or arbitrary Host payload ever crosses this boundary. */
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
/** Preset names are the Host's own keys: bounded, printable, never control characters. */
export const validPermissionName = value => typeof value === 'string'
  && value.length > 0
  && [...value].length <= 128
  && Buffer.byteLength(value) <= 512
  && !/[\p{Cc}\p{Cs}\p{Cf}\p{Zl}\p{Zp}]/u.test(value);
/** Sessions are opaque identifiers; the bridge never interprets one. */
export const validPermissionSession = value => validPermissionName(value);
const MAX_OPTIONS = 64;
const MAX_OPTION_BYTES = 8192;
const boundedText = (value, limit) => typeof value === 'string'
  && [...value].length <= limit
  && Buffer.byteLength(value) <= 4096
  && !/[\p{Cc}\p{Cs}\p{Cf}\p{Zl}\p{Zp}]/u.test(value);
/** One `PresetOption`, reduced to the three fields a client renders. */
const boundedOption = value => {
  if (!object(value) || !validPermissionName(value.value)) return undefined;
  const option = { value: value.value, name: boundedText(value.name, 256) ? value.name : value.value };
  if (boundedText(value.description, 1024)) option.description = value.description;
  return Buffer.byteLength(JSON.stringify(option)) <= MAX_OPTION_BYTES ? option : undefined;
};
/** `PermissionCatalog` as the Host declares it: `{ options, defaultOptions, defaultPreset }`. */
export const boundedPermissionCatalog = value => {
  if (!object(value) || !Array.isArray(value.options)) return undefined;
  const options = value.options.slice(0, MAX_OPTIONS).map(boundedOption);
  if (options.length === 0 || options.some(option => option === undefined)) return undefined;
  const defaults = Array.isArray(value.defaultOptions) ? value.defaultOptions : [];
  const defaultOptions = defaults.slice(0, MAX_OPTIONS).map(boundedOption).filter(option => option !== undefined);
  const defaultPreset = validPermissionName(value.defaultPreset) ? value.defaultPreset : options[0].value;
  return { options, defaultOptions, defaultPreset };
};
const failure = (code, message) => ({ ok: false, error: { code, message } });
const unavailable = () => failure('unavailable', 'permission presets are unavailable');
/** One bridge per Core process; `ctx.get` resolves the Host's own service. */
export const createPermissionBridge = ({ ctx }) => {
  const service = () => {
    const value = ctx.get('permissionPresets');
    return object(value) && typeof value.catalog === 'function' && typeof value.set === 'function'
      ? value
      : undefined;
  };
  return {
    handle: async command => {
      switch (object(command) ? command.type : undefined) {
        case 'permission-catalog': {
          const presets = service();
          if (presets === undefined) return unavailable();
          try {
            const catalog = boundedPermissionCatalog(await presets.catalog());
            return catalog === undefined ? failure('invalid', 'permission catalog is malformed') : { ok: true, catalog };
          } catch {
            return unavailable();
          }
        }
        case 'permission-set': {
          if (!validPermissionSession(command.session) || !validPermissionName(command.name)) {
            return failure('invalid', 'permission set needs a bounded session and preset');
          }
          const presets = service();
          if (presets === undefined) return unavailable();
          try {
            // `set` records the change and updates each changed knob: unknown names throw.
            await presets.set(command.session, command.name);
            return { ok: true };
          } catch {
            return failure('invalid', 'permission preset was refused');
          }
        }
        default:
          return unavailable();
      }
    },
  };
};
