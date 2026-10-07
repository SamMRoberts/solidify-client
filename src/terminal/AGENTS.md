# Terminal presentation

## Scope

Own output display, scrollback, text selection, and command-entry interaction. These instructions supplement ancestor `AGENTS.md` files for this subtree. The bounded transcript model and React renderer implement styled text and basic line controls, not a terminal cell grid. Transcript-scoped appearance preserves explicit ANSI colors, inverse defaults, scroll anchoring and the existing measured viewport path.

## Working guidance

- Render parsed text and styles without inserting raw MUD markup into the DOM.
- Preserve output order, meaningful whitespace, and partial prompt updates; do not reinterpret Telnet bytes here.
- Bound retained output and keep user-controlled scroll position stable. Prevent password-mode input from entering ordinary history or logs.

## Verification

Cover long output bursts, Unicode, split prompt updates, scrollback, keyboard input, and hostile markup.
