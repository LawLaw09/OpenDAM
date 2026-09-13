import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type {
  Asset,
  Library,
  Collection,
  Tag,
  SearchQuery,
  FilterSpec,
  SortField,
  SortOrder,
} from '../types';
import { api } from '../api';

// ── UI State ──────────────────────────────────────────────────────────────
interface UIState {
  viewMode: 'grid' | 'list';
  gridSize: 'sm' | 'md' | 'lg';
  sidebarOpen: boolean;
  detailPanelOpen: boolean;
  selectedAssetIds: Set<string>;
  focusedAssetId: string | null;
  activeLibraryId: string | null;
  activeCollectionId: string | null;

  setViewMode: (mode: 'grid' | 'list') => void;
  setGridSize: (size: 'sm' | 'md' | 'lg') => void;
  toggleSidebar: () => void;
  toggleDetailPanel: () => void;
  selectAsset: (id: string, multi?: boolean) => void;
  selectAll: (ids: string[]) => void;
  clearSelection: () => void;
  focusAsset: (id: string | null) => void;
  setActiveLibrary: (id: string | null) => void;
  setActiveCollection: (id: string | null) => void;
}

export const useUIStore = create<UIState>()(
  persist(
    (set) => ({
      viewMode: 'grid',
      gridSize: 'md',
      sidebarOpen: true,
      detailPanelOpen: true,
      selectedAssetIds: new Set(),
      focusedAssetId: null,
      activeLibraryId: null,
      activeCollectionId: null,

      setViewMode: (viewMode) => set({ viewMode }),
      setGridSize: (gridSize) => set({ gridSize }),
      toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
      toggleDetailPanel: () => set((s) => ({ detailPanelOpen: !s.detailPanelOpen })),

      selectAsset: (id, multi = false) =>
        set((s) => {
          if (multi) {
            const next = new Set(s.selectedAssetIds);
            if (next.has(id)) next.delete(id);
            else next.add(id);
            return { selectedAssetIds: next, focusedAssetId: id };
          }
          return { selectedAssetIds: new Set([id]), focusedAssetId: id };
        }),

      selectAll: (ids) =>
        set({ selectedAssetIds: new Set(ids) }),

      clearSelection: () =>
        set({ selectedAssetIds: new Set(), focusedAssetId: null }),

      focusAsset: (id) => set({ focusedAssetId: id }),
      setActiveLibrary: (id) =>
        set({ activeLibraryId: id, activeCollectionId: null }),
      setActiveCollection: (id) =>
        set({ activeCollectionId: id, activeLibraryId: null }),
    }),
    {
      name: 'opendam-ui',
      partialize: (s) => ({
        viewMode: s.viewMode,
        gridSize: s.gridSize,
        sidebarOpen: s.sidebarOpen,
        detailPanelOpen: s.detailPanelOpen,
      }),
    }
  )
);

// ── Library Store ─────────────────────────────────────────────────────────
interface LibraryStore {
  libraries: Library[];
  loading: boolean;
  fetchLibraries: () => Promise<void>;
  addLibrary: (name: string, rootPaths: string[]) => Promise<void>;
  removeLibrary: (id: string) => Promise<void>;
}

export const useLibraryStore = create<LibraryStore>()((set) => ({
  libraries: [],
  loading: false,

  fetchLibraries: async () => {
    set({ loading: true });
    try {
      const libraries = await api.listLibraries();
      set({ libraries });
    } finally {
      set({ loading: false });
    }
  },

  addLibrary: async (name, rootPaths) => {
    const lib = await api.addLibrary({ name, rootPaths });
    set((s) => ({ libraries: [...s.libraries, lib] }));
  },

  removeLibrary: async (id) => {
    await api.removeLibrary(id);
    set((s) => ({ libraries: s.libraries.filter((l) => l.id !== id) }));
  },
}));

// ── Asset / Search Store ──────────────────────────────────────────────────
interface AssetStore {
  assets: Asset[];
  total: number;
  page: number;
  pageSize: number;
  loading: boolean;
  query: SearchQuery;
  assetMap: Map<string, Asset>;

