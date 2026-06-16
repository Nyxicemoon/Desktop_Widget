<script lang="ts">
  // Widget windows are transparent; the global transparent body comes from app.css.
  let { children } = $props();
</script>

{@render children()}
