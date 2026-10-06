# Agent instruction discovery and maintenance

## Discovery

Codex assembles guidance at startup: global instructions first, then project files along the path from the project root to the working directory. Within a directory, an override file takes precedence over the ordinary file; nearer guidance can override broader guidance. The documented default combined project limit is 32 KiB. See the [official AGENTS.md guide](https://learn.chatgpt.com/docs/agent-configuration/agents-md).

Starting at the repository root does not load every descendant guide. This repository therefore asks agents to read applicable nested files before editing their subtrees.

## Repository conventions

Use the root guide for shared working agreements and navigation. Use nested guides for concrete local boundaries, failure concerns, and verification. Do not repeat the entire root policy in each directory or use the documents to define autonomous agents with publishing permissions.

Ordinary `AGENTS.md` files are sufficient here. No overrides, fallback names, user-global changes, or Codex configuration edits are part of this scaffold. General architecture belongs in docs and external references belong in resources.

## Maintaining instructions

When implementation arrives, update affected guides to reflect actual commands and behavior. Keep future design options labeled as proposals. Review ancestor and descendant guidance together so local rules remain consistent with the owning subsystem.

During review, inspect non-empty files, precedence along representative directory paths, and cumulative byte size. Leave margin for global guidance. Link to maintained documentation instead of embedding lengthy manuals.

To confirm runtime discovery, start a fresh Codex session in the relevant directory and ask it to identify the instruction files loaded. A static file review alone cannot prove a running session refreshed its instructions.