  setQuery: (q: Partial<SearchQuery>) => void;
  setFilter: (f: Partial<FilterSpec>) => void;
  replaceFilter: (f: FilterSpec, collectionId?: string) => void;
  resetFilter: () => void;
  clearFilter: (key: keyof FilterSpec) => void;
  setSort: (field: SortField, order?: SortOrder) => void;
  setPage: (page: number) => void;
  search: () => Promise<void>;
  patchAsset: (id: string, patch: Partial<Asset>) => Promise<void>;
  updateAssetPreview: (id: string, thumbnailPath?: string, previewModel?: string) => void;
  setPreviewStatus: (id: string, status: Asset['previewStatus']) => void;
  getById: (id: string) => Asset | undefined;
}

const defaultQuery: SearchQuery = {
  text: '',
  filter: {},
  sort: { field: 'date', order: 'desc' },
  includeSubcollections: true,
};

export const useAssetStore = create<AssetStore>()((set, get) => ({
  assets: [],
  total: 0,
  page: 0,
  pageSize: 100,
  loading: false,
  query: defaultQuery,
  assetMap: new Map(),

  setQuery: (q) => set((s) => ({ query: { ...s.query, ...q }, page: 0 })),

  setFilter: (f) =>
    set((s) => ({
      query: { ...s.query, filter: { ...s.query.filter, ...f } },
      page: 0,
    })),

  replaceFilter: (f, collectionId) =>
    set((s) => ({
      query: { ...s.query, collectionId, filter: f },
      page: 0,
    })),

  resetFilter: () =>
    set((s) => ({
      query: { ...s.query, collectionId: undefined, filter: {} },
      page: 0,
    })),

  clearFilter: (key) =>
    set((s) => {
      const filter = { ...s.query.filter };
      delete filter[key];
      return { query: { ...s.query, filter }, page: 0 };
    }),

  setSort: (field, order) =>
    set((s) => {
      const currentOrder = s.query.sort.order;
      return {
        query: {
          ...s.query,
          sort: {
            field,
            order: order ?? (field === s.query.sort.field && currentOrder === 'desc' ? 'asc' : 'desc'),
          },
        },
        page: 0,
      };
    }),

  setPage: (page) => set({ page }),

  search: async () => {
    const { query, page, pageSize } = get();
    set({ loading: true });
    try {
      const result = await api.searchAssets({ query, page, pageSize });
      const assetMap = new Map(result.assets.map((a) => [a.id, a]));
      set({ assets: result.assets, total: result.total, assetMap });
    } finally {
      set({ loading: false });
    }
  },

  patchAsset: async (id, patch) => {
    const updated = await api.updateAsset(id, patch);
    set((s) => ({
      assets: s.assets.map((a) => (a.id === id ? updated : a)),
      assetMap: new Map(s.assetMap).set(id, updated),
    }));
  },

  updateAssetPreview: (id, thumbnailPath, previewModel) => {
    set((s) => {
      const asset = s.assetMap.get(id);
      if (!asset) return s;
      const updated: Asset = {
        ...asset,
        thumbnailPath: thumbnailPath || asset.thumbnailPath,
        previewStatus: 'done',
        metadata: previewModel ? { ...asset.metadata, preview_model: previewModel } : asset.metadata,
      };
      return {
        assets: s.assets.map((a) => (a.id === id ? updated : a)),
        assetMap: new Map(s.assetMap).set(id, updated),
      };
    });
  },

  setPreviewStatus: (id, status) => {
    set((s) => {
      const asset = s.assetMap.get(id);
      if (!asset) return s;
      const updated: Asset = { ...asset, previewStatus: status };
      return {
        assets: s.assets.map((a) => (a.id === id ? updated : a)),
        assetMap: new Map(s.assetMap).set(id, updated),
      };
    });
  },

  getById: (id) => get().assetMap.get(id),
}));

