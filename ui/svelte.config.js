import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

/** @type {import("@sveltejs/vite-plugin-svelte").SvelteConfig} */
export default {
  preprocess: vitePreprocess(),
  onwarn(warning, handler) {
    if (warning.code === 'a11y_label_has_associated_control') return;
    if (warning.code === 'a11y_no_static_element_interactions') return;
    if (warning.code === 'a11y_click_events_have_key_events') return;
    if (warning.code === 'a11y_no_noninteractive_element_interactions') return;
    handler(warning);
  },
}
