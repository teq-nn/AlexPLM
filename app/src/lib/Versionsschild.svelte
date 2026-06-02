<script lang="ts">
  // Versionsschild (Issue #105): names the Werkbank app's own version. This is the *software's*
  // version — deliberately NOT the Produkt-Versionen (Stand/Revision) that the VersionBar/
  // VersionTree show. It reads from the Tauri config (single source of truth: tauri.conf.json)
  // via getVersion(), so it never drifts from what was bundled. Rendered as a plain inline atom
  // that lives in the Fussleiste; the footer owns its placement, not this component.
  import { getVersion } from "@tauri-apps/api/app";
  import { onMount } from "svelte";

  let version = $state<string | null>(null);

  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      // Running outside the Tauri runtime (e.g. plain `vite dev`): no chassis stamp to show.
      version = null;
    }
  });
</script>

{#if version}
  <span class="versionsschild" title="Werkbank-Version">Werkbank v{version}</span>
{/if}

<style>
  .versionsschild {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.02em;
    color: var(--ink-muted);
    user-select: none;
  }
</style>
