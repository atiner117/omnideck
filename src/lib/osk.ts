// On-screen keyboard layout (controller/mouse text entry for search; search is
// case-insensitive so it's lowercase-only — no shift needed). The page's unified input
// routing uses the grid shape for D-pad movement; SearchModal renders the flat key list.
export const OSK_ROWS = [
  ["a", "b", "c", "d", "e", "f"],
  ["g", "h", "i", "j", "k", "l"],
  ["m", "n", "o", "p", "q", "r"],
  ["s", "t", "u", "v", "w", "x"],
  ["y", "z", "0", "1", "2", "3"],
  ["4", "5", "6", "7", "8", "9"],
  ["␣", ".", "-", "⌫", "✕", "⏎"],
];
export const OSK_FLAT = OSK_ROWS.flat();
export const OSK_COLS = 6;
