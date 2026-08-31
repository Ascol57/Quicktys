<!-- The automations workspace: the full visual editor. -->
<script setup>
import { inject, ref } from "vue";
import AutomationEditor from "../AutomationEditor.vue";

const plugins = inject("plugins", ref([]));
</script>

<template>
  <AutomationEditor class="fill" :plugins="plugins" />
</template>

<style scoped>
.fill {
  height: 100%;
  min-height: 0;
}
</style>
