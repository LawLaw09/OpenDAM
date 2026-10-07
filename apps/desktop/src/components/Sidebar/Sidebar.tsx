import { useState } from 'react';
import clsx from 'clsx';
import { open } from '@tauri-apps/plugin-dialog';
import {
  useLibraryStore, useCollectionStore, useTagStore,
  useUIStore, useAssetStore,
} from '../../store';
import type { FilterSpec, ColorLabel } from '../../types';
import {
  IconFolder, IconCollection, IconTag,
  IconChevronDown, IconChevronRight, IconPlus,
  IconCheck, IconX, IconTrash, IconInbox, IconFilter,
  IconHeartFilled, IconStar, IconLayers, IconModel3D,
  IconImage, IconVideo, IconEye,
} from '../Icons';
import { ConfirmDeleteDialog } from '../ConfirmDeleteDialog';
import { SmartFilterModal } from '../SmartFilterModal';
import styles from './Sidebar.module.css';

const SIDEBAR_COLOR_LABELS: { label: ColorLabel; name: string; hex: string }[] = [
  { label: 'red', name: 'Red', hex: '#ef4444' },
  { label: 'orange', name: 'Orange', hex: '#f97316' },
  { label: 'yellow', name: 'Yellow', hex: '#eab308' },
  { label: 'green', name: 'Green', hex: '#22c55e' },
  { label: 'teal', name: 'Teal', hex: '#14b8a6' },
  { label: 'blue', name: 'Blue', hex: '#3b82f6' },
  { label: 'purple', name: 'Purple', hex: '#a855f7' },
  { label: 'pink', name: 'Pink', hex: '#ec4899' },
];

const getStartOfDay = () => {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime();
};

