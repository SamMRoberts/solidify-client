# Parser fuzz targets

## Scope

Own individual harnesses for protocol parsing behavior. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Feed arbitrary byte streams and varied chunk boundaries into the same parsers used by the application.
- Assert meaningful properties such as termination, bounded state, and equivalent output for different chunking.
- Avoid clocks, external processes, network connections, and silent suppression of parser failures.

## Verification

Reproduce minimized failures deterministically and add regression coverage when the parser is fixed.
