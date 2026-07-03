// a11y: move focus into a dialog when it opens (so screen readers announce it), and restore
// focus to the opener when it closes. No Tab-trap on purpose — nav is controller-first via
// arrow keys, and the catalog binds Tab to its sort toggle; a hard trap would fight both.
// Used by <Modal> and the first-run wizard (which has its own full-screen shell).
export function dialogFocus(node: HTMLElement) {
  const prev = document.activeElement as HTMLElement | null;
  node.focus();
  return { destroy() { prev?.focus?.(); } };
}
