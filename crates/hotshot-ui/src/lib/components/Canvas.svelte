<script lang="ts">
  import { currentScreenshot, currentImageSrc } from "$lib/stores/screenshot";

  let containerEl: HTMLDivElement;
  let containerWidth = $state(0);
  let containerHeight = $state(0);

  let imageEl: HTMLImageElement | null = $state(null);
  let imageNatW = $state(0);
  let imageNatH = $state(0);
  let imageLoaded = $state(false);

  let scale = $derived.by(() => {
    if (!imageNatW || !imageNatH || !containerWidth || !containerHeight) return 1;
    const sx = containerWidth / imageNatW;
    const sy = containerHeight / imageNatH;
    return Math.min(sx, sy, 1);
  });

  let displayW = $derived(Math.round(imageNatW * scale));
  let displayH = $derived(Math.round(imageNatH * scale));

  $effect(() => {
    if (!containerEl) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        containerWidth = entry.contentRect.width;
        containerHeight = entry.contentRect.height;
      }
    });
    ro.observe(containerEl);
    return () => ro.disconnect();
  });

  $effect(() => {
    const src = $currentImageSrc;
    if (!src) {
      imageLoaded = false;
      return;
    }
    const img = new Image();
    img.onload = () => {
      imageNatW = img.naturalWidth;
      imageNatH = img.naturalHeight;
      imageEl = img;
      imageLoaded = true;
    };
    img.src = src;
  });
</script>

<div
  bind:this={containerEl}
  class="flex-1 flex items-center justify-center bg-background overflow-hidden"
>
  {#if imageLoaded && $currentScreenshot}
    <img
      src={$currentImageSrc}
      alt="Screenshot"
      width={displayW}
      height={displayH}
      class="shadow-2xl"
      style="image-rendering: auto;"
    />
  {:else}
    <div class="text-muted-foreground text-sm">
      {#if $currentScreenshot}
        Loading...
      {:else}
        No screenshot. Click Capture to take one.
      {/if}
    </div>
  {/if}
</div>
