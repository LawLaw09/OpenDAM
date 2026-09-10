# OpenDAM — Open-Source Connecter-Style Asset Manager

## Implementation Plan (built by agentic AI coding agents)

---

## 1. What This Is (clarified scope)

**Product:** A regular, local-first digital asset manager for 3D/AEC/design files — feature parity with Connecter. No AI features inside the product.

**Development method:** The software is designed, coded, tested, and assembled by agentic AI coding agents, orchestrated by a human tech lead. Every phase below is written so an agent (or a swarm of agents) can execute it with minimal ambiguity: strict interfaces, testable specs, vertical slices, and automated verification.

**Core product principles (same as Connecter):**

- Binary files stay on the user's machine/network — only metadata syncs (team mode)
- Visual-first browsing: thumbnails, interactive 3D viewer, material previews — never blank icons
- Deep drag-and-drop integration with DCC host apps
- Local-first: local SQLite index + optional self-hostable team sync server

---

## 2. Feature Parity Checklist (everything Connecter has)

### 2.1 Visual Previewing

- [ ] Thumbnail generation for formats the OS can't render
- [ ] Interactive 3D model viewer (orbit/zoom/pan, turntable)
- [ ] Material/shader-ball preview (reflections, bump, roughness)
- [ ] HDRI environment preview
- [ ] Image preview incl. PSD, EXR, HDR, layered formats
- [ ] Video playback preview
- [ ] Embedded preview extraction from proprietary files (e.g., `.max` embedded previews, renderer info)
- [ ] Custom previews — user-assigned cover image per asset
- [ ] Batch preview generation (queue + re-generate at scale)
- [ ] Preview caching + lazy loading for libraries of 100k+ assets

### 2.2 Supported Asset Types

- [ ] 3D models: `.max .fbx .obj .3ds .dae .gltf/.glb .skp .blend .c4d .3dm .rfa/.rvt .stl .usd/.usdz ...`
- [ ] Materials: `.mat` (3ds Max), `.vrmat`, Blender materials, substance `.sbsar`
- [ ] Textures/images: `.jpg .png .tga .tif .psd .exr .hdr .bmp .dds ...`
- [ ] IES light profiles
- [ ] BIM files (Revit families, SketchUp)
- [ ] Video: `.mp4 .mov .avi .mkv ...`
- [ ] Documents/other: PDF, notes attached to assets

### 2.3 Metadata Reading (no host app required)

- [ ] Parse `.max` metadata: Max version, renderer, face/object counts, dimensions
- [ ] Parse `.mat` material libraries: contents, material names/types, embedded previews
- [ ] EXIF / color profile / resolution for images
- [ ] Generic metadata sidecar (XMP-style) per asset
- [ ] Custom user-defined attributes & attribute templates

### 2.4 Organizing

- [ ] Tags (multi-level/hierarchical), folders, virtual Collections
- [ ] Star ratings, color labels, descriptions/notes
- [ ] Duplicate detection and merging
- [ ] Drag-and-drop organization, multi-select batch editing
- [ ] Add assets / update-in-place (refresh changed files, keep metadata)

### 2.5 Browsing & Search

- [ ] Instant full-text search (name, tags, description, metadata)
- [ ] Filters (type, tag, rating, date, format, renderer...)
- [ ] Saved searches / smart collections

### 2.6 DCC Integrations (the differentiator)

- [ ] **3ds Max** (most advanced — full parity):
- [ ] Drag-and-drop merge with options
- [ ] Merge-and-place as proxy; object↔proxy replace; xRef objects
- [ ] Align & rotate placement tools
- [ ] "Choose what to merge" sub-object picker
- [ ] Replace scene objects with library assets
- [ ] Load materials from `.mat` / library
- [ ] Save assets from Max → DAM with auto preview generation
- [ ] Update assets in place
- [ ] Manage External Files (MEF): relink missing textures without opening Max
- [ ] Full `.mat` library management (view, rename, copy, move between libraries)
- [ ] **Blender** — merge (append/link), material loading, save-to-DAM, preview gen
- [ ] **Cinema 4D** — merge, materials
- [ ] **SketchUp** — component import, material load
- [ ] **Revit** — family load, material info
- [ ] **Rhino** — import, material/texture handling
- [ ] **Unreal Engine** — FBX/glTF import, material application
- [ ] Fallback everywhere: OS drag-and-drop of the raw file

### 2.7 Team Collaboration (Connecter Suite parity)