export function Sidebar() {
  const libraries = useLibraryStore((s) => s.libraries);
  const addLibrary = useLibraryStore((s) => s.addLibrary);
  const removeLibrary = useLibraryStore((s) => s.removeLibrary);

  const collections = useCollectionStore((s) => s.collections);
  const createCollection = useCollectionStore((s) => s.createCollection);
  const deleteCollection = useCollectionStore((s) => s.deleteCollection);
  const addToCollection = useCollectionStore((s) => s.addToCollection);

  const tags = useTagStore((s) => s.tags);
  const createTag = useTagStore((s) => s.createTag);
  const deleteTag = useTagStore((s) => s.deleteTag);

  const {
    activeLibraryId,
    selectedLibraryIds,
    activeCollectionId,
    setActiveLibrary,
    setActiveCollection,
    toggleSelectedLibrary,
    clearLibrarySelection,
  } = useUIStore();
  const replaceFilter = useAssetStore((s) => s.replaceFilter);
  const setFilter = useAssetStore((s) => s.setFilter);
  const resetFilter = useAssetStore((s) => s.resetFilter);
  const clearFilter = useAssetStore((s) => s.clearFilter);
  const setQuery = useAssetStore((s) => s.setQuery);
  const currentFilter = useAssetStore((s) => s.query.filter);
  const activeTagId = useAssetStore((s) => s.query.filter.tags?.[0] ?? null);
  const search = useAssetStore((s) => s.search);

  const [filterSubExpanded, setFilterSubExpanded] = useState({
    types: true,
    ratings: true,
    colors: true,
    dateAdded: false,
    fileSize: false,
    previewStatus: false,
  });

  const toggleFilterSub = (key: keyof typeof filterSubExpanded) =>
    setFilterSubExpanded((prev) => ({ ...prev, [key]: !prev[key] }));

  const [expanded, setExpanded] = useState({
    libraries: true,
    collections: true,
    tags: true,
    smartFilters: true,
    filters: true,
  });

  const [activeSmartFilterId, setActiveSmartFilterId] = useState<string | null>(null);
  const [isCreatingSmartFilter, setIsCreatingSmartFilter] = useState(false);

  // Inline creation states
  const [isCreatingTag, setIsCreatingTag] = useState(false);
  const [newTagName, setNewTagName] = useState('');
  const [tagError, setTagError] = useState<string | null>(null);

  const [isCreatingCollection, setIsCreatingCollection] = useState(false);
  const [parentCollectionId, setParentCollectionId] = useState<string | null>(null);
  const [newCollectionName, setNewCollectionName] = useState('');
  const [collectionError, setCollectionError] = useState<string | null>(null);

  // Track expanded/collapsed state for collections that have subcollections
  const [expandedColIds, setExpandedColIds] = useState<Record<string, boolean>>({});

  const toggleCollectionExpanded = (id: string, e?: React.MouseEvent) => {
    if (e) e.stopPropagation();
    setExpandedColIds((prev) => ({ ...prev, [id]: !prev[id] }));
  };

  // Dialog state for confirm delete
  const [deleteTarget, setDeleteTarget] = useState<{
    id: string;
    name: string;
    type: 'tag' | 'collection' | 'library';
  } | null>(null);

  const toggle = (key: keyof typeof expanded) =>
    setExpanded((e) => ({ ...e, [key]: !e[key] }));

  const handleAddLibrary = async () => {
    try {
      const selectedPath = await open({
        directory: true,
        multiple: false,
        title: 'Select Library Folder',
      });
      if (selectedPath && typeof selectedPath === 'string') {
        const name = selectedPath.split(/[/\\]/).filter(Boolean).pop() || 'New Library';
        await addLibrary(name, [selectedPath]);
        search();
      }
    } catch (err) {
      console.error('Failed to add library:', err);
    }
  };

  const handleCreateTag = async () => {
    const name = newTagName.trim();
    if (!name) {
      setIsCreatingTag(false);
      setTagError(null);
      return;
    }

    const isDuplicate = tags.some(
      (t) => t.name.trim().toLowerCase() === name.toLowerCase()
    );
    if (isDuplicate) {
      setTagError(`Tag "${name}" already exists`);
      return;
    }

    try {
      await createTag(name);
      setNewTagName('');
      setTagError(null);
      setIsCreatingTag(false);
    } catch (err) {
      console.error('Failed to create tag:', err);
      setTagError('Failed to create tag');
    }
  };

  const handleCreateCollection = async () => {
    const name = newCollectionName.trim();
    if (!name) {
      setIsCreatingCollection(false);
      setParentCollectionId(null);
      setCollectionError(null);
      return;
    }

    // Check duplicates under the same parent (or root)
    const isDuplicate = collections.some(
      (c) =>
        (c.parentId ?? null) === (parentCollectionId ?? null) &&
        c.name.trim().toLowerCase() === name.toLowerCase()
    );
    if (isDuplicate) {
      setCollectionError(`Collection "${name}" already exists here`);
      return;
    }

    try {
      await createCollection(name, undefined, parentCollectionId ?? undefined);
      if (parentCollectionId) {
        setExpandedColIds((prev) => ({ ...prev, [parentCollectionId]: true }));
      }
      setNewCollectionName('');
      setParentCollectionId(null);
      setCollectionError(null);
      setIsCreatingCollection(false);
    } catch (err) {
      console.error('Failed to create collection:', err);
      setCollectionError('Failed to create collection');
    }
  };

  const handleAllLibrariesClick = () => {
    setActiveSmartFilterId(null);
    setActiveCollection(null);
    clearLibrarySelection();
    setQuery({
      libraryIds: undefined,
      libraryId: undefined,
      collectionId: undefined,
    });
    search();
  };

  const handleLibraryClick = (id: string, e?: React.MouseEvent) => {
    setActiveSmartFilterId(null);
    setActiveCollection(null);
    const isCheckbox = (e?.target as HTMLElement)?.tagName?.toLowerCase() === 'input';
    const isMulti = e ? Boolean(e.metaKey || e.ctrlKey || e.shiftKey || isCheckbox) : false;
    toggleSelectedLibrary(id, isMulti);
    const nextSelected = useUIStore.getState().selectedLibraryIds;
    setQuery({
      libraryIds: nextSelected.length > 0 ? nextSelected : undefined,
      libraryId: nextSelected.length === 1 ? nextSelected[0] : undefined,
      collectionId: undefined,
    });
    search();
  };

  const handleCollectionClick = (id: string) => {
    setActiveSmartFilterId(null);
    resetFilter();
    if (activeCollectionId === id) {
      setActiveCollection(null);
      setQuery({ collectionId: undefined });
    } else {
      setActiveCollection(id);
      setQuery({ collectionId: id, libraryId: undefined });
    }
    search();
  };

  const handleSmartFilterClick = (col: (typeof collections)[0]) => {
    setActiveCollection(null);
    if (activeSmartFilterId === col.id) {
      setActiveSmartFilterId(null);
      resetFilter();
      search();
    } else {
      setActiveSmartFilterId(col.id);
      try {
        const spec: FilterSpec = col.filterSpec ? JSON.parse(col.filterSpec) : {};
        replaceFilter(spec);
        search();
      } catch (err) {
        console.error('Failed to parse smart filter spec:', err);
      }
    }
  };

  const handleTagFilter = (tagId: string) => {
    setActiveSmartFilterId(null);
    setActiveCollection(null);
    if (activeTagId === tagId) {
      resetFilter();
    } else {
      replaceFilter({ tags: [tagId] });
    }
    search();
  };

  const handleDeleteTag = (e: React.MouseEvent, id: string, name: string) => {
    e.stopPropagation();
    setDeleteTarget({ id, name, type: 'tag' });
  };

  const handleDeleteCollection = (e: React.MouseEvent, id: string, name: string) => {
    e.stopPropagation();
    setDeleteTarget({ id, name, type: 'collection' });
  };

  const handleRemoveLibrary = (e: React.MouseEvent, id: string, name: string) => {
    e.stopPropagation();
    setDeleteTarget({ id, name, type: 'library' });
  };

  const executeDeleteTarget = async () => {
    if (!deleteTarget) return;
    const { id, type } = deleteTarget;
    setDeleteTarget(null);

    try {
      if (type === 'tag') {
        await deleteTag(id);
        if (activeTagId === id) {
          clearFilter('tags');
          search();
        }
      } else if (type === 'collection') {
        await deleteCollection(id);
        if (activeCollectionId === id) {
          setActiveCollection(null);
          setQuery({ collectionId: undefined });
          search();
        }
      } else if (type === 'library') {
        await removeLibrary(id);
        if (activeLibraryId === id) {
          setActiveLibrary(null);
          setQuery({ libraryId: undefined });
        }
        search();
      }
    } catch (err) {
      console.error(`Failed to delete ${type}:`, err);
    }
  };

  const handleDropOnCollection = async (e: React.DragEvent, colId: string) => {
    e.preventDefault();
    const rawData = e.dataTransfer.getData('application/opendam-asset-id') || e.dataTransfer.getData('text/plain');
    if (!rawData) return;

    let assetId = rawData;
    if (rawData.includes('/') || rawData.includes('\\')) {
      const norm = (p: string) => p.replace(/\\/g, '/').toLowerCase();
      const match = useAssetStore.getState().assets.find((a) => norm(a.filePath) === norm(rawData));
      if (match) {
        assetId = match.id;
      }
    }

    try {
      await addToCollection(colId, [assetId]);
      await useCollectionStore.getState().fetchCollections();
      const curAsset = useAssetStore.getState().assetMap.get(assetId);
      if (curAsset) {
        const updatedCols = Array.from(new Set([...(curAsset.collections || []), colId]));
        useAssetStore.getState().patchAsset(assetId, { collections: updatedCols });
      }
    } catch (err) {
      console.error('Failed to add asset to collection:', err);
    }
  };

  const renderCollectionTree = (parentId: string | null = null, depth: number = 0) => {
    const items = collections.filter((c) => !c.isSmart && (c.parentId ?? null) === parentId);
    if (items.length === 0 && (!isCreatingCollection || parentCollectionId !== parentId)) {
      return null;
    }

    return (
      <div className={depth > 0 ? styles.subTree : undefined}>
        {items.map((col) => {
          const hasChildren = collections.some((c) => c.parentId === col.id);
          const isExpanded = expandedColIds[col.id] ?? false;
          const isCreatingSub = isCreatingCollection && parentCollectionId === col.id;

          return (
            <div key={col.id}>
              <div
                id={`col-${col.id}`}
                className={clsx(styles.item, activeCollectionId === col.id && styles.active)}
                onClick={() => handleCollectionClick(col.id)}
                onDragOver={(e) => {
                  e.preventDefault();
                  e.dataTransfer.dropEffect = 'copy';
                }}
                onDrop={(e) => handleDropOnCollection(e, col.id)}
                title="Click to view; drag assets here to add"
                role="button"
                tabIndex={0}
              >
                {hasChildren ? (
                  <span
                    className={styles.chevronBtn}
                    onClick={(e) => toggleCollectionExpanded(col.id, e)}
                    title={isExpanded ? 'Collapse subcollections' : 'Expand subcollections'}
                  >
                    {isExpanded ? <IconChevronDown size={11} /> : <IconChevronRight size={11} />}
                  </span>
                ) : (
                  <IconCollection size={13} />
                )}

                <span className={clsx(styles.itemName, 'truncate')}>{col.name}</span>
                <span className={styles.badge}>{col.assetIds.length}</span>

                <div className={styles.itemActions}>
                  <span
                    className={styles.itemActionBtn}
                    onClick={(e) => {
                      e.stopPropagation();
                      setParentCollectionId(col.id);
                      setNewCollectionName('');
                      setCollectionError(null);
                      setIsCreatingCollection(true);
                      setExpandedColIds((prev) => ({ ...prev, [col.id]: true }));
                    }}
                    title="Add subcollection"
                  >
                    <IconPlus size={11} />
                  </span>
                  <span
                    className={clsx(styles.itemActionBtn, styles.itemActionBtnDanger)}
                    onClick={(e) => handleDeleteCollection(e, col.id, col.name)}
                    title="Delete collection"
                  >
                    <IconTrash size={11} />
                  </span>
                </div>
              </div>

              {/* Inline input for new subcollection inside this item */}
              {isCreatingSub && (
                <div className={clsx(styles.createFormWrap, styles.subTree)}>
                  <form
                    className={styles.createForm}
                    onSubmit={(e) => {
                      e.preventDefault();
                      handleCreateCollection();
                    }}
                  >
                    <input
                      type="text"
                      autoFocus
                      placeholder="Subcollection name…"
                      className={styles.createInput}
                      value={newCollectionName}
                      onChange={(e) => {
                        setNewCollectionName(e.target.value);
                        if (collectionError) setCollectionError(null);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === 'Escape') {
                          setIsCreatingCollection(false);
                          setParentCollectionId(null);
                          setNewCollectionName('');
                          setCollectionError(null);
                        }
                      }}
                    />
                    <button
                      type="submit"
                      className={clsx(styles.createBtn, styles.createBtnConfirm)}
                      title="Save subcollection"
                    >
                      <IconCheck size={12} />
                    </button>
                    <button
                      type="button"
                      className={clsx(styles.createBtn, styles.createBtnCancel)}
                      onClick={() => {
                        setIsCreatingCollection(false);
                        setParentCollectionId(null);
                        setNewCollectionName('');
                        setCollectionError(null);
                      }}
                      title="Cancel"
                    >
                      <IconX size={12} />
                    </button>
                  </form>
                  {collectionError && <span className={styles.formError}>{collectionError}</span>}
                </div>
              )}

              {/* Render subcollection children if expanded */}
              {isExpanded && renderCollectionTree(col.id, depth + 1)}
            </div>
          );
        })}
      </div>
    );
  };

  return (
    <aside className={styles.sidebar} id="sidebar">
      {/* ── Libraries ── */}
      <SidebarSection
        label="Libraries"
        icon={<IconFolder size={14} />}
        expanded={expanded.libraries}
        onToggle={() => toggle('libraries')}
        onAdd={handleAddLibrary}
      >
        {libraries.length === 0 && (
          <p className={styles.empty}>No libraries yet</p>
        )}
        {libraries.length > 1 && (
          <div
            id="lib-all"
            className={clsx(styles.item, selectedLibraryIds.length === 0 && styles.active)}
            onClick={handleAllLibrariesClick}
            role="button"
            tabIndex={0}
            title="Search across all libraries (none selected)"
          >
            <IconFolder size={13} />
            <span className={clsx(styles.itemName, 'truncate')} style={{ fontWeight: selectedLibraryIds.length === 0 ? 600 : 400 }}>
              All Libraries
            </span>
            <span className={styles.badge}>
              {libraries.reduce((sum, l) => sum + (l.assetCount || 0), 0).toLocaleString()}
            </span>
          </div>
        )}
        {libraries.map((lib) => {
          const isSelected = selectedLibraryIds.includes(lib.id);
          return (
            <div
              key={lib.id}
              id={`lib-${lib.id}`}
              className={clsx(styles.item, isSelected && styles.active)}
              onClick={(e) => handleLibraryClick(lib.id, e)}
              role="button"
              tabIndex={0}
              title="Click to select library; Ctrl+Click to select multiple"
            >
              {libraries.length > 1 && (
                <input
                  type="checkbox"
                  checked={isSelected}
                  onClick={(e) => {
                    e.stopPropagation();
                    handleLibraryClick(lib.id, e);
                  }}
                  onChange={() => {}}
                  style={{
                    width: 13,
                    height: 13,
                    accentColor: 'var(--color-brand-500)',
                    cursor: 'pointer',
                    marginRight: 6,
                  }}
                  title="Toggle this library in filter"
                />
              )}
              <IconFolder size={13} />
              <span className={clsx(styles.itemName, 'truncate')}>{lib.name}</span>
              <span className={styles.badge}>{(lib.assetCount || 0).toLocaleString()}</span>
              <span
                className={clsx(styles.itemActionBtn, styles.itemActionBtnDanger)}
                onClick={(e) => handleRemoveLibrary(e, lib.id, lib.name)}
                title="Remove library"
                role="button"
                tabIndex={0}
              >
                <IconTrash size={11} />
              </span>
            </div>
          );
        })}
      </SidebarSection>

      {/* ── Collections ── */}
      <SidebarSection
        label="Collections"
        icon={<IconCollection size={14} />}
        expanded={expanded.collections}
        onToggle={() => toggle('collections')}
        onAdd={() => {
          setExpanded((e) => ({ ...e, collections: true }));
          setIsCreatingCollection(true);
        }}
      >
        {/* Inline Create Root Collection Input */}
        {isCreatingCollection && parentCollectionId === null && (
          <div className={styles.createFormWrap}>
            <form
              className={styles.createForm}
              onSubmit={(e) => {
                e.preventDefault();
                handleCreateCollection();
              }}
            >
              <input
                type="text"
                autoFocus
                placeholder="Collection name…"
                className={styles.createInput}
                value={newCollectionName}
                onChange={(e) => {
                  setNewCollectionName(e.target.value);
                  if (collectionError) setCollectionError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') {
                    setIsCreatingCollection(false);
                    setParentCollectionId(null);
                    setNewCollectionName('');
                    setCollectionError(null);
                  }
                }}
              />
              <button
                type="submit"
                className={clsx(styles.createBtn, styles.createBtnConfirm)}
                title="Save collection"
              >
                <IconCheck size={12} />
              </button>
              <button
                type="button"
                className={clsx(styles.createBtn, styles.createBtnCancel)}
                onClick={() => {
                  setIsCreatingCollection(false);
                  setParentCollectionId(null);
                  setNewCollectionName('');
                  setCollectionError(null);
                }}
                title="Cancel"
              >
                <IconX size={12} />
              </button>
            </form>
            {collectionError && <span className={styles.formError}>{collectionError}</span>}
          </div>
        )}

        {collections.length === 0 && !isCreatingCollection && (
          <p className={styles.empty}>No collections yet</p>
        )}

        {/* Recursive Tree renderer for collections */}
        {renderCollectionTree(null)}
      </SidebarSection>

      {/* ── Tags ── */}
      <SidebarSection
        label="Tags"
        icon={<IconTag size={14} />}
        expanded={expanded.tags}
        onToggle={() => toggle('tags')}
        onAdd={() => {
          setExpanded((e) => ({ ...e, tags: true }));
          setIsCreatingTag(true);
        }}
      >
        {/* Inline Create Tag Input */}
        {isCreatingTag && (
          <div className={styles.createFormWrap}>
            <form
              className={styles.createForm}
              onSubmit={(e) => {
                e.preventDefault();
                handleCreateTag();
              }}
            >
              <input
                type="text"
                autoFocus
                placeholder="Tag name…"
                className={styles.createInput}
                value={newTagName}
                onChange={(e) => {
                  setNewTagName(e.target.value);
                  if (tagError) setTagError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === 'Escape') {
                    setIsCreatingTag(false);
                    setNewTagName('');
                    setTagError(null);
                  }
                }}
              />
              <button
                type="submit"
                className={clsx(styles.createBtn, styles.createBtnConfirm)}
                title="Save tag"
              >
                <IconCheck size={12} />
              </button>
              <button
                type="button"
                className={clsx(styles.createBtn, styles.createBtnCancel)}
                onClick={() => {
                  setIsCreatingTag(false);
                  setNewTagName('');
                  setTagError(null);
                }}
                title="Cancel"
              >
                <IconX size={12} />
              </button>
            </form>
            {tagError && <span className={styles.formError}>{tagError}</span>}
          </div>
        )}

        {tags.length === 0 && !isCreatingTag && (
          <p className={styles.empty}>No tags yet</p>
        )}
        {tags.map((tag) => {
          const isActiveFilter = activeTagId === tag.id;
          return (
            <div
              key={tag.id}
              id={`tag-${tag.id}`}
              className={clsx(styles.item, isActiveFilter && styles.active)}
              onClick={() => handleTagFilter(tag.id)}
              title={isActiveFilter ? 'Click to clear filter' : 'Click to filter by tag'}
              role="button"
              tabIndex={0}
            >
              <span
                className={styles.tagDot}
                style={{ background: tag.color ? `var(--swatch-${tag.color})` : 'hsl(220 85% 60%)' }}
              />
              <span className={clsx(styles.itemName, 'truncate')}>{tag.name}</span>
              {isActiveFilter && (
                <span className={styles.filterActiveChip}>●</span>
              )}
              <span
                className={clsx(styles.itemActionBtn, styles.itemActionBtnDanger)}
                onClick={(e) => handleDeleteTag(e, tag.id, tag.name)}
                title="Delete tag"
                role="button"
                tabIndex={0}
              >
                <IconTrash size={11} />
              </span>
            </div>
          );
        })}
      </SidebarSection>

      {/* ── Smart Filters ── */}
      <SidebarSection
        label="Smart Filters"
        icon={<IconFilter size={14} />}
        expanded={expanded.smartFilters}
        onToggle={() => toggle('smartFilters')}
        onAdd={() => setIsCreatingSmartFilter(true)}
      >
        {collections.filter((c) => c.isSmart).length === 0 && (
          <p className={styles.empty}>No smart filters yet</p>
        )}
        {collections
          .filter((c) => c.isSmart)
          .map((col) => {
            const isActive = activeSmartFilterId === col.id;
            return (
              <div
                key={col.id}
                id={`smart-filter-${col.id}`}
                className={clsx(styles.item, isActive && styles.active)}
                onClick={() => handleSmartFilterClick(col)}
                role="button"
                tabIndex={0}
              >
                <IconFilter size={13} />
                <span className={clsx(styles.itemName, 'truncate')}>{col.name}</span>
                {isActive && <span className={styles.filterActiveChip}>●</span>}
                <span
                  className={clsx(styles.itemActionBtn, styles.itemActionBtnDanger)}
                  onClick={(e) => handleDeleteCollection(e, col.id, col.name)}
                  title="Delete smart filter"
                  role="button"
                  tabIndex={0}
                >
                  <IconTrash size={11} />
                </span>
              </div>
            );
          })}
      </SidebarSection>

      {/* ── Quick & Advanced Filters ── */}
      <SidebarSection
        label="Filters"
        icon={<IconFilter size={14} />}
        expanded={expanded.filters}
        onToggle={() => toggle('filters')}
      >
        {/* Core Status Filters */}
        {(() => {
          const isAllActive =
            !currentFilter.isFavorite &&
            !currentFilter.unorganized &&
            (!currentFilter.kinds || currentFilter.kinds.length === 0) &&
            !currentFilter.rating &&
            (!currentFilter.colorLabels || currentFilter.colorLabels.length === 0) &&
            !currentFilter.dateRange &&
            !currentFilter.sizeRange &&
            !currentFilter.previewStatus &&
            (!currentFilter.tags || currentFilter.tags.length === 0);
          const isFavActive = Boolean(currentFilter.isFavorite);
          const isUnorgActive = Boolean(currentFilter.unorganized);

          return (
            <>
              <button
                type="button"
                id="sidebar-filter-all"
                className={clsx(styles.item, isAllActive && styles.active)}
                onClick={() => {
                  setActiveSmartFilterId(null);
                  setActiveCollection(null);
                  resetFilter();
                  search();
                }}
              >
                <span className={clsx(styles.itemName, 'truncate')}>All assets</span>
              </button>

              <button
                type="button"
                id="sidebar-filter-favorites"
                className={clsx(styles.item, isFavActive && styles.favoriteActive)}
                onClick={() => {
                  setActiveSmartFilterId(null);
                  setActiveCollection(null);
                  if (isFavActive) {
                    clearFilter('isFavorite');
                  } else {
                    setFilter({ isFavorite: true });
                  }
                  search();
                }}
              >
                <span className={styles.favoriteIcon}>
                  <IconHeartFilled size={13} />
                </span>
                <span className={clsx(styles.itemName, 'truncate')}>Favorites</span>
                {isFavActive && <span className={styles.filterActiveChip}>●</span>}
              </button>

              <button
                type="button"
                id="sidebar-filter-unorganized"
                className={clsx(styles.item, isUnorgActive && styles.active)}
                onClick={() => {
                  setActiveSmartFilterId(null);
                  setActiveCollection(null);
                  if (isUnorgActive) {
                    clearFilter('unorganized');
                  } else {
                    setFilter({ unorganized: true });
                  }
                  search();
                }}
              >
                <IconInbox size={13} />
                <span className={clsx(styles.itemName, 'truncate')}>Unorganized</span>
                {isUnorgActive && <span className={styles.filterActiveChip}>●</span>}
              </button>
            </>
          );
        })()}

        {/* ── Subgroup: Types ── */}
        <div className={styles.subGroup}>
          <button
            type="button"
            className={styles.subGroupHeader}
            onClick={() => toggleFilterSub('types')}
          >
            <span className={styles.subGroupTitle}>
              {filterSubExpanded.types ? <IconChevronDown size={10} /> : <IconChevronRight size={10} />}
              <span>Types</span>
              {currentFilter.kinds && currentFilter.kinds.length > 0 && (
                <span className={styles.filterActiveChip}>●</span>
              )}
            </span>
            {currentFilter.kinds && currentFilter.kinds.length > 0 && (
              <span
                role="button"
                tabIndex={0}
                className={styles.clearSubFilterBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  clearFilter('kinds');
                  search();
                }}
              >
                Clear
              </span>
            )}
          </button>
          {filterSubExpanded.types && (
            <div className={styles.subGroupBody}>
              {([
                { kind: '3d_model' as const, label: '3D Models', icon: <IconModel3D size={13} /> },
                { kind: 'material' as const, label: 'Materials', icon: <IconLayers size={13} /> },
                { kind: 'texture' as const, label: 'Textures', icon: <IconImage size={13} /> },
                { kind: 'hdri' as const, label: 'HDRI', icon: <IconImage size={13} /> },
                { kind: 'image' as const, label: 'Images', icon: <IconImage size={13} /> },
                { kind: 'video' as const, label: 'Video', icon: <IconVideo size={13} /> },
              ]).map(({ kind, label, icon }) => {
                const isActive = currentFilter.kinds?.includes(kind);
                return (
                  <button
                    key={kind}
                    type="button"
                    id={`sidebar-filter-type-${kind}`}
                    className={clsx(styles.item, isActive && styles.active)}
                    onClick={() => {
                      setActiveSmartFilterId(null);
                      setActiveCollection(null);
                      if (isActive) {
                        clearFilter('kinds');
                      } else {
                        setFilter({ kinds: [kind] });
                      }
                      search();
                    }}
                  >
                    {icon}
                    <span className={clsx(styles.itemName, 'truncate')}>{label}</span>
                    {isActive && <span className={styles.filterActiveChip}>●</span>}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* ── Subgroup: Ratings ── */}
        <div className={styles.subGroup}>
          <button
            type="button"
            className={styles.subGroupHeader}
            onClick={() => toggleFilterSub('ratings')}
          >
            <span className={styles.subGroupTitle}>
              {filterSubExpanded.ratings ? <IconChevronDown size={10} /> : <IconChevronRight size={10} />}
              <span>Ratings</span>
              {currentFilter.rating && <span className={styles.filterActiveChip}>●</span>}
            </span>
            {currentFilter.rating && (
              <span
                role="button"
                tabIndex={0}
                className={styles.clearSubFilterBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  clearFilter('rating');
                  search();
                }}
              >
                Clear
              </span>
            )}
          </button>
          {filterSubExpanded.ratings && (
            <div className={styles.subGroupBody}>
              {[
                { label: '5 Stars', min: 5, max: 5 },
                { label: '4+ Stars', min: 4, max: 5 },
                { label: '3+ Stars', min: 3, max: 5 },
                { label: '1+ Stars', min: 1, max: 5 },
              ].map(({ label, min, max }) => {
                const isActive = currentFilter.rating?.min === min && currentFilter.rating?.max === max;
                return (
                  <button
                    key={label}
                    type="button"
                    id={`sidebar-filter-rating-${min}`}
                    className={clsx(styles.item, isActive && styles.active)}
                    onClick={() => {
                      setActiveSmartFilterId(null);
                      setActiveCollection(null);
                      if (isActive) {
                        clearFilter('rating');
                      } else {
                        setFilter({ rating: { min, max } });
                      }
                      search();
                    }}
                  >
                    <IconStar size={12} className={styles.starActive} />
                    <span className={clsx(styles.itemName, 'truncate')}>{label}</span>
                    {isActive && <span className={styles.filterActiveChip}>●</span>}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* ── Subgroup: Color Labels ── */}
        <div className={styles.subGroup}>
          <button
            type="button"
            className={styles.subGroupHeader}
            onClick={() => toggleFilterSub('colors')}
          >
            <span className={styles.subGroupTitle}>
              {filterSubExpanded.colors ? <IconChevronDown size={10} /> : <IconChevronRight size={10} />}
              <span>Color Labels</span>
              {currentFilter.colorLabels && currentFilter.colorLabels.length > 0 && (
                <span className={styles.filterActiveChip}>●</span>
              )}
            </span>
            {currentFilter.colorLabels && currentFilter.colorLabels.length > 0 && (
              <span
                role="button"
                tabIndex={0}
                className={styles.clearSubFilterBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  clearFilter('colorLabels');
                  search();
                }}
              >
                Clear
              </span>
            )}
          </button>
          {filterSubExpanded.colors && (
            <div className={styles.sidebarColorRow}>
              {SIDEBAR_COLOR_LABELS.map(({ label, name, hex }) => {
                const isActive = currentFilter.colorLabels?.includes(label);
                return (
                  <button
                    key={label}
                    type="button"
                    id={`sidebar-filter-color-${label}`}
                    className={clsx(styles.sidebarColorDot, isActive && styles.sidebarColorActive)}
                    style={{ backgroundColor: hex }}
                    onClick={() => {
                      setActiveSmartFilterId(null);
                      setActiveCollection(null);
                      if (isActive) {
                        clearFilter('colorLabels');
                      } else {
                        setFilter({ colorLabels: [label] });
                      }
                      search();
                    }}
                    title={name}
                  />
                );
              })}
            </div>
          )}
        </div>

        {/* ── Subgroup: Date Added / Modified ── */}
        <div className={styles.subGroup}>
          <button
            type="button"
            className={styles.subGroupHeader}
            onClick={() => toggleFilterSub('dateAdded')}
          >
            <span className={styles.subGroupTitle}>
              {filterSubExpanded.dateAdded ? <IconChevronDown size={10} /> : <IconChevronRight size={10} />}
              <span>Date Added</span>
              {currentFilter.dateRange && <span className={styles.filterActiveChip}>●</span>}
            </span>
            {currentFilter.dateRange && (
              <span
                role="button"
                tabIndex={0}
                className={styles.clearSubFilterBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  clearFilter('dateRange');
                  search();
                }}
              >
                Clear
              </span>
            )}
          </button>
          {filterSubExpanded.dateAdded && (
            <div className={styles.subGroupBody}>
              {(() => {
                const todayStart = getStartOfDay();
                const sevenDaysAgo = Date.now() - 7 * 86400000;
                const thirtyDaysAgo = Date.now() - 30 * 86400000;

                const curFrom = currentFilter.dateRange?.from;
                const isTodayActive = curFrom !== undefined && curFrom >= todayStart;
                const is7DaysActive = curFrom !== undefined && !isTodayActive && curFrom >= sevenDaysAgo - 10000;
                const is30DaysActive = curFrom !== undefined && !isTodayActive && !is7DaysActive && curFrom >= thirtyDaysAgo - 10000;

                return [
                  { label: 'Today', from: todayStart, active: isTodayActive },
                  { label: 'Last 7 Days', from: sevenDaysAgo, active: is7DaysActive },
                  { label: 'Last 30 Days', from: thirtyDaysAgo, active: is30DaysActive },
                ].map(({ label, from, active }) => (
                  <button
                    key={label}
                    type="button"
                    className={clsx(styles.item, active && styles.active)}
                    onClick={() => {
                      setActiveSmartFilterId(null);
                      setActiveCollection(null);
                      if (active) {
                        clearFilter('dateRange');
                      } else {
                        setFilter({ dateRange: { from } });
                      }
                      search();
                    }}
                  >
                    <span className={clsx(styles.itemName, 'truncate')}>{label}</span>
                    {active && <span className={styles.filterActiveChip}>●</span>}
                  </button>
                ));
              })()}
            </div>
          )}
        </div>

        {/* ── Subgroup: File Size ── */}
        <div className={styles.subGroup}>
          <button
            type="button"
            className={styles.subGroupHeader}
            onClick={() => toggleFilterSub('fileSize')}
          >
            <span className={styles.subGroupTitle}>
              {filterSubExpanded.fileSize ? <IconChevronDown size={10} /> : <IconChevronRight size={10} />}
              <span>File Size</span>
              {currentFilter.sizeRange && <span className={styles.filterActiveChip}>●</span>}
            </span>
            {currentFilter.sizeRange && (
              <span
                role="button"
                tabIndex={0}
                className={styles.clearSubFilterBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  clearFilter('sizeRange');
                  search();
                }}
              >
                Clear
              </span>
            )}
          </button>
          {filterSubExpanded.fileSize && (
            <div className={styles.subGroupBody}>
              {[
                { label: '< 5 MB', min: undefined, max: 5 * 1024 * 1024 },
                { label: '5 MB – 50 MB', min: 5 * 1024 * 1024, max: 50 * 1024 * 1024 },
                { label: '> 50 MB', min: 50 * 1024 * 1024, max: undefined },
              ].map(({ label, min, max }) => {
                const isActive =
                  currentFilter.sizeRange?.min === min && currentFilter.sizeRange?.max === max;
                return (
                  <button
                    key={label}
                    type="button"
                    className={clsx(styles.item, isActive && styles.active)}
                    onClick={() => {
                      setActiveSmartFilterId(null);
                      setActiveCollection(null);
                      if (isActive) {
                        clearFilter('sizeRange');
                      } else {
                        setFilter({ sizeRange: { min, max } });
                      }
                      search();
                    }}
                  >
                    <span className={clsx(styles.itemName, 'truncate')}>{label}</span>
                    {isActive && <span className={styles.filterActiveChip}>●</span>}
                  </button>
                );
              })}
            </div>
          )}
        </div>

        {/* ── Subgroup: Preview Status ── */}
        <div className={styles.subGroup}>
          <button
            type="button"
            className={styles.subGroupHeader}
            onClick={() => toggleFilterSub('previewStatus')}
          >
            <span className={styles.subGroupTitle}>
              {filterSubExpanded.previewStatus ? <IconChevronDown size={10} /> : <IconChevronRight size={10} />}
              <span>Preview Status</span>
              {currentFilter.previewStatus && <span className={styles.filterActiveChip}>●</span>}
            </span>
            {currentFilter.previewStatus && (
              <span
                role="button"
                tabIndex={0}
                className={styles.clearSubFilterBtn}
                onClick={(e) => {
                  e.stopPropagation();
                  clearFilter('previewStatus');
                  search();
                }}
              >
                Clear
              </span>
            )}
          </button>
          {filterSubExpanded.previewStatus && (
            <div className={styles.subGroupBody}>
              {[
                { label: 'Has Preview', status: 'done' as const },
                { label: 'Pending / Generating', status: 'pending' as const },
                { label: 'Needs Preview', status: 'none' as const },
              ].map(({ label, status }) => {
                const isActive = currentFilter.previewStatus === status;
                return (
                  <button
                    key={label}
                    type="button"
                    className={clsx(styles.item, isActive && styles.active)}
                    onClick={() => {
                      setActiveSmartFilterId(null);
                      setActiveCollection(null);
                      if (isActive) {
                        clearFilter('previewStatus');
                      } else {
                        setFilter({ previewStatus: status });
                      }
                      search();
                    }}
                  >
                    <IconEye size={13} />
                    <span className={clsx(styles.itemName, 'truncate')}>{label}</span>
                    {isActive && <span className={styles.filterActiveChip}>●</span>}
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </SidebarSection>


      <ConfirmDeleteDialog
        isOpen={Boolean(deleteTarget)}
        itemType={deleteTarget?.type ?? 'tag'}
        itemName={deleteTarget?.name ?? ''}
        onConfirm={executeDeleteTarget}
        onCancel={() => setDeleteTarget(null)}
      />

      <SmartFilterModal
        isOpen={isCreatingSmartFilter}
        onClose={() => setIsCreatingSmartFilter(false)}
      />
    </aside>
  );
}

interface SidebarSectionProps {
  label: string;
  icon: React.ReactNode;
  expanded: boolean;
  onToggle: () => void;
  onAdd?: () => void;
  children: React.ReactNode;
}

function SidebarSection({
  label, icon, expanded, onToggle, onAdd, children,
}: SidebarSectionProps) {
  return (
    <div className={styles.section}>
      <div className={styles.sectionHeader}>
        <button className={styles.sectionToggle} onClick={onToggle}>
          {expanded ? <IconChevronDown size={12} /> : <IconChevronRight size={12} />}
          {icon}
          <span className={styles.sectionLabel}>{label}</span>
        </button>
        {onAdd && (
          <button className={styles.addBtn} onClick={onAdd} title={`Add ${label}`} id={`add-${label.toLowerCase()}`}>
            <IconPlus size={12} />
          </button>
        )}
      </div>
      {expanded && <div className={styles.sectionBody}>{children}</div>}
    </div>
  );
}
