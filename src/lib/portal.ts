/** Svelte action that relocates a node to <body>, so a popover escapes any
 *  ancestor `overflow:hidden` / stacking-context clipping. */
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}
