<script lang="ts">
  import { filterOptions, type ComboOption } from "$lib/combobox";
  import { portal } from "$lib/portal";

  let {
    options,
    value = $bindable(),
    placeholder,
    anyLabel = "any",
    id,
    onchange,
    clearable = true,
  }: {
    options: ComboOption[];
    value: string;
    placeholder: string;
    anyLabel?: string;
    id?: string;
    onchange?: (value: string) => void;
    clearable?: boolean;
  } = $props();

  // --- local state ---
  let open = $state(false);
  let query = $state("");
  let activeIndex = $state(-1);

  // element refs
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let popEl = $state<HTMLDivElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let listEl = $state<HTMLUListElement | null>(null);

  // positioning (fixed coords derived from the trigger rect)
  let pos = $state({ left: 0, top: 0, width: 0, above: false });

  // --- ids for ARIA wiring ---
  const uid = Math.random().toString(36).slice(2, 8);
  const baseId = $derived(id ?? `cbx-${uid}`);
  const listboxId = $derived(`${baseId}-listbox`);
  const optionId = (i: number) => `${baseId}-opt-${i}`;

  // --- derived data ---
  const selected = $derived(options.find((o) => o.value === value) ?? null);
  const selectedLabel = $derived(selected?.label ?? null);
  const isAny = $derived(value === "");
  const filtered = $derived(filterOptions(options, query));
  // The pinned "any/clear" row only appears when the query is empty.
  const showAnyRow = $derived(query.trim() === "");
  const activeOptionId = $derived(activeIndex >= 0 ? optionId(activeIndex) : undefined);

  interface Segment {
    text: string;
    mark: boolean;
  }
  interface OptionView {
    option: ComboOption;
    segments: Segment[]; // label split into matched / unmatched runs
    aliasHint: string | null; // alias that caused the match, when label didn't match
  }

  function buildView(option: ComboOption, q: string): OptionView {
    const label = option.label;
    if (q === "") return { option, segments: [{ text: label, mark: false }], aliasHint: null };

    const lower = label.toLowerCase();
    const idx = lower.indexOf(q);
    if (idx >= 0) {
      const segments: Segment[] = [];
      if (idx > 0) segments.push({ text: label.slice(0, idx), mark: false });
      segments.push({ text: label.slice(idx, idx + q.length), mark: true });
      if (idx + q.length < label.length) segments.push({ text: label.slice(idx + q.length), mark: false });
      return { option, segments, aliasHint: null };
    }

    // Label didn't match → it survived the filter via an alias.
    const aliasHint = (option.aliases ?? []).find((a) => a.toLowerCase().includes(q)) ?? null;
    return { option, segments: [{ text: label, mark: false }], aliasHint };
  }

  const optionViews = $derived(filtered.map((o) => buildView(o, query.trim().toLowerCase())));

  // --- positioning ---
  function reposition() {
    if (!triggerEl) return;
    const r = triggerEl.getBoundingClientRect();
    const popH = popEl?.offsetHeight ?? 0;
    const spaceBelow = window.innerHeight - r.bottom;
    const spaceAbove = r.top;
    const gap = 6; // ~.4rem
    // Flip above only when there isn't room below but there is above.
    const above = popH > 0 && spaceBelow < popH + gap && spaceAbove > spaceBelow;
    pos = {
      left: r.left,
      width: r.width,
      top: above ? r.top - gap : r.bottom + gap,
      above,
    };
  }

  // --- open / close ---
  function openPopover() {
    if (open) return;
    query = "";
    open = true;
    // Start active on the selected option (if visible) else first match.
    queueMicrotask(() => {
      reposition();
      const selIdx = filtered.findIndex((o) => o.value === value);
      activeIndex = selIdx >= 0 ? selIdx : filtered.length > 0 ? 0 : -1;
      inputEl?.focus();
      reposition(); // re-measure after the popover has real height
      scrollActiveIntoView();
    });
  }

  function closePopover(refocus = true) {
    if (!open) return;
    open = false;
    query = "";
    activeIndex = -1;
    if (refocus) triggerEl?.focus();
  }

  function selectOption(option: ComboOption) {
    value = option.value;
    onchange?.(value);
    closePopover();
  }

  function selectAny() {
    value = "";
    onchange?.(value);
    closePopover();
  }

  function clearValue(e: Event) {
    e.stopPropagation();
    value = "";
    onchange?.(value);
    triggerEl?.focus();
  }

  function onClearKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      clearValue(e);
    }
  }

  // --- keyboard ---
  function onTriggerKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openPopover();
    }
  }

  function moveActive(delta: number) {
    const n = filtered.length;
    if (n === 0) {
      activeIndex = -1;
      return;
    }
    const current = activeIndex < 0 ? (delta > 0 ? -1 : 0) : activeIndex;
    activeIndex = (current + delta + n) % n;
    scrollActiveIntoView();
  }

  function scrollActiveIntoView() {
    queueMicrotask(() => {
      if (activeIndex < 0 || !listEl) return;
      const row = listEl.querySelector<HTMLElement>(`#${CSS.escape(optionId(activeIndex))}`);
      row?.scrollIntoView({ block: "nearest" });
    });
  }

  function onInputKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        moveActive(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        moveActive(-1);
        break;
      case "Home":
        e.preventDefault();
        if (filtered.length) {
          activeIndex = 0;
          scrollActiveIntoView();
        }
        break;
      case "End":
        e.preventDefault();
        if (filtered.length) {
          activeIndex = filtered.length - 1;
          scrollActiveIntoView();
        }
        break;
      case "Enter":
        e.preventDefault();
        if (activeIndex >= 0 && filtered[activeIndex]) selectOption(filtered[activeIndex]);
        break;
      case "Escape":
        e.preventDefault();
        closePopover();
        break;
      case "Tab":
        closePopover(false);
        break;
    }
  }

  // When the query changes, keep the active row valid (snap to first match).
  $effect(() => {
    // re-run when the filtered list identity changes
    filtered;
    if (!open) return;
    if (filtered.length === 0) {
      activeIndex = -1;
    } else if (activeIndex < 0 || activeIndex >= filtered.length) {
      activeIndex = 0;
    }
  });

  // --- dismissal + reposition listeners (only while open) ---
  $effect(() => {
    if (!open) return;

    const onPointerDown = (e: PointerEvent) => {
      const t = e.target as Node;
      if (triggerEl?.contains(t) || popEl?.contains(t)) return;
      closePopover(false);
    };
    const onScroll = (e: Event) => {
      // Ignore scrolling inside the popover's own list.
      if (popEl?.contains(e.target as Node)) return;
      closePopover(false);
    };
    const onResize = () => closePopover(false);

    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);

    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<div class="cbx" class:open>
  <button
    bind:this={triggerEl}
    id={id}
    type="button"
    class="cbx-trigger"
    class:is-any={isAny}
    role="combobox"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={listboxId}
    onclick={() => (open ? closePopover() : openPopover())}
    onkeydown={onTriggerKeydown}
  >
    <span class="lab">
      <span class="key">{placeholder}:&nbsp;</span><span class="val">{selectedLabel ?? anyLabel}</span>
    </span>
    {#if clearable && !isAny}
      <span
        class="clear"
        role="button"
        tabindex="-1"
        aria-label={`Clear ${placeholder}`}
        title="Clear"
        onclick={clearValue}
        onkeydown={onClearKeydown}
      >×</span>
    {/if}
    <span class="chev" aria-hidden="true">▾</span>
  </button>

  {#if open}
    <div
      bind:this={popEl}
      class="cbx-pop"
      class:above={pos.above}
      use:portal
      style:left={`${pos.left}px`}
      style:top={`${pos.top}px`}
      style:--cbx-w={`${pos.width}px`}
    >
      <div class="cbx-search">
        <input
          bind:this={inputEl}
          bind:value={query}
          type="text"
          placeholder={`Filter ${placeholder.toLowerCase()}…`}
          aria-label={`Filter ${placeholder}`}
          aria-autocomplete="list"
          aria-controls={listboxId}
          aria-activedescendant={activeOptionId}
          autocomplete="off"
          spellcheck="false"
          onkeydown={onInputKeydown}
        />
        <span class="kbd" aria-hidden="true">↑↓ · ↵ · esc</span>
      </div>

      {#if filtered.length === 0}
        <div class="cbx-empty">
          <div class="ico">∅</div>
          <div class="ttl">No {placeholder.toLowerCase()} matches <q>{query}</q></div>
          <div class="hint">Tried label and aliases.</div>
          <button type="button" class="clearbtn" onclick={() => { query = ""; inputEl?.focus(); }}>
            Clear search
          </button>
        </div>
      {:else}
        <ul bind:this={listEl} id={listboxId} class="cbx-list" role="listbox" aria-label={placeholder}>
          {#if clearable && showAnyRow}
            <!-- svelte-ignore a11y_click_events_have_key_events -- keyboard nav is centralized on the search input via aria-activedescendant (W3C combobox/listbox pattern) -->
            <li
              class="cbx-opt any"
              class:selected={isAny}
              role="option"
              aria-selected={isAny}
              onclick={selectAny}
            >
              <span class="bar" aria-hidden="true"></span>
              <span class="aster" aria-hidden="true">∗</span>
              <span class="otext">{placeholder}: {anyLabel} <span class="raw">(clear filter)</span></span>
              <span class="dot" aria-hidden="true"></span>
            </li>
            <div class="cbx-sep" aria-hidden="true"></div>
          {/if}

          {#each optionViews as view, i (view.option.value)}
            {@const isSel = view.option.value === value}
            <!-- svelte-ignore a11y_click_events_have_key_events -- keyboard nav is centralized on the search input via aria-activedescendant (W3C combobox/listbox pattern) -->
            <li
              id={optionId(i)}
              class="cbx-opt"
              class:selected={isSel}
              class:active={i === activeIndex}
              role="option"
              aria-selected={isSel}
              onclick={() => selectOption(view.option)}
              onpointermove={() => (activeIndex = i)}
            >
              <span class="otext"
                >{#each view.segments as seg}{#if seg.mark}<mark>{seg.text}</mark>{:else}{seg.text}{/if}{/each}{#if view.aliasHint}<span
                    class="hint"> · matches "<mark>{view.aliasHint}</mark>"</span
                  >{/if}</span
              >
              <span class="dot" aria-hidden="true"></span>
            </li>
          {/each}
        </ul>
        <div class="cbx-foot">
          <b>{filtered.length}</b>&nbsp;of {options.length}<span class="spacer"></span>matched on label + alias
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .cbx {
    position: relative;
    font-family: var(--font-sans);
    display: inline-block;
    min-width: 12rem;
  }

  /* --- closed trigger --- */
  .cbx-trigger {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    font-family: var(--font-sans);
    font-size: var(--fs-sm);
    font-weight: 500;
    padding: 0.4rem 0.65rem;
    background: var(--card);
    color: var(--ink);
    border: 1px solid var(--wire);
    border-radius: var(--r-md);
    cursor: pointer;
    text-align: left;
    line-height: 1.2;
    transition: border-color 0.14s, box-shadow 0.14s, background 0.14s;
  }
  .cbx-trigger:hover {
    background: var(--card-hover);
    border-color: var(--primary);
  }
  .cbx-trigger .lab {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cbx-trigger .lab .key {
    color: var(--muted);
    font-weight: 600;
  }
  .cbx-trigger .lab .val {
    color: var(--ink);
    font-weight: 600;
  }
  .cbx-trigger.is-any .lab .val {
    color: var(--faint);
    font-weight: 500;
    font-style: italic;
  }
  .cbx-trigger .clear {
    flex: none;
    width: 1.05rem;
    height: 1.05rem;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--muted);
    font-size: 0.85rem;
    line-height: 1;
    background: transparent;
    transition: background 0.12s, color 0.12s;
  }
  .cbx-trigger .clear:hover {
    background: var(--flat-soft);
    color: var(--primary);
  }
  .cbx-trigger .chev {
    flex: none;
    color: var(--primary);
    font-size: 1.2rem;
    line-height: 1.2;
    transition: transform 0.16s ease;
  }
  .cbx.open .cbx-trigger .chev {
    transform: rotate(180deg);
  }

  .cbx-trigger:focus-visible,
  .cbx.open .cbx-trigger {
    outline: none;
    border-color: var(--primary);
    box-shadow: 0 0 0 3px var(--primary-ring);
    background: var(--card);
  }

  /* --- popover (portaled to body, position:fixed) --- */
  :global(.cbx-pop) {
    position: fixed;
    z-index: 1000;
    min-width: 12rem;
    width: var(--cbx-w);
    max-width: 22rem;
    font-family: var(--font-sans);
    background: var(--card);
    border: 1px solid var(--wire);
    border-radius: var(--r-lg);
    box-shadow:
      0 1px 0 rgba(255, 255, 255, 0.7) inset,
      0 18px 44px -16px rgba(40, 15, 30, 0.42),
      0 4px 12px -8px rgba(40, 15, 30, 0.3);
    overflow: hidden;
    transform-origin: top left;
    animation: cbx-pop 0.14s cubic-bezier(0.2, 0.7, 0.3, 1);
  }
  /* when flipped above, anchor the bottom edge to `top` */
  :global(.cbx-pop.above) {
    transform: translateY(-100%);
    transform-origin: bottom left;
  }
  @keyframes cbx-pop {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.985);
    }
    to {
      opacity: 1;
    }
  }

  :global(.cbx-pop .cbx-search) {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.6rem;
    border-bottom: 1px solid var(--line);
    background: var(--panel-head);
  }
  :global(.cbx-pop .cbx-search .ico) {
    color: var(--primary);
    font-size: 0.85rem;
    flex: none;
    line-height: 1;
  }
  :global(.cbx-pop .cbx-search input) {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    outline: none;
    font-family: var(--font-sans);
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--ink);
    padding: 0.05rem 0;
  }
  :global(.cbx-pop .cbx-search input::placeholder) {
    color: var(--faint);
    font-weight: 400;
  }
  :global(.cbx-pop .cbx-search .kbd) {
    font-family: var(--font-sans);
    font-size: 0.58rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    color: var(--faint);
    border: 1px solid var(--wire);
    border-radius: 5px;
    padding: 0.04rem 0.26rem;
    background: var(--card);
    white-space: nowrap;
  }

  :global(.cbx-pop .cbx-list) {
    list-style: none;
    margin: 0;
    padding: 0.3rem;
    max-height: 16rem;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  :global(.cbx-pop .cbx-opt) {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0.42rem 0.55rem 0.42rem 0.6rem;
    border-radius: var(--r-sm);
    font-family: var(--font-sans);
    font-size: var(--fs-sm);
    color: var(--ink-soft);
    font-weight: 500;
    cursor: pointer;
    position: relative;
    line-height: 1.25;
  }
  :global(.cbx-pop .cbx-opt .bar) {
    position: absolute;
    left: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 3px;
    height: 0;
    border-radius: 99px;
    background: var(--primary);
    transition: height 0.12s;
  }
  :global(.cbx-pop .cbx-opt .otext) {
    flex: 1;
    min-width: 0;
  }
  :global(.cbx-pop .cbx-opt .otext .hint) {
    color: var(--faint);
    font-size: 0.7rem;
    font-weight: 500;
  }
  :global(.cbx-pop .cbx-opt .raw) {
    color: var(--faint);
    font-size: 0.7rem;
    font-weight: 500;
    margin-left: 0.05rem;
  }
  :global(.cbx-pop .cbx-opt mark) {
    background: var(--accent-soft);
    color: var(--accent-ink);
    border-radius: 3px;
    padding: 0 0.06em;
    font-weight: 700;
  }

  /* active (keyboard / hover) row */
  :global(.cbx-pop .cbx-opt.active) {
    background: var(--primary-soft);
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--primary-ring);
  }

  /* selected row */
  :global(.cbx-pop .cbx-opt.selected) {
    color: var(--primary);
    font-weight: 700;
  }
  :global(.cbx-pop .cbx-opt.selected .bar) {
    height: 1.1rem;
  }
  :global(.cbx-pop .cbx-opt .dot) {
    flex: none;
    width: 0.42rem;
    height: 0.42rem;
    border-radius: 50%;
    background: var(--primary);
    opacity: 0;
    transition: opacity 0.12s;
  }
  :global(.cbx-pop .cbx-opt.selected .dot) {
    opacity: 1;
  }

  /* the pinned "any / clear" row */
  :global(.cbx-pop .cbx-opt.any) {
    color: var(--muted);
    font-style: italic;
  }
  :global(.cbx-pop .cbx-opt.any .aster) {
    color: var(--faint);
    font-size: 0.8rem;
    line-height: 1;
    width: 0.9rem;
    text-align: center;
    flex: none;
  }
  :global(.cbx-pop .cbx-opt.any.selected) {
    color: var(--primary);
    font-style: italic;
    font-weight: 700;
  }
  :global(.cbx-pop .cbx-sep) {
    height: 1px;
    background: var(--line);
    margin: 0.3rem 0.2rem;
  }

  /* no-matches empty state */
  :global(.cbx-pop .cbx-empty) {
    padding: 1.15rem 0.8rem 1.3rem;
    text-align: center;
    color: var(--muted);
  }
  :global(.cbx-pop .cbx-empty .ico) {
    width: 2.2rem;
    height: 2.2rem;
    border-radius: 50%;
    display: grid;
    place-items: center;
    margin: 0 auto 0.5rem;
    background: var(--flat-soft);
    color: var(--faint);
    font-size: 0.9rem;
  }
  :global(.cbx-pop .cbx-empty .ttl) {
    font-family: var(--font-sans);
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--ink-soft);
  }
  :global(.cbx-pop .cbx-empty .ttl q) {
    color: var(--primary);
    font-style: normal;
    font-weight: 700;
  }
  :global(.cbx-pop .cbx-empty .hint) {
    font-family: var(--font-sans);
    font-size: 0.72rem;
    color: var(--faint);
    margin-top: 0.2rem;
  }
  :global(.cbx-pop .cbx-empty .clearbtn) {
    margin-top: 0.7rem;
    font-family: var(--font-sans);
    font-size: 0.74rem;
    font-weight: 600;
    color: var(--primary);
    background: var(--primary-soft);
    border: 1px solid rgba(125, 35, 72, 0.2);
    border-radius: var(--r-pill);
    padding: 0.26rem 0.7rem;
    cursor: pointer;
  }
  :global(.cbx-pop .cbx-empty .clearbtn:hover) {
    background: var(--primary-soft);
  }

  /* footer count */
  :global(.cbx-pop .cbx-foot) {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.4rem 0.7rem;
    border-top: 1px solid var(--line);
    background: var(--panel-head);
    font-family: var(--font-sans);
    font-size: 0.66rem;
    color: var(--muted);
  }
  :global(.cbx-pop .cbx-foot b) {
    color: var(--ink-soft);
    font-weight: 700;
  }
  :global(.cbx-pop .cbx-foot .spacer) {
    flex: 1;
  }
</style>
