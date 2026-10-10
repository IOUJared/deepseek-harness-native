// Pure public-alpha PiAiProviderProfile subset; no Settings or provider execution.
const fields = Object.freeze([
  'transport', 'cacheRetention', 'timeoutMs', 'websocketConnectTimeoutMs',
  'streamIdleTimeoutMs', 'requestImagePixelBudget', 'requestImageMaxBytes',
  'maxRequestImageBytes',
]);
const transports = new Set(['sse', 'websocket', 'websocket-cached', 'auto']);
const cacheRetentions = new Set(['none', 'short', 'long']);
const errorCodes = new Set([
  'CODEX_RUNTIME_UNAVAILABLE', 'CODEX_RUNTIME_CONFLICT', 'CODEX_RUNTIME_INDETERMINATE',
]);

/** Return a frozen diagnostic with a closed code; never retain arbitrary prose. */
export function runtimeUnavailable(code = 'CODEX_RUNTIME_UNAVAILABLE') {
  const safeCode = typeof code === 'string' && errorCodes.has(code)
    ? code : 'CODEX_RUNTIME_UNAVAILABLE';
  const error = new Error(safeCode);
  error.name = 'CodexRuntimeError';
  error.code = safeCode;
  return Object.freeze(error);
}

function fail() {
  throw runtimeUnavailable();
}

function record(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) fail();
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) fail();
}

function fieldValue(field, value) {
  if (value === null) return null;
  if (field === 'transport' ? transports.has(value)
    : field === 'cacheRetention' ? cacheRetentions.has(value)
      : ['timeoutMs', 'websocketConnectTimeoutMs'].includes(field) ? Number.isSafeInteger(value) && value >= 0
        : field === 'streamIdleTimeoutMs' ? Number.isFinite(value) && value > 0 && value <= 2147483647
          : Number.isSafeInteger(value) && value > 0) return value === 0 ? 0 : value;
  fail();
}

function dataValue(descriptor) {
  if (!Object.hasOwn(descriptor, 'value') || !descriptor.enumerable) fail();
  return descriptor.value;
}

/** Capture exactly eight own data fields; null inherits, with no coercion. */
export function validateCodexRuntimeValues(input) {
  try {
    record(input);
    const keys = Reflect.ownKeys(input);
    if (keys.length !== fields.length || keys.some((key) => !fields.includes(key))) fail();
    const result = {};
    for (const field of fields) {
      const descriptor = Object.getOwnPropertyDescriptor(input, field);
      if (!descriptor) fail();
      result[field] = fieldValue(field, dataValue(descriptor));
    }
    return Object.freeze(result);
  } catch {
    // Contain reflection failures without retaining input or thrown external text.
    fail();
  }
}

/** Pick only known own fields from a trusted redacted profile; absence inherits. */
export function projectCodexRuntimeValues(profile) {
  try {
    if (profile !== undefined) record(profile);
    const result = {};
    for (const field of fields) {
      const descriptor = profile === undefined
        ? undefined : Object.getOwnPropertyDescriptor(profile, field);
      result[field] = descriptor ? fieldValue(field, dataValue(descriptor)) : null;
    }
    return Object.freeze(result);
  } catch {
    // Extra profile fields are never read, copied or serialized.
    fail();
  }
}

/** Project inherited values, materializing defaults from the exact public Config schema. */
export function projectCodexRuntimeBaseValues(schema, profile) {
  try {
    const inherited = projectCodexRuntimeValues(profile);
    const root = schema.refs[schema.uid];
    const providers = schema.refs[root.dict.providers];
    const provider = schema.refs[providers.inner];
    if (root.type !== 'object' || providers.type !== 'dict' || provider.type !== 'object') fail();
    const result = { ...inherited };
    for (const field of fields) {
      const definition = schema.refs[provider.dict[field]];
      if (!definition) fail();
      if (result[field] === null && definition.meta && Object.hasOwn(definition.meta, 'default')) {
        result[field] = fieldValue(field, definition.meta.default);
      }
    }
    return Object.freeze(result);
  } catch (_error) { throw runtimeUnavailable(); }
}

/** Return eight immutable fixed-provider set/unset operations; no caller paths. */
export function codexRuntimePathOps(values) {
  const captured = validateCodexRuntimeValues(values);
  return Object.freeze(fields.map((field) => {
    const path = Object.freeze(['providers', 'openai-codex', field]);
    return Object.freeze(captured[field] === null
      ? { op: 'unset', path }
      : { op: 'set', path, value: captured[field] });
  }));
}

/** Revisions are numbers in [0, Number.MAX_SAFE_INTEGER], without coercion. */
export function validCodexRuntimeRevision(value) {
  return Number.isSafeInteger(value) && value >= 0;
}