- [ ] Shared team workspace over local/network storage
- [ ] Metadata-only sync (binaries stay local)
- [ ] Real-time sync of tags, ratings, previews, collections
- [ ] Roles & granular permissions (view / edit / admin), user management
- [ ] Web catalogs (share visual collections via browser links)
- [ ] Notifications (asset updated, comment, request)
- [ ] Review system: comments, annotations, approvals
- [ ] Version control: history, compare versions, rollback
- [ ] Self-hostable sync server + optional S3/MinIO hybrid storage

---

## 3. Architecture

```javascript
┌─────────────────────────────────────────────────────────┐
│ Desktop App (Tauri + React + TypeScript)                │
│  • Library browser (grid/list, filters, search)         │
│  • 3D viewer (Three.js), shader-ball preview            │
│  • Review/comment UI                                    │
├─────────────────────────────────────────────────────────┤
│ Local Core Service (Rust)                               │
│  • File-system watcher & incremental indexer            │
│  • Preview job queue (worker pool)                      │
│  • SQLite DB: assets, metadata, tags, versions, FTS5    │
├─────────────────────────────────────────────────────────┤
│ Preview/Metadata Engine (Python worker)                 │
│  • assimp / Blender headless / trimesh for geometry     │
│  • OpenImageIO / libvips: PSD, EXR, HDR decode          │
│  • .max/.mat metadata parsers, embedded preview extract │
│  • headless render for thumbnails & turntables          │
├─────────────────────────────────────────────────────────┤
│ DCC Integration Plugins (per host app)                  │
│  • 3ds Max: MAXScript + C# bridge                       │
│  • Blender/C4D/Rhino: Python; SketchUp: Ruby;           │
│    Revit: C#; Unreal: editor plugin                     │
│  • Local WebSocket bridge: DAM ↔ plugins (drag&drop)    │
├─────────────────────────────────────────────────────────┤
│ Team Sync Server (self-hosted, optional)                │
│  • Postgres + CRDT/op-log metadata sync                 │
│  • WebSocket real-time, REST API, web catalog hosting   │
│  • OIDC auth, roles/permissions, S3/MinIO adapters      │
└─────────────────────────────────────────────────────────┘
```

**All dependencies open source:** Tauri, React, Three.js, SQLite/FTS5, Postgres, Automerge-style CRDTs, assimp, Blender headless, OpenImageIO, Rust notify, Tokio.

---

## 4. How Agentic AI Builds This (the actual "agentic" part)

### 4.1 Operating model

- **Human tech lead** = architect + reviewer. Owns the spec repo, reviews agent PRs, merges.
- **Agent workers** = one agent per bounded task, working in isolated branches with a strict contract: *spec → tests → implementation → self-verification → PR*.
- **Orchestrator agent** = maintains the task graph (below), spawns workers, runs CI, and reports blockers to the human.

### 4.2 Spec-driven development (the critical discipline)

Agents fail when specs are vague. Every task ships as a **spec file first**:

```javascript
docs/specs/<id>_<name>.md
  ## Goal            (one sentence, verifiable)
  ## Interface        (exact types / API / schema — the agent must not invent these)
  ## Behavior         (given/when/then acceptance criteria)
  ## Test plan        (unit + integration; golden files where relevant)
  ## Out of scope     (explicitly)
```

Rule: **no code until the spec compiles against existing interfaces.** This is what makes multi-agent work safe — agents integrate through written contracts, not chat context.

### 4.3 Repo layout

```javascript
opendam/
  docs/
    specs/            # one file per task — the agent's source of truth
    architecture/     # RFCs (human-written, versioned)
    adr/              # architecture decision records
  apps/
    desktop/          # Tauri shell + React UI
    sync-server/      # Team server (Docker)
    web-catalogs/     # Shared browser collections
  packages/
    core/             # Rust: indexer, SQLite schema, sync client
    preview-engine/   # Python: thumbnail/3D/HDRI pipeline
    dcc-bridge/       # WebSocket protocol shared by all plugins
    plugin-sdk/       # Types + protocol for DCC plugins
  plugins/
    3dsmax/ blender/ c4d/ sketchup/ rhino/ revit/ unreal/
  tests/
    fixtures/         # golden 3D/textures files for preview tests
    e2e/              # Playwright + synthetic libraries
```

### 4.4 Agent task workflow (per task)

1. Orchestrator assigns spec + context pack (relevant specs, ADRs, interface defs — never the whole repo)
2. Agent writes failing tests from the spec's test plan
3. Agent implements until tests pass; runs lint/typecheck/build
4. Agent self-reviews against acceptance criteria; writes PR with a behavioral summary
5. CI runs full suite + preview-engine golden-image diffs
6. Human reviews (spot-check); orchestrator merges and updates the task graph

