/** Pure, Svelte-free typeahead match logic for the Combobox.
 *  Matching is case-insensitive substring over LABEL + ALIASES only (NOT the
 *  raw slug). The empty query returns every option unchanged. */

export interface ComboOption {
  label: string;
  value: string; // raw slug; "" reserved for the "any" row
  aliases?: string[];
}

/** Match tiers, lowest sorts first. */
const TIER_LABEL_PREFIX = 0;
const TIER_LABEL_SUBSTRING = 1;
const TIER_ALIAS_ONLY = 2;

function tierFor(option: ComboOption, q: string): number | null {
  const label = option.label.toLowerCase();
  if (label.startsWith(q)) return TIER_LABEL_PREFIX;
  if (label.includes(q)) return TIER_LABEL_SUBSTRING;
  const aliases = option.aliases ?? [];
  for (const alias of aliases) {
    if (alias.toLowerCase().includes(q)) return TIER_ALIAS_ONLY;
  }
  return null;
}

/** Case-insensitive substring match on LABEL + ALIASES only (NOT the slug).
 *  Empty query returns all options unchanged. Ranking: label-prefix, then
 *  label-substring, then alias-only match; stable within each tier. */
export function filterOptions(options: ComboOption[], query: string): ComboOption[] {
  const q = query.trim().toLowerCase();
  if (q === "") return options;

  const matched: { option: ComboOption; tier: number; index: number }[] = [];
  options.forEach((option, index) => {
    const tier = tierFor(option, q);
    if (tier !== null) matched.push({ option, tier, index });
  });

  // Sort by tier, then original index → stable within each tier.
  matched.sort((a, b) => a.tier - b.tier || a.index - b.index);
  return matched.map((m) => m.option);
}