// ── Tag Store ─────────────────────────────────────────────────────────────
interface TagStore {
  tags: Tag[];
  tagMap: Map<string, Tag>;
  fetchTags: () => Promise<void>;
  createTag: (name: string, parentId?: string) => Promise<void>;
  deleteTag: (id: string) => Promise<void>;
}

export const useTagStore = create<TagStore>()((set) => ({
  tags: [],
  tagMap: new Map(),

  fetchTags: async () => {
    const tags = await api.listTags();
    set({ tags, tagMap: new Map(tags.map((t) => [t.id, t])) });
  },

  createTag: async (name, parentId) => {
    const tag = await api.createTag(name, parentId);
    set((s) => ({
      tags: [...s.tags, tag],
      tagMap: new Map(s.tagMap).set(tag.id, tag),
    }));
  },

  deleteTag: async (id) => {
    await api.deleteTag(id);
    set((s) => {
      const tagMap = new Map(s.tagMap);
      tagMap.delete(id);
      return { tags: s.tags.filter((t) => t.id !== id), tagMap };
    });
  },
}));

// ── Collection Store ──────────────────────────────────────────────────────
interface CollectionStore {
  collections: Collection[];
  fetchCollections: () => Promise<void>;
  createCollection: (
    name: string,
    description?: string,
    parentId?: string,
    isSmart?: boolean,
    filterSpec?: string,
  ) => Promise<void>;
  addToCollection: (collectionId: string, assetIds: string[]) => Promise<void>;
  removeFromCollection: (collectionId: string, assetIds: string[]) => Promise<void>;
  deleteCollection: (id: string) => Promise<void>;
}

export const useCollectionStore = create<CollectionStore>()((set) => ({
  collections: [],

  fetchCollections: async () => {
    const collections = await api.listCollections();
    set({ collections });
  },

  createCollection: async (name, description, parentId, isSmart, filterSpec) => {
    const col = await api.createCollection(name, description, parentId, isSmart, filterSpec);
    set((s) => ({ collections: [...s.collections, col] }));
  },

  addToCollection: async (collectionId, assetIds) => {
    await api.addToCollection(collectionId, assetIds);
    set((s) => ({
      collections: s.collections.map((c) =>
        c.id === collectionId
          ? { ...c, assetIds: [...new Set([...c.assetIds, ...assetIds])] }
          : c
      ),
    }));
    useAssetStore.setState((s) => {
      const assets = s.assets.map((a) => {
        if (assetIds.includes(a.id)) {
          const cur = a.collections ?? [];
          return { ...a, collections: [...new Set([...cur, collectionId])] };
        }
        return a;
      });
      const assetMap = new Map(s.assetMap);
      for (const aid of assetIds) {
        const a = assetMap.get(aid);
        if (a) {
          const cur = a.collections ?? [];
          assetMap.set(aid, { ...a, collections: [...new Set([...cur, collectionId])] });
        }
      }
      return { assets, assetMap };
    });
  },

  removeFromCollection: async (collectionId, assetIds) => {
    await api.removeFromCollection(collectionId, assetIds);
    set((s) => ({
      collections: s.collections.map((c) =>
        c.id === collectionId
          ? { ...c, assetIds: c.assetIds.filter((id) => !assetIds.includes(id)) }
          : c
      ),
    }));
    useAssetStore.setState((s) => {
      const assets = s.assets.map((a) => {
        if (assetIds.includes(a.id)) {
          const cur = a.collections ?? [];
          return { ...a, collections: cur.filter((cid) => cid !== collectionId) };
        }
        return a;
      });
      const assetMap = new Map(s.assetMap);
      for (const aid of assetIds) {
        const a = assetMap.get(aid);
        if (a) {
          const cur = a.collections ?? [];
          assetMap.set(aid, { ...a, collections: cur.filter((cid) => cid !== collectionId) });
        }
      }
      return { assets, assetMap };
    });
  },

  deleteCollection: async (id) => {
    await api.deleteCollection(id);
    set((s) => ({ collections: s.collections.filter((c) => c.id !== id) }));
  },
}));
