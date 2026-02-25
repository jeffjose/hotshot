<script lang="ts">
  import { Monitor, Square, AppWindow, ChevronDown } from "lucide-svelte";
  import { captureFullscreen, captureRegion, captureWindow, listMonitors, imageUrl } from "$lib/api";
  import { currentScreenshot, currentImageSrc, isLoading } from "$lib/stores/screenshot";
  import { screenshots } from "$lib/stores/gallery";
  import { listScreenshots } from "$lib/api";
  import type { Monitor as MonitorType } from "$lib/types";

  let open = $state(false);
  let monitors = $state<MonitorType[]>([]);
  let menuEl: HTMLDivElement;

  async function loadMonitors() {
    try {
      monitors = await listMonitors();
    } catch {
      monitors = [];
    }
  }

  function handleMouseEnter() {
    loadMonitors();
    open = true;
  }

  function handleMouseLeave(e: MouseEvent) {
    const related = e.relatedTarget as Node | null;
    if (menuEl && related && menuEl.contains(related)) return;
    open = false;
  }

  async function doCapture(fn: () => Promise<import("$lib/types").Metadata>) {
    open = false;
    $isLoading = true;
    try {
      const meta = await fn();
      $currentScreenshot = meta;
      $currentImageSrc = imageUrl(meta.path);
      // Refresh gallery
      $screenshots = await listScreenshots();
    } catch (e) {
      console.error("Capture failed:", e);
    } finally {
      $isLoading = false;
    }
  }
</script>

<div
  class="relative"
  bind:this={menuEl}
  onmouseenter={handleMouseEnter}
  onmouseleave={handleMouseLeave}
  role="menu"
  tabindex="-1"
>
  <button
    class="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium
           hover:bg-secondary text-foreground transition-colors"
  >
    <Monitor size={16} />
    Capture
    <ChevronDown size={14} />
  </button>

  {#if open}
    <div
      class="absolute top-full left-0 mt-1 w-56 rounded-md border border-border
             bg-popover p-1 shadow-lg z-50"
    >
      <button
        class="flex items-center gap-2 w-full px-3 py-2 text-sm rounded-sm
               hover:bg-accent text-popover-foreground transition-colors text-left"
        onclick={() => doCapture(() => captureFullscreen())}
      >
        <Monitor size={16} />
        Fullscreen
      </button>

      {#each monitors as monitor, i}
        <button
          class="flex items-center gap-2 w-full px-3 py-2 text-sm rounded-sm
                 hover:bg-accent text-popover-foreground transition-colors text-left pl-8"
          onclick={() => doCapture(() => captureFullscreen(String(i)))}
        >
          {monitor.name} ({monitor.width}x{monitor.height})
        </button>
      {/each}

      <button
        class="flex items-center gap-2 w-full px-3 py-2 text-sm rounded-sm
               hover:bg-accent text-popover-foreground transition-colors text-left"
        onclick={() => doCapture(() => captureRegion())}
      >
        <Square size={16} />
        Region
      </button>

      <button
        class="flex items-center gap-2 w-full px-3 py-2 text-sm rounded-sm
               hover:bg-accent text-popover-foreground transition-colors text-left"
        onclick={() => doCapture(() => captureWindow())}
      >
        <AppWindow size={16} />
        Window
      </button>
    </div>
  {/if}
</div>
