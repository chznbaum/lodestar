<script lang="ts">
  import "$lib/styles/index.css";
  import "@fontsource-variable/bodoni-moda";
  import "@fontsource/lora/400.css";
  import "@fontsource/lora/500.css";
  import "@fontsource/lora/600.css";
  import "@fontsource/spline-sans/400.css";
  import "@fontsource/spline-sans/500.css";
  import "@fontsource/spline-sans/600.css";
  import "@fontsource/spline-sans/700.css";
  import { page } from "$app/state";

  let { children } = $props();

  // Companies is the only built surface; it owns "/" and "/companies/*".
  const onCompanies = $derived(
    page.url.pathname === "/" || page.url.pathname.startsWith("/companies"),
  );
</script>

<div class="app">
  <nav class="rail">
    <div class="rail__brand">Lodestar</div>
    <span class="navlink future">Today</span>
    <span class="navlink future">Triage</span>
    <span class="navlink future">Pipeline</span>
    <a class="navlink" class:on={onCompanies} href="/">Companies</a>
    <span class="navlink future">Network</span>
    <span class="navlink future">Patterns</span>
    <div class="rail__sep"></div>
    <div class="rail__util">Diagnostics</div>
    <span class="navlink future">Checks</span>
  </nav>

  <div class="app__content">
    {@render children()}
  </div>
</div>
