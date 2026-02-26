<script lang="ts">
  import "../app.css";

  let { children } = $props();
</script>

<svelte:document oncontextmenu={(e) => e.preventDefault()} />

{@render children()}
