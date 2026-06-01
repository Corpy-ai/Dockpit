# Developer Notes

Internal technical notes kept for historical context. They document how
specific rendering, performance, and visual problems were diagnosed and fixed
during development. **They are not user documentation** — for usage see the
top-level [README](../../README.md) and [CHANGELOG](../../CHANGELOG.md).

| File | What it covers |
|------|----------------|
| [OPTIMIZATIONS.md](./OPTIMIZATIONS.md) | Performance work: task lifecycle, batch processing, adaptive refresh, debouncing. |
| [VISUAL_FIXES.md](./VISUAL_FIXES.md) | Visual residue fixes when switching containers/views; loading screens. |
| [VISUAL_RESIDUES_FIX.md](./VISUAL_RESIDUES_FIX.md) | Deeper dive into terminal buffer clearing and forced redraws. |
| [GHOST_CHARACTERS_FIX.md](./GHOST_CHARACTERS_FIX.md) | Root cause and fix for ghost/leftover characters in the TUI. |
| [MENU_BACKGROUNDS_FIX.md](./MENU_BACKGROUNDS_FIX.md) | Overlay/menu background rendering fixes. |
| [VERSION_AND_FEATURES.md](./VERSION_AND_FEATURES.md) | Versioning notes and a feature-by-feature breakdown. |
| [VERIFICATION.md](./VERIFICATION.md) | Manual verification checklist used before releases. |
