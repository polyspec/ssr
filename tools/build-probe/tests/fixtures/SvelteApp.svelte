<script>
  let { name, renderState } = $props();
  let count = $state(0);
  function updateRenderState() {
    if (renderState.input !== null) {
      renderState.output = { count: renderState.input.count + 1 };
    }
  }
  updateRenderState();
</script>

<svelte:head>
  <meta name="svelte-head" content={name}>
  <script>window.__svelteHead = true;</script>
  <style>.svelte-head-style { color: #abcdef; }</style>
</svelte:head>

<main>
  <h1>{name ?? location.pathname}</h1>
  <button onclick={() => count += 1}>{count}</button>
</main>

<style>
  main { color: #123456; }
</style>
