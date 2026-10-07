## Description

Briefly describe the purpose of this Pull Request and what changes were made.

Closes # (issue number if applicable)

---

## Type of Change

- [ ] 🐛 Bug fix (non-breaking change which fixes an issue)
- [ ] ✨ New feature (non-breaking change which adds functionality)
- [ ] ⚡ Performance optimization (rendering, SQLite, or preview engine speedup)
- [ ] 🎨 UI / UX refinement
- [ ] 📝 Documentation update
- [ ] 🔧 Refactoring / Code quality

---

## How Was This Tested?

Please describe the manual or automated tests conducted to verify your changes:

- [ ] Verified locally on Windows / macOS / Linux
- [ ] Tested with real asset files (e.g. `.max`, `.rvt`, `.skp`, `.fbx`, `.vrmat`)
- [ ] Verified SQLite database operations and NAS compatibility

---

## Checklist

- [ ] My code follows the project's code style and conventions
- [ ] `pnpm --filter @opendam/desktop vite:build` passes cleanly
- [ ] `cargo clippy --all-targets -- -D warnings` passes without errors or warnings
- [ ] `ruff check packages/preview-engine/` passes without errors
- [ ] No secrets, local `.sqlite` databases, or personal files are committed
