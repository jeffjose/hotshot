<script lang="ts">
  import CaptureMenu from "./CaptureMenu.svelte";
  import { PanelRight } from "lucide-svelte";
  import { sidebarOpen } from "$lib/stores/gallery";
</script>

<div class="h-14 border-b border-border flex items-center px-3 gap-2 shrink-0 bg-card">
  <CaptureMenu />

  <div class="flex-1" />

  <button
    class="p-2 rounded-md hover:bg-secondary text-muted-foreground hover:text-foreground transition-colors"
    onclick={() => ($sidebarOpen = !$sidebarOpen)}
    title="Toggle gallery"
  >
    <PanelRight size={18} />
  </button>
</div>
