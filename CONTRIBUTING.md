# Contributing to OpenDAM 🤝

Thank you for your interest in contributing to **OpenDAM**! OpenDAM is the first free and open-source Digital Asset Management (DAM) system engineered specifically for 3D artists, architects, visualizers, and creative studios.

We welcome all contributions: bug fixes, new 3D/CAD format parsers, performance optimizations, UI improvements, DCC plugins, and documentation.

---

## 🏗️ Repository Architecture

OpenDAM is structured as a high-performance monorepo using **pnpm workspaces**:

```
opendam/
├── apps/
│   └── desktop/
│       ├── src/               # React 18 + TypeScript + Vite + Zustand frontend
│       │   ├── components/    # Modular UI components (AssetGrid, DetailPanel, Sidebar, etc.)
│       │   ├── store/         # Zustand state management stores
│       │   └── types/         # Core TypeScript domain models
│       └── src-tauri/         # Rust backend (Tauri v2)
│           ├── migrations/    # SQLite DDL schemas (master & per-library DBs)
│           ├── src/
│           │   ├── commands/  # Tauri IPC commands callable from frontend
│           │   ├── db.rs      # Multi-library SQLite connection manager & NAS pragmas
│           │   ├── preview.rs # In-process Rust image thumbnailer & persistent Python bridge
│           │   └── watcher.rs # High-speed in-memory directory indexing & change detection
│           └── Cargo.toml
├── packages/
│   └── preview-engine/        # Headless Python daemon worker for 3D/CAD/DCC formats
│       ├── worker.py          # JSON-RPC listener (Max, Revit, SketchUp, FBX, Rhino, VRmat, etc.)
│       └── pyproject.toml
├── plugins/
│   └── blender/               # DCC integration bridge plugins
└── docs/                      # Architectural Decision Records (ADRs) and system specs
```

---

## 🛠️ Prerequisites

Before developing locally, ensure you have the following installed:

1. **Node.js**: `v20.x` or later ([nodejs.org](https://nodejs.org))
2. **pnpm**: `v9.x` or later (`npm install -g pnpm`)
3. **Rust**: `1.80+` stable toolchain ([rustup.rs](https://rustup.rs))
4. **Python**: `3.10+` with `pip` ([python.org](https://python.org))
5. **C++ Build Tools**: (On Windows, Visual Studio C++ Build Tools with Windows SDK)

---

## 🚀 Development Setup

1. **Clone the repository**:
   ```bash
   git clone https://github.com/LawLaw09/OpenDAM.git
   cd opendam
   ```

2. **Install frontend dependencies**:
   ```bash
   pnpm install
   ```

3. **Set up the Python preview engine virtualenv**:
   ```bash
   cd packages/preview-engine
   python -m venv venv
   # On Windows:
   .\venv\Scripts\pip install -r requirements.txt
   # On macOS/Linux:
   ./venv/bin/pip install -r requirements.txt
   cd ../..
   ```

4. **Run OpenDAM in development mode**:
   ```bash
   # Starts the Vite dev server with hot reload and launches the Tauri window
   pnpm dev
   ```

---

## 🧪 Pre-PR Validation Checklist

Before submitting a Pull Request, ensure that all automated checks pass locally. These match the automated GitHub Actions CI pipeline:

### 1. Frontend Typecheck & Build
```bash
pnpm --filter @opendam/desktop vite:build
```

### 2. Rust Clippy & Compilation
```bash
cd apps/desktop/src-tauri
cargo clippy --all-targets -- -D warnings
cargo check
cd ../../
```

### 3. Python Preview Engine Lint
```bash
ruff check packages/preview-engine/
```

---

## 📐 Core Architectural Principles

When contributing code, please adhere to our core design principles:

### 1. NAS & Local-First Safety
- **Rollback Journaling**: OpenDAM explicitly uses SQLite `DELETE` (rollback) journaling for `.opendam/library.sqlite` databases. **Never change this to WAL mode for library databases**, as SQLite WAL shared-memory (`-shm`) locking is unreliable on network file shares (SMB/NFS) and can cause silent corruption across multi-user environments.
- **Batch Commits**: Always batch database writes (200+ items per transaction) to protect NAS spinning disks and network file shares from I/O exhaustion.
- **Relative File Paths**: All paths stored in library SQLite databases must be relative to the library root (`.opendam/thumbs/...` or `textures/...`). Never store machine-specific absolute drive letters (`C:\`, `D:\`) in `.opendam/library.sqlite`.

### 2. Zero Telemetry & Privacy
- OpenDAM has **zero analytics, zero telemetry, and zero cloud tracking**. No external API calls may be added to third-party tracking services.

### 3. High Performance First
- **Native Rust Resizing**: Standard raster images (`.jpg`, `.png`, `.tga`, `.tif`, `.bmp`, `.webp`) must be handled directly in Rust via the `image` crate. Do not dispatch standard raster images to Python.
- **Persistent Workers**: When calling external workers (like Python), never spawn new processes per file. Communicate through persistent stdin/stdout streaming daemons.

---

## 🌿 Git & Pull Request Guidelines

1. **Branch Naming**:
   - `feat/feature-name` (e.g., `feat/blender-material-preview`)
   - `fix/bug-description` (e.g., `fix/exr-alpha-rendering`)
   - `perf/optimization` (e.g., `perf/sqlite-index-optimization`)
   - `docs/documentation-update`

2. **Commit Messages**:
   Follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat: add thumbnail extractor for Rhino .3dm curves`
   - `fix: resolve crash on corrupted DWG header sentinel`
   - `perf: implement persistent Python worker loop`
   - `docs: update build instructions for Linux`

3. **Pull Request Process**:
   - Open a PR against the `main` branch.
   - Fill out the PR template with a description of the changes and how you verified them.
   - Ensure the CI workflow passes on your PR.

---

## 💬 Community & Questions

Have questions, ideas, or feedback?
- Open an [Issue](https://github.com/LawLaw09/OpenDAM/issues) for bug reports and feature proposals.
- Join the discussion in [GitHub Discussions](https://github.com/LawLaw09/OpenDAM/discussions).

Thank you for helping make OpenDAM the best open-source asset manager for creators worldwide!
