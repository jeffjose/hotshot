<script lang="ts">
  import { screenshots, sidebarOpen } from "$lib/stores/gallery";
  import { currentScreenshot, currentImageSrc } from "$lib/stores/screenshot";
  import { imageUrl } from "$lib/api";
  import { formatFileSize } from "$lib/utils";
  import type { Metadata } from "$lib/types";

  function selectScreenshot(meta: Metadata) {
    $currentScreenshot = meta;
    $currentImageSrc = imageUrl(meta.path);
  }

  function formatTime(ts: string): string {
    const d = new Date(ts);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

{#if $sidebarOpen}
  <div class="w-70 border-l border-border bg-card flex flex-col shrink-0 overflow-hidden">
    <div class="px-3 py-2 border-b border-border text-xs font-medium text-muted-foreground uppercase tracking-wider">
      Screenshots ({$screenshots.length})
    </div>
    <div class="flex-1 overflow-y-auto">
      {#each $screenshots as meta (meta.id)}
        <button
          class="w-full text-left px-2 py-2 hover:bg-accent transition-colors border-b border-border/50
                 {$currentScreenshot?.id === meta.id ? 'bg-accent' : ''}"
          onclick={() => selectScreenshot(meta)}
        >
          <div class="aspect-video bg-background rounded overflow-hidden mb-1">
            <img
              src={imageUrl(meta.path)}
              alt={meta.id}
              class="w-full h-full object-cover"
              loading="lazy"
            />
          </div>
          <div class="text-xs text-muted-foreground flex justify-between">
            <span>{formatTime(meta.timestamp)}</span>
            <span>{meta.width}x{meta.height}</span>
          </div>
          <div class="text-xs text-muted-foreground">
            {meta.format.toUpperCase()} &middot; {formatFileSize(meta.file_size)}
          </div>
        </button>
      {/each}
      {#if $screenshots.length === 0}
        <div class="p-4 text-sm text-muted-foreground text-center">
          No screenshots yet
        </div>
      {/if}
    </div>
  </div>
{/if}
