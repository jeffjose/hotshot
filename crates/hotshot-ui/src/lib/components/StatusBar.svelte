<script lang="ts">
  import { currentScreenshot } from "$lib/stores/screenshot";
  import { formatFileSize } from "$lib/utils";
</script>

<div class="h-7 border-t border-border flex items-center px-3 gap-4 text-xs text-muted-foreground shrink-0 bg-card">
  {#if $currentScreenshot}
    <span>{$currentScreenshot.width} &times; {$currentScreenshot.height}</span>
    <span>{$currentScreenshot.format.toUpperCase()}</span>
    <span>{formatFileSize($currentScreenshot.file_size)}</span>
    <span class="text-muted-foreground/60">{$currentScreenshot.capture_mode}</span>
  {:else}
    <span>No image loaded</span>
  {/if}
</div>
