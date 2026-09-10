# ADR-001: Tauri v2 + React + SQLite for Desktop Application

## Status
Accepted

## Context
OpenDAM is a local-first digital asset manager. We need a desktop app framework that:
- Works on Windows, macOS, Linux
- Can access the local filesystem and run system processes
- Has a modern, performant UI
- Keeps bundle sizes small and avoids embedding a full Electron Chromium

## Decision
- **Shell**: Tauri v2 (Rust backend + OS WebView)
- **UI**: React 18 + TypeScript + Vite (fast HMR, tree-shaking, CSS modules)
- **State**: Zustand (minimal, no boilerplate)
- **DB**: SQLite via sqlx (local-first, zero external deps)
- **FTS**: SQLite FTS5 (built-in, fast, no extra service)

## Consequences
- ✅ Small installer (~5MB vs ~80MB Electron)
- ✅ Native performance for Rust hot paths (indexing, FS watching)
- ✅ Single binary distribution, no Node.js runtime required
- ⚠️ WebView differences between OS (Safari/WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux)
- ⚠️ Inter-process: React ↔ Rust via Tauri IPC (typed commands)
