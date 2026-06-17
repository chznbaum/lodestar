<script lang="ts">
  import { filterOptions, type ComboOption } from "$lib/combobox";
  import { portal } from "$lib/portal";
  import { segments, type Segment } from "$lib/highlight";

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

  interface OptionView {
    option: ComboOption;
    segments: Segment[]; // label split into matched / unmatched runs
    aliasHint: string | null; // alias that caused the match, when label didn't match
  }

  function buildView(option: ComboOption, q: string): OptionView {
    const segs = segments(option.label, q);
    // q is already trimmed+lowercased by the caller. Empty query or a label
    // hit → no alias hint; otherwise the row survived via an alias.
    if (q === "" || segs.some((s) => s.mark)) {
      return { option, segments: segs, aliasHint: null };
    }
    const aliasHint = (option.aliases ?? []).find((a) => a.toLowerCase().includes(q)) ?? null;
    return { option, segments: segs, aliasHint };
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
          <div class="cbx-opt__note">Tried label and aliases.</div>
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
                    class="cbx-opt__note"> · matches "<mark>{view.aliasHint}</mark>"</span
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
