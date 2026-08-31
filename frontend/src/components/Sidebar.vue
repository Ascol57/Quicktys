<!-- Left navigation rail. Leaf entries open a workspace tab; the Profiles folder
     expands to list profiles (create with +, click one to open its tab). -->
<script setup>
import { inject, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { NAV } from "../nav.js";
import { useProfiles } from "../stores/profiles.js";

defineProps({ active: { type: String, default: null } });
const emit = defineEmits(["select"]);
const { t, te } = useI18n();

// Tabs contributed by extensions (modules declaring a `view`) — rendered as
// leaf entries after the built-in sections.
const views = inject("contributed-views", ref([]));
const viewLabel = (v) => (v.label && te(v.label) ? t(v.label) : v.name || v.id);

const store = useProfiles();
onMounted(() => store.load());

const expanded = ref({ profiles: true });
const toggle = (id) => (expanded.value[id] = !expanded.value[id]);

async function createProfile() {
  expanded.value.profiles = true;
  const id = await store.create();
  emit("select", `profile:${id}`);
}
</script>

<template>
  <nav class="sidebar">
    <template v-for="item in NAV" :key="item.id">
      <!-- Folder (Profiles): expandable list of resources -->
      <template v-if="item.folder">
        <div class="folder">
          <button class="nav folder__head" @click="toggle(item.id)">
            <i class="chev pi" :class="expanded[item.id] ? 'pi-chevron-down' : 'pi-chevron-right'" />
            <i :class="item.icon" />
            <span class="nav__label">{{ t(item.labelKey) }}</span>
            <i class="folder__add pi pi-plus" :title="t('core.profiles.new')" @click.stop="createProfile" />
          </button>
          <template v-if="expanded[item.id]">
            <button
              v-for="p in store.list"
              :key="p.id"
              class="nav child"
              :class="{ active: active === `profile:${p.id}` }"
              @click="emit('select', `profile:${p.id}`)"
            >
              <i class="pi pi-file" />
              <span class="nav__label">{{ p.name || p.id }}</span>
            </button>
            <div v-if="!store.list.length" class="empty">{{ t("core.profiles.none") }}</div>
          </template>
        </div>
      </template>

      <!-- Leaf entry -->
      <button
        v-else
        class="nav"
        :class="{ active: item.id === active }"
        @click="emit('select', item.id)"
      >
        <i :class="item.icon" />
        <span class="nav__label">{{ t(item.labelKey) }}</span>
      </button>
    </template>

    <!-- Extension-contributed tabs (e.g. the webview overlay editor) -->
    <button
      v-for="v in views"
      :key="v.id"
      class="nav"
      :class="{ active: active === `view:${v.id}` }"
      @click="emit('select', `view:${v.id}`)"
    >
      <i :class="v.icon || 'pi pi-circle'" />
      <span class="nav__label">{{ viewLabel(v) }}</span>
    </button>
  </nav>
</template>

<style scoped>
.sidebar {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 0.5rem 0.4rem;
  gap: 0.1rem;
  overflow-y: auto;
}
.nav {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  width: 100%;
  padding: 0.55rem 0.7rem;
  background: transparent;
  border: none;
  border-radius: 7px;
  color: #cfcfcf;
  cursor: pointer;
  font-size: 0.92rem;
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
}
.nav i {
  font-size: 1.02rem;
  flex: 0 0 auto;
}
.nav__label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
}
.nav:hover {
  background: #222;
  color: #fff;
}
.nav.active {
  background: #2a2a2a;
  color: #fff;
  box-shadow: inset 3px 0 0 #7ec27e;
}
.chev {
  font-size: 0.7rem !important;
  opacity: 0.6;
}
.folder__head {
  font-weight: 600;
}
.folder__add {
  font-size: 0.75rem !important;
  opacity: 0;
  padding: 3px;
  border-radius: 4px;
}
.folder__head:hover .folder__add {
  opacity: 0.7;
}
.folder__add:hover {
  opacity: 1 !important;
  background: #363636;
}
.child {
  padding-left: 2rem;
  font-size: 0.86rem;
  color: #b6b6b6;
}
.child i {
  font-size: 0.85rem;
  opacity: 0.7;
}
.child.active {
  color: #fff;
}
.empty {
  padding: 0.4rem 2rem;
  font-size: 0.78rem;
  opacity: 0.4;
}
</style>
