// Structured-clone-safe payloads for the iframe bridges.
//
// `postMessage` serialises with the structured clone algorithm, which throws
// `DataCloneError` on a **Proxy** — and every object handed to us by Vue
// (`props.config`, `props.context`, anything out of a store) is a reactive
// proxy. WebKit, which is what Tauri uses on Linux and macOS, rejects those
// outright, so the widget never received its settings.
//
// `toRaw()` is not enough: it unwraps the top level only, leaving nested
// proxies behind. The bridges are JSON by contract, so a JSON round-trip is
// both exact and total — it also drops functions and undefined, which could
// not cross the boundary anyway.
export function plain(value) {
  if (value === null || typeof value !== "object") return value;
  try {
    return JSON.parse(JSON.stringify(value));
  } catch {
    // Cyclic or otherwise unserialisable: send nothing rather than throw and
    // break the whole message.
    return Array.isArray(value) ? [] : {};
  }
}
