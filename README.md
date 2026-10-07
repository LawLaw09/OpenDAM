# OpenDAM 🗂️✨

<div align="center">

![OpenDAM Banner](https://raw.githubusercontent.com/LawLaw09/OpenDAM/main/apps/desktop/src-tauri/icons/128x128.png)

### The First Free & Open-Source Digital Asset Management (DAM) System for 3D, CAD, Architecture & Design

**A modern, local-first, high-performance alternative to Connecter, Eagle, and Adobe Bridge.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Platform](https://img.shields.io/badge/Platform-Windows-0078D6?logo=windows&logoColor=white)](https://github.com)
[![Built with Tauri](https://img.shields.io/badge/Tauri-v2-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Backend-Rust_1.80+-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![React](https://img.shields.io/badge/Frontend-React_18_+_TypeScript-61DAFB?logo=react&logoColor=black)](https://react.dev)
[![Three.js](https://img.shields.io/badge/3D_Viewport-Three.js-000000?logo=threedotjs&logoColor=white)](https://threejs.org)

[**Download Latest Release (.exe)**](#-download--installation) • [**Features**](#-features) • [**NAS & Team Workflow**](#-network-and-nas-architecture) • [**Build from Source**](#-build-from-source)

</div>

---

## 🌟 Why OpenDAM?

Proprietary asset managers force creative studios and 3D artists into expensive per-seat subscriptions, centralized cloud lock-in, or brittle network databases that corrupt over local file shares.

**OpenDAM is different:**
- **100% Free & Open Source (MIT)**: Zero per-seat fees, zero subscription costs.
- **Local-First & Private**: Your files and databases never leave your machine or private office network. Zero telemetry, zero cloud tracking.
- **Built for Teams & NAS**: Each library maintains its own self-contained database (`.opendam/library.sqlite`) directly inside the shared folder. Anyone who mounts the directory gets immediate access to the shared previews, tags, and collections without server setup.
- **Engineered for Speed**: SQLite FTS5 instant full-text search, in-memory directory caching, and batch commits that protect NAS hardware from I/O exhaustion.

---

## 🚀 Features

### 🎨 Visual-First Browsing (Never Blank Icons)
- **Supported 3D/CAD Formats**: `.max`, `.rvt`, `.rfa`, `.dwg`, `.dxf`, `.skp`, `.blend`, `.c4d`, `.3dm`, `.obj`, `.fbx`, `.3ds`, `.dae`, `.gltf`, `.glb`, `.stl`, and more.
- **Materials & Textures**: `.mat`, `.vrmat`, `.sbsar`, `.jpg`, `.png`, `.tga`, `.tif`, `.exr`, `.hdr`, `.ies`.
- **Spacebar Quick Look**: Press and hold `Spacebar` on any selected asset for an instant, enlarged high-definition preview overlay.
- **Interactive 3D Viewport**: Orbit, pan, and inspect 3D geometry in real-time powered by Three.js.
- **One-Click Path Copying**: Select an asset and hit `Ctrl + C` to instantly copy its full, absolute file path to your clipboard.

### 🏢 Network and NAS Architecture (Zero Stress on Storage)
Network-attached storage (NAS) and office SMB shares have limited read/write capabilities. OpenDAM implements an ultra-efficient network protocol:
- **In-Memory Traversal**: Startup scanning preloads file metadata in 1 single query. Unchanged files are verified in RAM in **0 microseconds with 0 database queries**, indexing 6,000+ assets in under 200 ms.
- **Chunked Batch Transactions**: All database writes are committed in batches of 200 items, slashing network sync packets and SQLite lock contention by **over 98%**.
- **Thumbnail Pre-Check**: Previews are rendered once into `.opendam/thumbs/`. OpenDAM pre-checks thumbnail existence on disk, completely eliminating redundant reads of 100MB+ CAD files across the network.
- **RAM Temp Storage**: Temp sorting buffers, FTS5 scratchpads, and index buffers are kept in local RAM (`PRAGMA temp_store = MEMORY`), writing zero temporary files to your NAS.
- **Portable Relative Paths**: Paths inside `.opendam/library.sqlite` are stored relative to the library root. Team members mounting the share on `Z:\`, `Y:\`, or `\\server\share` share the exact same database without broken links.

### 🗂️ Professional Organization & Safety
- **Hierarchical Collections & Sub-Collections**: Organize assets into nested collection trees with live recursive asset count rollups.
- **Multi-Level Tagging & Color Labels**: Quickly categorize assets with color-coded labels, star ratings, and tags.
- **Accidental-Deletion Safeguards**: High-value items (libraries, collections, tags) are protected by a confirmation dialog requiring you to type `confirm` before deletion.
- **Revit Backup Auto-Filtering**: Automatic detection and suppression of Revit backup files (`*.0001.rfa`, `*.0002.rvt`) keeps your workspace clean.
- **DCC Integration Ready**: Drag and drop assets directly into 3ds Max, Blender, Revit, SketchUp, and Unreal Engine.

---

## 📦 Download & Installation

### Windows (Recommended)
1. Download the latest installer from the [Releases](https://github.com) page:
   - **`OpenDAM_x64-setup.exe`** (Standard Windows Installer with Desktop & Start Menu shortcuts)
   - **`opendam-desktop.exe`** (Portable standalone executable, no installation needed)
2. Run the installer and launch **OpenDAM**.
3. Click **"+"** under Libraries in the left sidebar, choose any folder with 3D or CAD files, and watch OpenDAM index your assets visually in seconds!

---

## 🛠️ Tech Stack

| Layer | Technology |
|---|---|
| **Shell & Runtime** | [Tauri v2](https://tauri.app) (Rust + Native WebView) |
| **Frontend** | [React 18](https://react.dev), [TypeScript](https://www.typescriptlang.org), [Vite](https://vitejs.dev) |
| **Styling** | Vanilla CSS Modules |
| **State Management** | [Zustand](https://github.com/pmndrs/zustand) |
| **Database & Search** | SQLite + FTS5 full-text indexing via [SQLx](https://github.com/launchbadge/sqlx) |
| **3D Rendering** | [Three.js](https://threejs.org) / [@react-three/fiber](https://github.com/pmndrs/react-three-fiber) |
| **Preview Engine** | Python worker (Pillow, OpenImageIO, assimp, olefile, ezdxf, ffmpeg) |

---

## 💻 Build from Source

### Prerequisites
- [Node.js 20+](https://nodejs.org) and [pnpm 9+](https://pnpm.io)
- [Rust 1.80+](https://www.rust-lang.org)
- Python 3.10+ (for preview engine workers)

### Development Setup

```bash
# Clone the repository
git clone https://github.com/LawLaw09/OpenDAM.git
cd opendam

# Install dependencies
pnpm install

# Run desktop app in development mode (hot-reload)
pnpm dev
```

### Production Build

```bash
# Build the production Vite bundle and Tauri executable
pnpm --filter @opendam/desktop build
```
The compiled installer will be available at:
`apps/desktop/src-tauri/target/release/bundle/nsis/OpenDAM_0.1.0_x64-setup.exe`

---

## 🗺️ Project Roadmap

- [x] Per-Library Portable SQLite Architecture (`.opendam/library.sqlite`)
- [x] Extreme NAS Efficiency (in-memory caching, 200-item batch transactions)
- [x] Multi-format 3D & CAD Support (DWG, RVT, RFA, MAX, SKP, OBJ, FBX, BLEND)
- [x] Spacebar High-Definition Quick Look Overlay
- [x] Hierarchical Sub-Collections & Recursive Filtering
- [x] Accidental-Deletion Protection Dialogs
- [x] Revit Backup Auto-Suppression (`filename.####.rfa`)
- [ ] DCC Bridge Plugins: 3ds Max, Blender, Revit, SketchUp, Cinema 4D
- [ ] Headless Turnable & Video Turntable Rendering
- [ ] Custom User Attributes & Metadata Schemas

---

## 🤝 Contributing

Contributions are welcome! OpenDAM is an open, community-driven project. If you'd like to report a bug, request a format, or contribute a feature:
1. Fork the repo.
2. Create a feature branch (`git checkout -b feature/amazing-feature`).
3. Commit your changes (`git commit -m 'Add amazing feature'`).
4. Push to the branch (`git push origin feature/amazing-feature`).
5. Open a Pull Request.

---

## 📄 License

Distributed under the **MIT License**. See [`LICENSE`](./LICENSE) for more information.
All binary files stay on your machine or network storage — you retain 100% ownership and control over your creative assets.
