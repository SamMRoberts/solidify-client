# Protocol fixture conventions

## Current contents

No traffic captures or executable fixtures are included. This document defines the metadata required for future test material; it contains no fabricated capture.

## Provenance and privacy

For each fixture, record a descriptive name, whether it is synthetic or captured, the relevant protocol and server variant, source or author, capture/creation date, reuse permission or license, and sanitization performed.

Use synthetic traffic when real content is unnecessary. Remove credentials, account identifiers, private chat, tokens, and sensitive paths from captured material. Record transformations that change message lengths or framing, and revalidate the resulting fixture.

## Byte representation

Represent raw stream inputs in labeled fenced blocks as space-separated, two-digit hexadecimal bytes. State that whitespace in the block is a presentation separator, not input data. Preserve line-ending bytes explicitly; do not rely on Markdown line endings or visually rendered control characters.

Keep decoded text and JSON separate from the raw bytes. If escaped text is needed, identify its encoding and escape grammar rather than mixing literal backslashes with control characters. Never embed terminal control bytes directly in Markdown.

Record chunk boundaries as explicit byte offsets or separately labeled chunks, and state whether they model transport reads or protocol message boundaries.

## Expected outcomes

Describe initial negotiation/parser state, input sequence, expected display text/styles, structured events, outbound replies, retained partial state, and expected errors or recovery as relevant. Link to the consuming test when it exists.

State limits and unsupported behavior explicitly. A fixture specification is not evidence that a test passed. Future binary corpora should be introduced only with implementation scope, provenance, and a documented consumer.
