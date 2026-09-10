# SPEC-001: SQLite Schema + Full-Text Search

## Goal
Define and implement the SQLite schema (tables, indexes, FTS5) for all Phase 1 entities so that any Phase 1 agent can read/write assets, tags, collections, and libraries.

## Interface

```typescript
// Core entities (matches Rust models.rs and TypeScript types/index.ts)
interface Asset { id, filePath, fileName, extension, kind, sizeBytes,
                  modifiedAt, indexedAt, rating, colorLabel, description,
                  tags, thumbnailPath, previewStatus, metadata }
interface Library { id, name, rootPaths, createdAt, assetCount }
interface Tag      { id, name, color?, parentId? }
interface Collection { id, name, description, assetIds, isSmart, filterSpec?, createdAt }
```

## Behavior
- **Given** the app starts for the first time  
  **When** it opens the SQLite DB  
  **Then** all tables exist and all indexes are created (migration is idempotent)
- **Given** an asset is inserted  
  **When** queried via FTS5 by file name substring  
  **Then** the asset is returned (latency < 50 ms on 50k rows)
- **Given** an asset is deleted  
  **When** FTS index is checked  
  **Then** the deleted record is removed from FTS via trigger

## Test Plan
- [x] Migration runs without error on fresh DB
- [x] Migration is idempotent (run twice → no error)
- [ ] FTS returns correct results for prefix matches
- [ ] FTS trigger correctly removes stale entries

## Out of Scope
- Migrations > 001 (handled by future specs)
- Query performance benchmarks (Phase 5)

## Implementation Notes
- Schema file: `apps/desktop/src-tauri/migrations/001_initial.sql`
- Read by `db.rs` at startup via `include_str!`
- FTS5 sync is trigger-based to avoid manual bookkeeping
