import DefaultTheme from "vitepress/theme";
import Screenshot from "./Screenshot.vue";
import "./custom.css";

export default {
  extends: DefaultTheme,
  enhanceApp({ app }) {
    // Available in every page without an import.
    app.component("Screenshot", Screenshot);
  },
};
