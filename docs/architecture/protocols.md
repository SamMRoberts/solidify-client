# Planned protocol support

## Status

No protocol parser or transport is implemented. This document defines intended coverage and acceptance concerns. Source links below are references, not a claim of full compliance; server-specific behavior must be identified separately.

| Protocol | Intended capability | Reference |
|---|---|---|
| Telnet | Stream framing, commands, negotiation, and subnegotiation | [RFC 854](https://www.rfc-editor.org/rfc/rfc854), [RFC 855](https://www.rfc-editor.org/rfc/rfc855) |
| ANSI controls | Text styling and a deliberately supported control subset | [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/) |
| MXP | Supported markup converted into safe client display/actions | [Zugg Software MXP specification](https://www.zuggsoft.com/zmud/mxp.htm) |
| GMCP | Negotiated structured messages and documented package handling | [Aardwolf GMCP documentation](https://www.aardwolf.com/wiki/index.php/Clients/GMCP), a server-specific reference |
| ATCP | Negotiated structured extension messages | [Iron Realms ATCP reference](https://www.ironrealms.com/rapture/manual/files/FeatATCP-txt.html) |
| MSP | Sound directives subject to local resource and playback policy | [Zugg Software MSP specification](https://www.zuggsoft.com/zmud/msp.htm) |

## Parsing requirements

- Accept arbitrary transport chunk boundaries, including splits within control sequences, text encoding, and subnegotiations.
- Preserve ordinary text, ordering, and partial prompts while routing structured protocol data separately.
- Maintain negotiation state per connection and avoid repeated negotiation loops. Document unsupported options and recovery behavior.
- Bound incomplete sequences, payload sizes, nesting, and buffered output; reject or recover from malformed input deliberately.
- Keep Telnet framing distinct from text decoding. Select and document encoding behavior during implementation.
- Interpret supported markup as data. Never pass server-provided HTML, scripts, URLs, or commands directly into privileged execution.
- Gate media retrieval, links, and command actions through explicit application policies. A server request is not user authorization for arbitrary downloads or execution.

## Acceptance evidence

For each implemented feature, use deterministic inputs covering valid, unknown, truncated, repeated, interleaved, and oversized messages. Feed equivalent streams with different chunk boundaries and compare observable results. Reset state across disconnects and confirm no state crosses sessions.

Document unsupported variants and capture provenance in the [fixture guide](../../resources/fixtures/README.md). Public-server acceptance, when explicitly requested, supplements local tests; it does not establish universal compatibility. See [testing](../development/testing.md).
