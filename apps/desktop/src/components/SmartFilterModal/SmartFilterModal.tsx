import React, { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useTagStore, useCollectionStore } from '../../store';
import type { FilterSpec, ColorLabel } from '../../types';
import { IconFilter, IconX, IconStar, IconFolder } from '../Icons';
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
  const [selectedTag, setSelectedTag] = useState<string>('');
  const [selectedCollection, setSelectedCollection] = useState<string>('');
  const [includeSubcollections, setIncludeSubcollections] = useState<boolean>(true);
  const [isSaving, setIsSaving] = useState(false);

  const tags = useTagStore((s) => s.tags);
  const collections = useCollectionStore((s) => s.collections);
  const createCollection = useCollectionStore((s) => s.createCollection);

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
      if (selectedTag) {
        filterSpec.tags = [selectedTag];
      }
      if (selectedCollection) {
        filterSpec.collectionId = selectedCollection;
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
      setSelectedTag('');
      setSelectedCollection('');
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
                placeholder="e.g. 5-Star Textures, High-Res Videos..."
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

            {/* Tag & Collection */}
            <div className={styles.grid2}>
              <div className={styles.field}>
                <label className={styles.label}>
                  <span>Tag</span>
                  <span className={styles.optional}>Optional</span>
                </label>
                <select
                  className={styles.select}
                  value={selectedTag}
                  onChange={(e) => setSelectedTag(e.target.value)}
                >
                  <option value="">-- Any Tag --</option>
                  {tags.map((t) => (
                    <option key={t.id} value={t.id}>
                      {t.name}
                    </option>
                  ))}
                </select>
              </div>

              <div className={styles.field}>
                <label className={styles.label}>
                  <span>Collection</span>
                  <span className={styles.optional}>Optional</span>
                </label>
                <select
                  className={styles.select}
                  value={selectedCollection}
                  onChange={(e) => setSelectedCollection(e.target.value)}
                >
                  <option value="">-- Any Collection --</option>
                  {collections
                    .filter((c) => !c.isSmart)
                    .map((c) => (
                      <option key={c.id} value={c.id}>
                        {c.name}
                      </option>
                    ))}
                </select>
              </div>
            </div>

            {selectedCollection && (
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12, color: 'var(--color-text-secondary)' }}>
                <input
                  type="checkbox"
                  id="smart-subcol"
                  checked={includeSubcollections}
                  onChange={(e) => setIncludeSubcollections(e.target.checked)}
                />
                <label htmlFor="smart-subcol" style={{ cursor: 'pointer' }}>
                  Include sub-collections automatically
                </label>
              </div>
            )}
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
