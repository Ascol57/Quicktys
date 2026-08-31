// The left-sidebar sections. Leaf items open a singleton section tab; a `folder`
// item (Profiles) expands to list its resources, each opening its own tab.
export const NAV = [
  { id: "dashboard", icon: "pi pi-th-large", labelKey: "core.menu.dashboard", closable: false },
  { id: "profiles", icon: "pi pi-sitemap", labelKey: "core.menu.profiles", folder: true },
  { id: "automations", icon: "pi pi-bolt", labelKey: "core.menu.automations" },
  { id: "variables", icon: "pi pi-database", labelKey: "core.menu.variables" },
  { id: "marketplace", icon: "pi pi-shopping-bag", labelKey: "core.menu.marketplace" },
  { id: "settings", icon: "pi pi-cog", labelKey: "core.menu.settings" },
];

export const navItem = (id) => NAV.find((n) => n.id === id);

// Tab descriptor for a dynamic (per-resource) tab id like "profile:main".
export function tabInfo(id) {
  if (id.startsWith("profile:")) {
    return { icon: "pi pi-sitemap", closable: true, profileId: id.slice("profile:".length) };
  }
  const item = navItem(id);
  return item ? { icon: item.icon, labelKey: item.labelKey, closable: item.closable !== false } : null;
}
