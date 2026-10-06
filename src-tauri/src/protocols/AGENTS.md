# Protocol processing

## Scope

Own streaming Telnet, ANSI, MXP, GMCP, ATCP, and MSP interpretation as those protocols are implemented. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Accept arbitrary input chunking; retain only bounded incomplete sequences and preserve text ordering.
- Separate protocol framing from decoded display/events. Never execute server markup or treat it as trusted frontend content.
- Document unsupported variants and malformed-input recovery against primary sources; do not claim compliance from happy-path samples.

## Verification

Test every meaningful split point, escaped control bytes, repeated negotiation, malformed or oversized payloads, and interleaved text. Keep parsers fuzzable without network access.
