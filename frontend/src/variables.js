// Build the catalogue of `@plugin.variable` template variables from the loaded
// plugins' declared reactive state, with translated names.
export function buildVariables(plugins, t, te) {
  const out = [];
  for (const p of plugins ?? []) {
    for (const s of p.state ?? []) {
      const id = `${p.id}.${s}`;
      const key = `plugin.${p.id}.state.${s}.name`;
      out.push({
        id,
        name: te(key) ? t(key) : s,
        plugin: p.name,
        icon: p.icon || "pi pi-box",
        color: p.color || "#7ec27e",
      });
    }
  }
  return out;
}
