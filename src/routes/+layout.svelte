<script lang="ts">
  import "$lib/styles/tokens.css";
  import "$lib/styles/base.css";
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
    <div class="brand">Lodestar</div>
    <span class="navlink future">Today</span>
    <span class="navlink future">Triage</span>
    <span class="navlink future">Pipeline</span>
    <a class="navlink" class:on={onCompanies} href="/">Companies</a>
    <span class="navlink future">Network</span>
    <span class="navlink future">Patterns</span>
    <div class="sep"></div>
    <div class="util">Diagnostics</div>
    <span class="navlink future">Checks</span>
  </nav>

  <div class="content">
    {@render children()}
  </div>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: var(--rail-w) 1fr;
    min-height: 100vh;
  }
  .rail {
    background: var(--rail);
    color: var(--rail-ink);
    padding: 0.6rem 0;
    font-size: var(--fs-sm);
  }
  .brand {
    color: #fff;
    font-family: var(--font-display);
    font-size: var(--fs-mhead);
    font-weight: 700;
    font-variation-settings: "opsz" 72, "wght" 700;
    line-height: 1.1;
    padding: 0.3rem 0.55rem 0.7rem;
  }
  .navlink {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    padding: 0.4rem 1.05rem;
    font-size: 0.95rem;
    font-weight: 500;
    color: var(--rail-ink);
    text-decoration: none;
    margin-bottom: 0.1rem;
  }
  .navlink.on {
    background: var(--rail-ink);
    color: var(--rail);
    font-weight: 700;
  }
  .navlink:not(.on):hover {
    background: var(--rail-hover);
  }
  .navlink.future {
    color: var(--rail-ink);
    cursor: default;
  }
  .sep {
    height: 1px;
    background: var(--rail-sep);
    margin: 0.5rem 0.4rem;
  }
  .util {
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--rail-muted);
    padding: 0.3rem 0.55rem 0.1rem;
  }
  .content {
    min-width: 0;
    background: var(--card);
  }
  @media (max-width: 760px) {
    .app {
      grid-template-columns: 1fr;
    }
    .rail {
      display: none;
    }
  }
</style>