### 4.5 CI as the agents' referee

- Unit + integration tests per package; golden-file image comparison for the preview engine (same input → same thumbnail hash within tolerance)
- E2E: Playwright scripts driving a synthetic 10k-asset library (generated fixtures)
- Contract tests on the dcc-bridge protocol (every plugin must pass the same conformance suite)
- Preview-engine conformance matrix: format × expected preview type

---

## 5. Implementation Phases & Milestones

### Phase 0 — Scaffold for agents (weeks 1–2, mostly human)

- Monorepo, CI pipeline, lint/typecheck gates, spec templates, task graph
- Golden test fixtures (small library of .fbx/.obj/.gltf/.psd/.exr/.hdr/.mp4/.mat files committed to repo)
- **Exit:** an agent can be pointed at one spec and produce a passing PR.

### Phase 1 — Core DAM (agent-built, weeks 2–8)

Tasks (each = one spec → one agent):

- SQLite schema + migrations; FTS5 search index
- FS watcher (Rust notify) → incremental index queue
- Asset model + metadata extraction interfaces (stub parsers)
- React grid/list browser; filters; saved searches
- Tag/rating/label/description UI + batch edit
- Preview job queue + cache layer (renders stubbed first)
- **Exit:** browse/search/organize a local folder end-to-end.

### Phase 2 — Preview Engine (weeks 6–14)

- Image pipeline: jpg/png/tga/tif + PSD/EXR/HDR via OpenImageIO → thumbnails
- Video thumbnails (ffmpeg)
- 3D pipeline: assimp → glTF normalization → Three.js turntable; Blender headless fallback for `.blend`/complex files
- `.max`/`.mat` embedded preview + metadata extraction (documented structs; graceful degradation)
- Shader-ball material preview; HDRI preview
- Batch regeneration UI; custom cover images
- **Exit:** preview parity for the top 20 formats; golden-image CI green.

### Phase 3 — DCC Integrations (weeks 12–26, parallel agent tracks)

- dcc-bridge protocol + plugin-sdk first (conformance suite before any plugin)
- 3ds Max plugin (largest task cluster — split into ~10 specs: bridge client, merge, proxies, xRef, align/rotate, sub-object picker, replace-object, material loading, save-to-DAM+MEF, .mat manager)
- Blender plugin; then C4D / SketchUp / Rhino / Revit / Unreal
- Each plugin: implement → pass conformance suite → manual QA checklist recorded as spec
- **Exit:** full Connecter integration parity; 3ds Max depth matched.

### Phase 4 — Team Collaboration (weeks 20–34)

- Sync server (Postgres, OIDC, roles) as its own service with specs
- CRDT/op-log sync client; conflict resolution; reconnect logic
- Real-time UI updates; notifications
- Reviews (comments, annotations, approvals); version history UI
- Web catalogs (Next.js reader of shared collections)
- S3/MinIO hybrid adapter
- **Exit:** 10-person team sync demo, <1s metadata propagation.

### Phase 5 — Hardening & Release (weeks 34–44)

- Performance specs: 50k/100k/500k-asset libraries; search <100ms p95
- Windows/macOS/Linux installers; server hardening docs
- Plugin SDK docs → community plugins (Maya, Houdini, Unity)
- Website, docs, v1.0 governance (open roadmap, RFCs)

---

## 6. Risks & Mitigations

| Risk | Mitigation |
| --- | --- |
| Agents drift from intent on big tasks | Hard 300-line spec cap; split anything bigger; interfaces frozen per phase |
| Integration hell between agent-written packages | Contracts first; conformance suites; contract tests in CI |
| `.max`/`.mat` parsing is reverse-engineered | Extract embedded previews where possible; degrade gracefully; never crash on unknown file |
| Plugin maintenance burden per host app | plugin-sdk + versioned protocol; conformance suite catches breakage |
| Preview engine flakiness | Golden-image CI; multi-backend fallback; stub-first development |
| Agent-generated code quality debt | Mandatory test plan per spec; human review of architecture-affecting PRs only |

---

## 7. Success Metrics

- Phase 1: search p95 <100ms on 50k assets
- Phase 2: preview coverage of top 20 formats; 0 viewer crashes on fuzzed fixture corpus
- Phase 3: all plugins pass conformance suite; 3ds Max workflow video matching Connecter's
- Phase 4: team sync demo with real-time metadata updates
- Process: >90% of tasks merged with ≤1 revision round; CI green on main 100% of the time