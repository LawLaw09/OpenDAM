import { useState } from 'react';
import clsx from 'clsx';
import { open } from '@tauri-apps/plugin-dialog';
import {
  useLibraryStore, useCollectionStore, useTagStore,
  useUIStore, useAssetStore,
} from '../../store';
import type { FilterSpec } from '../../types';
import {
  IconFolder, IconCollection, IconTag,
  IconChevronDown, IconChevronRight, IconPlus,
  IconCheck, IconX, IconTrash, IconInbox, IconFilter,
} from '../Icons';
import { ConfirmDeleteDialog } from '../ConfirmDeleteDialog';
import { SmartFilterModal } from '../SmartFilterModal';
import styles from './Sidebar.module.css';

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

  const { activeLibraryId, activeCollectionId, setActiveLibrary, setActiveCollection } = useUIStore();
  const setFilter = useAssetStore((s) => s.setFilter);
  const clearFilter = useAssetStore((s) => s.clearFilter);
  const setQuery = useAssetStore((s) => s.setQuery);
  const currentFilter = useAssetStore((s) => s.query.filter);
  const activeTagId = useAssetStore((s) => s.query.filter.tags?.[0] ?? null);
  const search = useAssetStore((s) => s.search);

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

  const handleLibraryClick = (id: string) => {
    setActiveSmartFilterId(null);
    if (activeLibraryId === id) {
      setActiveLibrary(null);
      setQuery({ libraryId: undefined });
    } else {
      setActiveLibrary(id);
      setQuery({ libraryId: id, collectionId: undefined });
    }
    search();
  };

  const handleCollectionClick = (id: string) => {
    setActiveSmartFilterId(null);
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
    if (activeSmartFilterId === col.id) {
      setActiveSmartFilterId(null);
      setFilter({
        kinds: undefined,
        tags: undefined,
        rating: undefined,
        colorLabels: undefined,
        extensions: undefined,
        unorganized: undefined,
        directory: undefined,
        namePrefix: undefined,
        nameSuffix: undefined,
        collectionId: undefined,
      });
      search();
    } else {
      setActiveSmartFilterId(col.id);
      setActiveCollection(null);
      try {
        const spec: FilterSpec = col.filterSpec ? JSON.parse(col.filterSpec) : {};
        setFilter({
          kinds: undefined,
          tags: undefined,
          rating: undefined,
          colorLabels: undefined,
          extensions: undefined,
          unorganized: undefined,
          directory: undefined,
          namePrefix: undefined,
          nameSuffix: undefined,
          collectionId: undefined,
          ...spec,
        });
        search();
      } catch (err) {
        console.error('Failed to parse smart filter spec:', err);
      }
    }
  };

  const handleTagFilter = (tagId: string) => {
    setActiveSmartFilterId(null);
    if (activeTagId === tagId) {
      // clicking the active tag clears it
      clearFilter('tags');
    } else {
      setFilter({ tags: [tagId] });
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
    const filePath = e.dataTransfer.getData('text/plain');
    const asset = useAssetStore.getState().assets.find((a) => a.filePath === filePath);
    if (asset) {
      try {
        await addToCollection(colId, [asset.id]);
      } catch (err) {
        console.error('Failed to add asset to collection:', err);
      }
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
        {libraries.map((lib) => (
          <div
            key={lib.id}
            id={`lib-${lib.id}`}
            className={clsx(styles.item, activeLibraryId === lib.id && styles.active)}
            onClick={() => handleLibraryClick(lib.id)}
            role="button"
            tabIndex={0}
          >
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
        ))}
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

      {/* ── Quick filters (Untouchable built-in filters) ── */}
      <SidebarSection
        label="Filters"
        icon={null}
        expanded={expanded.filters}
        onToggle={() => toggle('filters')}
      >
        {[
          { label: 'All assets', filter: { kinds: undefined, unorganized: undefined } as Partial<FilterSpec>, isAll: true },
          { label: 'Unorganized', filter: { kinds: undefined, unorganized: true } as Partial<FilterSpec>, isUnorganized: true },
          { label: '3D Models', filter: { kinds: ['3d_model' as const], unorganized: undefined } as Partial<FilterSpec> },
          { label: 'Materials', filter: { kinds: ['material' as const], unorganized: undefined } as Partial<FilterSpec> },
          { label: 'Textures', filter: { kinds: ['texture' as const], unorganized: undefined } as Partial<FilterSpec> },
          { label: 'HDRI', filter: { kinds: ['hdri' as const], unorganized: undefined } as Partial<FilterSpec> },
          { label: 'Images', filter: { kinds: ['image' as const], unorganized: undefined } as Partial<FilterSpec> },
          { label: 'Video', filter: { kinds: ['video' as const], unorganized: undefined } as Partial<FilterSpec> },
        ].map(({ label, filter, isAll, isUnorganized }) => {
          const isActive = isUnorganized
            ? Boolean(currentFilter.unorganized)
            : isAll
            ? !currentFilter.unorganized && (!currentFilter.kinds || currentFilter.kinds.length === 0)
            : currentFilter.kinds?.length === 1 && currentFilter.kinds[0] === filter.kinds?.[0] && !currentFilter.unorganized;

          return (
            <button
              key={label}
              id={`sidebar-filter-${label.toLowerCase().replace(/\s+/g, '-')}`}
              className={clsx(styles.item, isActive && styles.active)}
              onClick={() => {
                setActiveSmartFilterId(null);
                setFilter(filter);
                search();
              }}
            >
              {isUnorganized && <IconInbox size={13} />}
              <span className={clsx(styles.itemName, 'truncate')}>{label}</span>
            </button>
          );
        })}
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
