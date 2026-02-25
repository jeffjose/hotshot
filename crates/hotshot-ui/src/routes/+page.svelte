<script lang="ts">
  import Toolbar from "$lib/components/Toolbar.svelte";
  import Canvas from "$lib/components/Canvas.svelte";
  import Gallery from "$lib/components/Gallery.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import { listScreenshots, imageUrl } from "$lib/api";
  import { currentScreenshot, currentImageSrc } from "$lib/stores/screenshot";
  import { screenshots } from "$lib/stores/gallery";
  import { onMount } from "svelte";

  onMount(async () => {
    try {
      const list = await listScreenshots();
      $screenshots = list;
      if (list.length > 0) {
        $currentScreenshot = list[0];
        $currentImageSrc = imageUrl(list[0].path);
      }
    } catch (e) {
      console.error("Failed to load screenshots:", e);
    }
  });
</script>

<div class="h-screen flex flex-col">
  <Toolbar />
  <div class="flex flex-1 overflow-hidden">
    <Canvas />
    <Gallery />
  </div>
  <StatusBar />
</div>
