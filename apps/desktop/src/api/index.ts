import { invoke } from '@tauri-apps/api/core';
import type {
  Asset,
  Library,
  Collection,
  Tag,
  AddLibraryPayload,
  SearchPayload,
  SearchResult,
  PreviewJob,
} from '../types';

/** Thin wrapper around Tauri IPC commands */
export const api = {
  // Libraries
  addLibrary: (payload: AddLibraryPayload) =>
    invoke<Library>('add_library', { payload }),
  listLibraries: () => invoke<Library[]>('list_libraries'),
  removeLibrary: (id: string) => invoke<void>('remove_library', { id }),
  refreshLibrary: (id: string) => invoke<void>('refresh_library', { id }),
  refreshAllLibraries: () => invoke<void>('refresh_all_libraries'),

  // Assets
  searchAssets: (payload: SearchPayload) =>
    invoke<SearchResult>('search_assets', { payload }),
  getAsset: (id: string) => invoke<Asset>('get_asset', { id }),
  updateAsset: (id: string, patch: Partial<Asset>) =>
    invoke<Asset>('update_asset', { id, patch }),

  // Tags
  listTags: () => invoke<Tag[]>('list_tags'),
  createTag: (name: string, parentId?: string) =>
    invoke<Tag>('create_tag', { name, parentId }),
  deleteTag: (id: string) => invoke<void>('delete_tag', { id }),

  // Collections
  listCollections: () => invoke<Collection[]>('list_collections'),
  createCollection: (name: string, description?: string, parentId?: string) =>
    invoke<Collection>('create_collection', { name, description, parentId }),
  addToCollection: (collectionId: string, assetIds: string[]) =>
    invoke<void>('add_to_collection', { collectionId, assetIds }),
  removeFromCollection: (collectionId: string, assetIds: string[]) =>
    invoke<void>('remove_from_collection', { collectionId, assetIds }),
  deleteCollection: (id: string) =>
    invoke<void>('delete_collection', { id }),

  // Preview
  requestPreview: (assetIds: string[]) =>
    invoke<void>('request_preview', { assetIds }),
  getPreviewJobs: () => invoke<PreviewJob[]>('get_preview_jobs'),

  // Reveal in explorer / open with default app
  revealInExplorer: (filePath: string) =>
    invoke<void>('reveal_in_explorer', { filePath }),
  openWithDefault: (filePath: string) =>
    invoke<void>('open_with_default', { filePath }),
};
