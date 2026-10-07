# Eighth slice: extended ANSI colors

## Outcome and scope

Render common MUD color output through the real Telnet → presentation → desktop
path: explicit bright colors, indexed 256-color output, and direct RGB. Preserve
the existing palette's first eight entries, profile default colors, bold semantics,
mask protection, bounded transcript, single-session transport, and SOLIDIFY TTYPE.
No dependencies, persistence/schema changes, palette editor, terminal emulation,
OSC palette mutation, hyperlinks, or new Telnet negotiation are part of this slice.

## Decoding contract

Follow the [xterm control-sequence reference](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
for SGR 90–97/100–107, 38/48 with mode 5 (indexed) and mode 2 (RGB).
Support semicolon forms `38;5;n` / `48;5;n` and `38;2;r;g;b` / `48;2;r;g;b`.
Also support colon forms `38:5:n`, `38:2:r:g:b`, and `38:2::r:g:b`, plus background
counterparts. An explicit color-space slot is accepted only when empty or zero;
other color spaces and additional subparameters remain unsupported.

Operands must be present decimal integers in 0–255. Empty color operands, overflow,
incomplete groups, unsupported modes and mixed separators inside a group make the
entire completed SGR a no-op. Normal empty top-level parameters still mean reset.
Never reinterpret rejected color operands as bold/italic/reset codes. Semicolon
parameters after a complete group apply normally, left to right, with one final
style snapshot and no partial commit. Existing framing errors remain latched errors.

Preserve the 128-byte sequence limit, 16 semicolon-separated fields (including
operands and empty fields), 4 KiB text-event limit, fixed partial state, and lack
of an internal output queue. Colon subparameters occupy one semicolon field but
remain subject to the byte limit. New TextColor variants are typed indexed/RGB
values; existing variants and methods remain. Canonicalize indices 0–7 to basic
colors and bright SGR to indices 8–15. Exhaustive matches over the internal enum
must handle the added variants; this is an additive internal API change.

## Rendering contract

Keep existing named-color DTO strings and add tagged numeric indexed/RGB DTOs.
No arbitrary CSS text crosses this boundary. The frontend converts only validated
integer components into CSS values. Default colors remain profile-controlled;
explicit colors, including inverse combinations, remain fixed. Bold never implies
bright color. Preserve basic palette entries; document eight fixed bright entries,
a 6×6×6 cube at indices 16–231 (levels 0, 95, 135, 175, 215, 255), and grayscale
232–255 (8 + 10×offset). Style equality/coalescing compares color contents, not
object identity or generic object strings. Retained and incoming output must agree.

## Iterations and completion gates

1. Specify contracts; implement the pure decoder with split/bytewise tests, operand
   boundaries, mixed styles, atomic rejection, exact limits, resets and isolation.
2. Carry typed colors across IPC and implement safe rendering, value-based run
   coalescing, palette/inverse/profile-default tests, and unchanged retention limits.
3. Extend the bounded loopback demo with a `colors` command, including fragmented
   RGB/palette samples and reset. Add real session/application path coverage.
4. Run focused tests then all existing library/profiles/desktop/frontend suites,
   formatting, warnings-denied Clippy, builds, API docs, and Markdown/diff checks.
5. Verify browser output at 1100×760 and 720×480, then native macOS rendering through
   the local demo with disposable configuration; inspect defaults, explicit colors,
   inverse, clear/reconnect and cleanup. Record actual evidence separately from
   unverified Windows/Linux and public-server behavior. Update status/instructions.

Completion requires evidence for all five gates; builds alone do not prove UI or
TCP behavior. The final verification record belongs in testing.md.

All five gates are complete. See the [verification record](testing.md#eighth-slice-verification-2026-10-06) for observed results and platform limits.
