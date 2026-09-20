# econfcheck

Validates that real files in a directory actually follow the rules in
its own `.editorconfig` — an `eclint` (Node) alternative with no Node
runtime. `.editorconfig` is a widely-adopted, editor-agnostic convention
(most IDEs honor it automatically), but nothing stops a file from
drifting out of compliance (a paste from elsewhere, a tool that doesn't
respect it) — this is the CI gate that actually enforces what the file
only *suggests*.

## Usage

```bash
econfcheck              # scan the current directory using its own .editorconfig
econfcheck path/to/dir  # scan a specific directory
```

Reads `<dir>/.editorconfig`, applies its rules to every real file under
`<dir>` (skipping `.git`/`target`/`node_modules`/`vendor`/`dist`/`build`),
and prints every violation as `path:line: rule — message` (file-level
rules like a missing final newline print without a line number). Exit
code `1` if anything is found.

## Rules checked

The five most commonly-set real `.editorconfig` properties:
`indent_style` (space vs. tab — checked by the first character of each
line's leading whitespace, so tabs used for later alignment on an
otherwise space-indented line aren't flagged), `indent_size` (for
space-indented files only — flags an indent that isn't a multiple of the
configured size), `trim_trailing_whitespace`, `insert_final_newline`, and
`end_of_line` (`lf`/`crlf` consistency, checked byte-for-byte, not just
guessed from the platform).

Glob matching supports `*` (any run of characters except `/`), `**` (any
run including `/`, with the standard "`**/` can match zero directories"
convention so `src/**/*.rs` matches both `src/lib.rs` and
`src/a/b/lib.rs`), `?` (one character except `/`), and `{a,b,c}`
alternation — the shapes real `.editorconfig` files actually use.
Character classes (`[abc]`, `[!abc]`) aren't supported.

## Status: built and verified against a real .editorconfig and real files on disk

- **30 unit tests** (`cargo test --lib`): the glob matcher (exact names,
  `*` not crossing `/`, `**` crossing `/` including the zero-directory
  collapse case, `?`, `{...}` alternation, a malformed unclosed `{`
  handled without panicking); the `.editorconfig` parser (root
  detection, comments via both `#` and `;`, key/value
  lowercasing/trimming, a malformed line skipped rather than aborting
  the whole file, later sections correctly overriding earlier ones
  per-property for a file both match); and every rule checker
  individually plus a multi-violation-on-one-file case.
- **A real bug caught by the test suite itself**: the first version of
  the `**` glob handling only matched `**` as "any characters," which
  correctly handled `src/**/*.rs` against `src/a/b/c.rs` but failed
  against `src/c.rs` — real glob tools (gitignore, minimatch) treat
  `**/` as collapsible to zero directories, and mine didn't. Fixed by
  making `**` first try matching its own trailing slash away entirely
  before falling back to consuming one more character and retrying.
- **Live-verified against the real compiled binary, a real
  `.editorconfig`, and real files on disk**: a fixture directory with
  `indent_style = space`, `indent_size = 4`, `trim_trailing_whitespace =
  true`, and `insert_final_newline = true` scoped to `*.rs`. A clean
  file produced zero violations. A deliberately broken file (tab
  indentation, trailing spaces, no final newline) was flagged with all
  three violations, each on the correct line (or file-level, for the
  missing newline), and exit code `1`.

**Not done / deliberately deferred**: nested `.editorconfig` files at
different directory depths (real EditorConfig supports one per
directory, with `root = true` stopping the upward search from a given
file — this reads exactly one `.editorconfig` at the directory you point
it at and applies it to everything beneath, which covers the overwhelmingly
common single-root-config case but not a repo with intentionally
different rules in different subtrees); `max_line_length` and `charset`
properties (the five checked above are what most real-world configs
actually set); character-class globs (`[abc]`).
