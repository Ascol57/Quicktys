// Translated display names for extensions and their actions/triggers/state.
// Plugin lang is namespaced `plugin.<id>.*` by the host; these resolve the i18n
// key and fall back to the manifest name / raw id when a translation is missing.

/** Plugin display name — `plugin` is a PluginInfo (`{ id, name }`). */
export function pluginLabel(plugin, t, te) {
  if (!plugin) return "?";
  const key = `plugin.${plugin.id}.name`;
  return te(key) ? t(key) : (plugin.name ?? plugin.id);
}

/** Action display name for `pluginId.action`. */
export function actionLabel(pluginId, action, t, te) {
  if (!action) return "";
  const key = `plugin.${pluginId}.actions.${action}.name`;
  return te(key) ? t(key) : action;
}

/** Trigger display name for `pluginId.trigger`. */
export function triggerLabel(pluginId, trigger, t, te) {
  if (!trigger) return "";
  const key = `plugin.${pluginId}.triggers.${trigger}.name`;
  return te(key) ? t(key) : trigger;
}
