import React, { useState, useMemo } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTagStore, useCollectionStore } from '../../store';
import type { FilterSpec, ColorLabel, Collection } from '../../types';
import { IconFilter, IconX, IconStar, IconFolder, IconCheck, IconCollection } from '../Icons';
import styles from './SmartFilterModal.module.css';

interface SmartFilterModalProps {
  isOpen: boolean;
  onClose: () => void;
  onCreated?: () => void;
}

const COMMON_EXTENSIONS = [
  'jpg', 'png', 'gif', 'svg', 'webp', 'psd',
  'mp4', 'mov', 'mkv',
  'obj', 'fbx', 'gltf', 'blend',
  'pdf', 'mp3', 'wav',
];

const COLOR_OPTIONS: { label: ColorLabel; hex: string }[] = [
  { label: 'red', hex: '#ef4444' },
  { label: 'orange', hex: '#f97316' },
  { label: 'yellow', hex: '#eab308' },
  { label: 'green', hex: '#22c55e' },
  { label: 'teal', hex: '#14b8a6' },
  { label: 'blue', hex: '#3b82f6' },
  { label: 'purple', hex: '#a855f7' },
  { label: 'pink', hex: '#ec4899' },
];

export function SmartFilterModal({ isOpen, onClose, onCreated }: SmartFilterModalProps) {
  const [name, setName] = useState('');
  const [directory, setDirectory] = useState('');
  const [namePrefix, setNamePrefix] = useState('');
  const [nameSuffix, setNameSuffix] = useState('');
  const [selectedExtensions, setSelectedExtensions] = useState<string[]>([]);
  const [customExt, setCustomExt] = useState('');
  const [ratingMin, setRatingMin] = useState<number>(0);
  const [selectedColor, setSelectedColor] = useState<ColorLabel | null>(null);

  // Tag filter state: 'any' | 'none' (untagged) | 'specific'
  const [tagMode, setTagMode] = useState<'any' | 'none' | 'specific'>('any');
  const [selectedTags, setSelectedTags] = useState<string[]>([]);
  const [tagLogic, setTagLogic] = useState<'or' | 'and' | 'and_or'>('or');

  // Collection filter state: 'any' | 'none' (unorganized) | 'specific'
  const [collectionMode, setCollectionMode] = useState<'any' | 'none' | 'specific'>('any');
  const [selectedCollections, setSelectedCollections] = useState<string[]>([]);
  const [collectionLogic, setCollectionLogic] = useState<'or' | 'and' | 'and_or'>('or');
  const [includeSubcollections, setIncludeSubcollections] = useState<boolean>(true);

  const [isSaving, setIsSaving] = useState(false);

  const tags = useTagStore((s) => s.tags);
  const collections = useCollectionStore((s) => s.collections);
  const createCollection = useCollectionStore((s) => s.createCollection);

  // Compute full ancestral paths for collections (e.g. Textures / Wood / Oak)
  const collectionListWithPaths = useMemo(() => {
    const regularCols = collections.filter((c) => !c.isSmart);

    const getPath = (col: Collection): { path: string; parentChain: string[]; leaf: string } => {
      const parts = [col.name];
      let curr = col;
      while (curr.parentId) {
        const parent = regularCols.find((c) => c.id === curr.parentId);
        if (!parent) break;
        parts.unshift(parent.name);
        curr = parent;
      }
      const leaf = parts[parts.length - 1];
      const parentChain = parts.slice(0, parts.length - 1);
      return { path: parts.join(' / '), parentChain, leaf };
    };

    return regularCols
      .map((col) => ({
        ...col,
        ...getPath(col),
      }))
      .sort((a, b) => a.path.localeCompare(b.path));
  }, [collections]);

  if (!isOpen) return null;

  const toggleExtension = (ext: string) => {
    setSelectedExtensions((prev) =>
      prev.includes(ext) ? prev.filter((e) => e !== ext) : [...prev, ext]
    );
  };

  const handleAddCustomExt = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' && customExt.trim()) {
      e.preventDefault();
      const clean = customExt.trim().toLowerCase().replace(/^\./, '');
      if (clean && !selectedExtensions.includes(clean)) {
        setSelectedExtensions((prev) => [...prev, clean]);
      }
      setCustomExt('');
    }
  };

  const handleBrowseFolder = async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === 'string') {
        setDirectory(selected);
      }
    } catch (err) {
      console.error('Failed to pick folder', err);
    }
  };

  const toggleTagSelection = (tagId: string) => {
    setSelectedTags((prev) =>
      prev.includes(tagId) ? prev.filter((id) => id !== tagId) : [...prev, tagId]
    );
  };

  const toggleCollectionSelection = (colId: string) => {
    setSelectedCollections((prev) =>
      prev.includes(colId) ? prev.filter((id) => id !== colId) : [...prev, colId]
    );
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    setIsSaving(true);
    try {
      const filterSpec: FilterSpec = {};

      if (directory.trim()) {
        filterSpec.directory = directory.trim();
      }
      if (namePrefix.trim()) {
        filterSpec.namePrefix = namePrefix.trim();
      }
      if (nameSuffix.trim()) {
        filterSpec.nameSuffix = nameSuffix.trim();
      }
      if (selectedExtensions.length > 0) {
        filterSpec.extensions = selectedExtensions;
      }
      if (ratingMin > 0) {
        filterSpec.rating = { min: ratingMin, max: 5 };
      }
      if (selectedColor) {
        filterSpec.colorLabels = [selectedColor];
      }

      // Tag filter logic
      if (tagMode === 'none') {
        filterSpec.tagLogic = 'none';
      } else if (tagMode === 'specific' && selectedTags.length > 0) {
        filterSpec.tags = selectedTags;
        filterSpec.tagLogic = tagLogic;
      }

      // Collection filter logic
      if (collectionMode === 'none') {
        filterSpec.collectionLogic = 'none';
        filterSpec.unorganized = true;
      } else if (collectionMode === 'specific' && selectedCollections.length > 0) {
        filterSpec.collectionIds = selectedCollections;
        filterSpec.collectionLogic = collectionLogic;
        filterSpec.includeSubcollections = includeSubcollections;
      }

      await createCollection(
        name.trim(),
        undefined,
        undefined,
        true,
        JSON.stringify(filterSpec)
      );

      setName('');
      setDirectory('');
      setNamePrefix('');
      setNameSuffix('');
      setSelectedExtensions([]);
      setRatingMin(0);
      setSelectedColor(null);
      setTagMode('any');
      setSelectedTags([]);
      setCollectionMode('any');
      setSelectedCollections([]);
      onCreated?.();
      onClose();
    } catch (err) {
      console.error('Failed to create smart filter:', err);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <div className={styles.titleArea}>
            <div className={styles.iconCircle}>
              <IconFilter size={16} />
            </div>
            <div>
              <div className={styles.title}>Create Smart Filter</div>
              <div className={styles.subtitle}>Dynamic filter rule saved across your libraries</div>
            </div>
          </div>
          <button type="button" className={styles.closeBtn} onClick={onClose} aria-label="Close">
            <IconX size={15} />
          </button>
        </div>

        <form onSubmit={handleSave} style={{ display: 'contents' }}>
          <div className={styles.body}>
            {/* Filter Name */}
            <div className={styles.field}>
              <label className={styles.label}>Filter Name *</label>
              <input
                type="text"
                className={styles.input}
                placeholder="e.g. 5-Star Textures, Untagged Assets, Sci-Fi Models..."
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
                autoFocus
              />
            </div>

            {/* Directory Filter */}
            <div className={styles.field}>
              <label className={styles.label}>
                <span>Directory Path</span>
                <span className={styles.optional}>Optional</span>
              </label>
              <div style={{ display: 'flex', gap: 6 }}>
                <input
                  type="text"
                  className={styles.input}
                  placeholder="e.g. C:/Assets/Textures or subfolder name"
                  value={directory}
                  onChange={(e) => setDirectory(e.target.value)}
                />
                <button
                  type="button"
                  className={`${styles.btn} ${styles.cancelBtn}`}
                  onClick={handleBrowseFolder}
                  title="Browse folder"
                  style={{ display: 'flex', alignItems: 'center', gap: 4, padding: '7px 10px' }}
                >
                  <IconFolder size={14} />
                  <span>Browse</span>
                </button>
              </div>
            </div>

            {/* Name Prefix / Suffix */}
            <div className={styles.grid2}>
              <div className={styles.field}>
                <label className={styles.label}>
                  <span>Item Name Prefix</span>
                  <span className={styles.optional}>Starts with</span>
                </label>
                <input
                  type="text"
                  className={styles.input}
                  placeholder="e.g. T_ or hero_"
                  value={namePrefix}
                  onChange={(e) => setNamePrefix(e.target.value)}
                />
              </div>

              <div className={styles.field}>
                <label className={styles.label}>
                  <span>Item Name Suffix</span>
                  <span className={styles.optional}>Ends with</span>
                </label>
                <input
                  type="text"
                  className={styles.input}
                  placeholder="e.g. _albedo or _final"
                  value={nameSuffix}
                  onChange={(e) => setNameSuffix(e.target.value)}
                />
              </div>
            </div>

            {/* File Extensions */}
            <div className={styles.field}>
              <label className={styles.label}>
                <span>File Types / Extensions</span>
                <span className={styles.optional}>Multi-select</span>
              </label>
              <div className={styles.chipGrid}>
                {COMMON_EXTENSIONS.map((ext) => {
                  const active = selectedExtensions.includes(ext);
                  return (
                    <button
                      key={ext}
                      type="button"
                      className={`${styles.chip} ${active ? styles.chipActive : ''}`}
                      onClick={() => toggleExtension(ext)}
                    >
                      .{ext}
                    </button>
                  );
                })}
              </div>
              <input
                type="text"
                className={styles.input}
                style={{ marginTop: 6, fontSize: 12 }}
                placeholder="Type custom extension & press Enter (e.g. exr, tga, wav)..."
                value={customExt}
                onChange={(e) => setCustomExt(e.target.value)}
                onKeyDown={handleAddCustomExt}
              />
            </div>

            {/* Rating and Color Row */}
            <div className={styles.grid2}>
              <div className={styles.field}>
                <label className={styles.label}>
                  <span>Minimum Rating</span>
                  <span className={styles.optional}>{ratingMin > 0 ? `${ratingMin}+ stars` : 'Any'}</span>
                </label>
                <div className={styles.starRow}>
                  {[1, 2, 3, 4, 5].map((star) => (
                    <button
                      key={star}
                      type="button"
                      className={`${styles.starBtn} ${ratingMin >= star ? styles.starOn : ''}`}
                      onClick={() => setRatingMin(ratingMin === star ? 0 : star)}
                      title={`${star} Star${star > 1 ? 's' : ''} or higher`}
                    >
                      <IconStar size={18} />
                    </button>
                  ))}
                  {ratingMin > 0 && (
                    <button
                      type="button"
                      className={styles.chip}
                      style={{ marginLeft: 6, padding: '2px 6px', fontSize: 10 }}
                      onClick={() => setRatingMin(0)}
                    >
                      Clear
                    </button>
                  )}
                </div>
              </div>

              <div className={styles.field}>
                <label className={styles.label}>
                  <span>Color Label</span>
                  <span className={styles.optional}>{selectedColor ?? 'Any'}</span>
                </label>
                <div className={styles.colorRow}>
                  {COLOR_OPTIONS.map(({ label, hex }) => {
                    const active = selectedColor === label;
                    return (
                      <button
                        key={label}
                        type="button"
                        className={`${styles.colorDot} ${active ? styles.colorDotActive : ''}`}
                        style={{ backgroundColor: hex }}
                        onClick={() => setSelectedColor(active ? null : label)}
                        title={label}
                      />
                    );
                  })}
                  {selectedColor && (
                    <button
                      type="button"
                      className={styles.chip}
                      style={{ padding: '2px 6px', fontSize: 10 }}
                      onClick={() => setSelectedColor(null)}
                    >
                      Clear
                    </button>
                  )}
                </div>
              </div>
            </div>

            {/* ── TAG FILTER SECTION (Any / No-Tag / Specific Tags with AND/OR) ── */}
            <div className={styles.field}>
              <div className={styles.label}>
                <span>Tag Filter</span>
                <span className={styles.optional}>
                  {tagMode === 'none'
                    ? 'Matches untagged items'
                    : tagMode === 'specific'
                    ? `${selectedTags.length} selected (${tagLogic.toUpperCase()})`
                    : 'Any'}
                </span>
              </div>

              <div className={styles.segmentedControl}>
                <button
                  type="button"
                  className={`${styles.segmentBtn} ${tagMode === 'any' ? styles.segmentBtnActive : ''}`}
                  onClick={() => setTagMode('any')}
                >
                  Any Tags
                </button>
                <button
                  type="button"
                  className={`${styles.segmentBtn} ${tagMode === 'none' ? styles.segmentBtnActive : ''}`}
                  onClick={() => setTagMode('none')}
                >
                  Untagged (No Tags)
                </button>
                <button
                  type="button"
                  className={`${styles.segmentBtn} ${tagMode === 'specific' ? styles.segmentBtnActive : ''}`}
                  onClick={() => setTagMode('specific')}
                >
                  Specific Tags
                </button>
              </div>

              {tagMode === 'specific' && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: 4 }}>
                  <div className={styles.logicBar}>
                    <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>
                      Match logic:
                    </span>
                    <div className={styles.logicToggle}>
                      <button
                        type="button"
                        className={`${styles.logicBtn} ${tagLogic === 'or' ? styles.logicBtnActive : ''}`}
                        onClick={() => setTagLogic('or')}
                        title="Matches items having ANY of the selected tags"
                      >
                        ANY (OR)
                      </button>
                      <button
                        type="button"
                        className={`${styles.logicBtn} ${tagLogic === 'and' ? styles.logicBtnActive : ''}`}
                        onClick={() => setTagLogic('and')}
                        title="Matches items having ALL of the selected tags"
                      >
                        ALL (AND)
                      </button>
                      <button
                        type="button"
                        className={`${styles.logicBtn} ${tagLogic === 'and_or' ? styles.logicBtnActive : ''}`}
                        onClick={() => setTagLogic('and_or')}
                        title="Matches items having the primary tag AND at least one of the other selected tags (Any + All)"
                      >
                        AND + OR (ANY + ALL)
                      </button>
                    </div>
                  </div>

                  <div className={styles.chipGrid}>
                    {tags.length === 0 && (
                      <span style={{ fontSize: 11.5, color: 'var(--color-text-disabled)' }}>
                        No tags created yet.
                      </span>
                    )}
                    {tags.map((t) => {
                      const active = selectedTags.includes(t.id);
                      return (
                        <button
                          key={t.id}
                          type="button"
                          className={`${styles.chip} ${active ? styles.chipActive : ''}`}
                          onClick={() => toggleTagSelection(t.id)}
                          style={{ display: 'inline-flex', alignItems: 'center', gap: 4 }}
                        >
                          <span
                            style={{
                              width: 6,
                              height: 6,
                              borderRadius: '50%',
                              backgroundColor: t.color ? `var(--swatch-${t.color})` : 'hsl(220 85% 60%)',
                            }}
                          />
                          <span>{t.name}</span>
                          {active && <IconCheck size={10} />}
                        </button>
                      );
                    })}
                  </div>
                </div>
              )}
            </div>

            {/* ── COLLECTION FILTER SECTION (Any / No-Collection / Specific with Hierarchy & AND/OR) ── */}
            <div className={styles.field}>
              <div className={styles.label}>
                <span>Collection Filter</span>
                <span className={styles.optional}>
                  {collectionMode === 'none'
                    ? 'Matches unorganized items'
                    : collectionMode === 'specific'
                    ? `${selectedCollections.length} selected (${collectionLogic.toUpperCase()})`
                    : 'Any'}
                </span>
              </div>

              <div className={styles.segmentedControl}>
                <button
                  type="button"
                  className={`${styles.segmentBtn} ${collectionMode === 'any' ? styles.segmentBtnActive : ''}`}
                  onClick={() => setCollectionMode('any')}
                >
                  Any Collections
                </button>
                <button
                  type="button"
                  className={`${styles.segmentBtn} ${collectionMode === 'none' ? styles.segmentBtnActive : ''}`}
                  onClick={() => setCollectionMode('none')}
                >
                  Unorganized (No Collection)
                </button>
                <button
                  type="button"
                  className={`${styles.segmentBtn} ${collectionMode === 'specific' ? styles.segmentBtnActive : ''}`}
                  onClick={() => setCollectionMode('specific')}
                >
                  Specific Collections
                </button>
              </div>

              {collectionMode === 'specific' && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 6, marginTop: 4 }}>
                  <div className={styles.logicBar}>
                    <span style={{ fontSize: 11, color: 'var(--color-text-secondary)' }}>
                      Match logic:
                    </span>
                    <div className={styles.logicToggle}>
                      <button
                        type="button"
                        className={`${styles.logicBtn} ${collectionLogic === 'or' ? styles.logicBtnActive : ''}`}
                        onClick={() => setCollectionLogic('or')}
                        title="Matches items in ANY of the selected collections"
                      >
                        ANY (OR)
                      </button>
                      <button
                        type="button"
                        className={`${styles.logicBtn} ${collectionLogic === 'and' ? styles.logicBtnActive : ''}`}
                        onClick={() => setCollectionLogic('and')}
                        title="Matches items belonging to ALL selected collections"
                      >
                        ALL (AND)
                      </button>
                      <button
                        type="button"
                        className={`${styles.logicBtn} ${collectionLogic === 'and_or' ? styles.logicBtnActive : ''}`}
                        onClick={() => setCollectionLogic('and_or')}
                        title="Matches items in parent/main collection AND in any selected sub-collection (Any + All)"
                      >
                        AND + OR (ANY + ALL)
                      </button>
                    </div>
                  </div>

                  {/* Hierarchical scrollable selector showing Parent / Sub-collection */}
                  <div className={styles.multiSelectBox}>
                    {collectionListWithPaths.length === 0 && (
                      <span style={{ fontSize: 11.5, color: 'var(--color-text-disabled)', padding: 6 }}>
                        No collections created yet.
                      </span>
                    )}
                    {collectionListWithPaths.map((col) => {
                      const active = selectedCollections.includes(col.id);
                      return (
                        <div
                          key={col.id}
                          className={`${styles.multiSelectItem} ${active ? styles.multiSelectItemActive : ''}`}
                          onClick={() => toggleCollectionSelection(col.id)}
                        >
                          <input
                            type="checkbox"
                            checked={active}
                            onChange={() => {}}
                            style={{ cursor: 'pointer', accentColor: 'var(--color-brand-500)' }}
                          />
                          <span style={{ opacity: 0.7, flexShrink: 0, display: 'flex' }}>
                            <IconCollection size={12} />
                          </span>
                          <div className={styles.itemPath}>
                            {col.parentChain.length > 0 && (
                              <span className={styles.parentPart}>
                                {col.parentChain.join(' / ')} /
                              </span>
                            )}
                            <span className={styles.leafPart}>{col.leaf}</span>
                          </div>
                        </div>
                      );
                    })}
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, color: 'var(--color-text-secondary)', marginTop: 2 }}>
                    <input
                      type="checkbox"
                      id="smart-subcol"
                      checked={includeSubcollections}
                      onChange={(e) => setIncludeSubcollections(e.target.checked)}
                      style={{ cursor: 'pointer', accentColor: 'var(--color-brand-500)' }}
                    />
                    <label htmlFor="smart-subcol" style={{ cursor: 'pointer' }}>
                      Include sub-collections automatically
                    </label>
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className={styles.footer}>
            <button
              type="button"
              className={`${styles.btn} ${styles.cancelBtn}`}
              onClick={onClose}
              disabled={isSaving}
            >
              Cancel
            </button>
            <button
              type="submit"
              className={`${styles.btn} ${styles.saveBtn}`}
              disabled={!name.trim() || isSaving}
            >
              {isSaving ? 'Saving...' : 'Create Smart Filter'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
