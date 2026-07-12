// Quote-aware argv splitting for the custom-launcher form (review #6).
//
// `cmd.split(/\s+/)` couldn't represent paths with spaces ("/home/user/My Games/launcher").
// This tokenizer decides WORD BOUNDARIES only — there is still no shell anywhere: the result
// goes to the backend's `Command::new(argv[0]).args(argv[1..])` verbatim, so quoting here
// grants no new capability, it just lets legitimate paths survive the split.
//
// Rules (deliberately the small, predictable subset of POSIX-ish quoting):
// - "…" and '…' group characters, including whitespace, into the current word.
//   Quotes may appear mid-word: --path="/My Games" is one arg `--path=/My Games`.
// - Inside "double quotes", backslash escapes only `"` and `\` (so Windows-style paths
//   pasted inside quotes don't need doubling of every slash).
// - Inside 'single quotes', everything is literal (no escapes), like sh.
// - Outside quotes, backslash escapes the next character (space, quote, backslash, …).
// - An unbalanced quote returns null — the form rejects it instead of guessing.

/** Split a command line into argv, honoring quotes. Returns null on an unbalanced quote. */
export function splitArgv(cmd: string): string[] | null {
  const out: string[] = [];
  let cur = "";
  let inWord = false; // distinguishes `""` (empty arg) from no arg at all
  let quote: '"' | "'" | null = null;
  for (let i = 0; i < cmd.length; i++) {
    const ch = cmd[i];
    if (quote === "'") {
      if (ch === "'") quote = null;
      else cur += ch;
    } else if (quote === '"') {
      if (ch === '"') quote = null;
      else if (ch === "\\" && (cmd[i + 1] === '"' || cmd[i + 1] === "\\")) cur += cmd[++i];
      else cur += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      inWord = true;
    } else if (ch === "\\" && i + 1 < cmd.length) {
      cur += cmd[++i];
      inWord = true;
    } else if (/\s/.test(ch)) {
      if (inWord) { out.push(cur); cur = ""; inWord = false; }
    } else {
      cur += ch;
      inWord = true;
    }
  }
  if (quote) return null; // unbalanced quote — let the caller surface an error
  if (inWord) out.push(cur);
  return out;
}
