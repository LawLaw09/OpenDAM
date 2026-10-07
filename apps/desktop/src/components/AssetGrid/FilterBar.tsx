import { useState } from 'react';
import { useAssetStore } from '../../store';
import type { AssetKind, ColorLabel } from '../../types';
import { IconFilter, IconX, IconLayers, IconFileText, IconInbox, IconHeartFilled } from '../Icons';
import styles from './FilterBar.module.css';
import clsx from 'clsx';

const KIND_OPTIONS: { label: string; value: AssetKind }[] = [
  { label: '3D Models', value: '3d_model' },
  { label: 'Materials', value: 'material' },
  { label: 'Textures', value: 'texture' },
  { label: 'HDRI', value: 'hdri' },
  { label: 'Images', value: 'image' },
  { label: 'Video', value: 'video' },
  { label: 'Documents', value: 'document' },
];

const COLOR_LABELS: ColorLabel[] = [
  'red', 'orange', 'yellow', 'green', 'teal', 'blue', 'purple', 'pink',
];

interface SoftwareFilter {
  id: string;
  label: string;
  extLabel: string;
  extensions: string[];
  color: string;
}

const SOFTWARE_OPTIONS: SoftwareFilter[] = [
  { id: 'max',         label: '3ds Max',     extLabel: '.max',       extensions: ['max'],                    color: '#0696d7' },
  { id: 'rhino',       label: 'Rhino',       extLabel: '.3dm',       extensions: ['3dm'],                    color: '#e53935' },
  { id: 'revit',       label: 'Revit',       extLabel: '.rvt, .rfa', extensions: ['rvt', 'rfa'],             color: '#00838f' },
  { id: 'autocad',     label: 'AutoCAD',     extLabel: '.dwg, .dxf', extensions: ['dwg', 'dxf'],             color: '#c62828' },
  { id: 'sketchup',    label: 'SketchUp',    extLabel: '.skp',       extensions: ['skp'],                    color: '#005f9e' },
  { id: 'blender',     label: 'Blender',     extLabel: '.blend',     extensions: ['blend'],                  color: '#f5792a' },
  { id: 'vray',        label: 'V-Ray',       extLabel: '.vrmat',     extensions: ['vrmat', 'mat'],           color: '#8e24aa' },
  { id: 'photoshop',   label: 'Photoshop',   extLabel: '.psd',       extensions: ['psd', 'psb'],             color: '#1e88e5' },
  { id: 'illustrator', label: 'Illustrator', extLabel: '.ai',        extensions: ['ai', 'svg'],              color: '#ff8f00' },
  { id: 'hdri',        label: 'HDR / EXR',   extLabel: '.exr, .hdr', extensions: ['exr', 'hdr', 'hdri'],      color: '#d81b60' },
  { id: 'open3d',      label: 'FBX / OBJ',   extLabel: '.fbx, .obj', extensions: ['fbx', 'obj', '3ds', 'gltf', 'glb', 'stl', 'dae'], color: '#43a047' },
  { id: 'textures',    label: 'Textures',    extLabel: '.jpg, .png', extensions: ['jpg', 'jpeg', 'png', 'tif', 'tiff', 'tga', 'bmp', 'webp'], color: '#00acc1' },
];

interface ExtensionOption {
  ext: string;
  label: string;
  softwareHint: string;
}

const EXTENSION_OPTIONS: ExtensionOption[] = [
  { ext: 'max',   label: '.max',   softwareHint: '3ds Max' },
  { ext: '3dm',   label: '.3dm',   softwareHint: 'Rhino' },
  { ext: 'rvt',   label: '.rvt',   softwareHint: 'Revit Project' },
  { ext: 'rfa',   label: '.rfa',   softwareHint: 'Revit Family' },
  { ext: 'dwg',   label: '.dwg',   softwareHint: 'AutoCAD DWG' },
  { ext: 'skp',   label: '.skp',   softwareHint: 'SketchUp' },
  { ext: 'blend', label: '.blend', softwareHint: 'Blender' },
  { ext: 'vrmat', label: '.vrmat', softwareHint: 'V-Ray' },
  { ext: 'psd',   label: '.psd',   softwareHint: 'Photoshop' },
  { ext: 'ai',    label: '.ai',    softwareHint: 'Illustrator' },
  { ext: 'exr',   label: '.exr',   softwareHint: 'OpenEXR' },
  { ext: 'hdr',   label: '.hdr',   softwareHint: 'Radiance HDR' },
  { ext: 'fbx',   label: '.fbx',   softwareHint: 'FBX' },
  { ext: 'obj',   label: '.obj',   softwareHint: 'OBJ' },
  { ext: '3ds',   label: '.3ds',   softwareHint: '3DS' },
  { ext: 'jpg',   label: '.jpg',   softwareHint: 'JPEG' },
  { ext: 'png',   label: '.png',   softwareHint: 'PNG' },
  { ext: 'tif',   label: '.tif',   softwareHint: 'TIFF' },
];

export function FilterBar() {
  const { query, setFilter, setSort, search } = useAssetStore();
  const { filter, sort } = query;
  const [filterMode, setFilterMode] = useState<'software' | 'extension'>('software');

  const hasActiveFilters =
    (filter.kinds?.length ?? 0) > 0 ||
    (filter.colorLabels?.length ?? 0) > 0 ||
    filter.rating !== undefined ||
    (filter.extensions?.length ?? 0) > 0 ||
    Boolean(filter.unorganized) ||
    Boolean(filter.isFavorite);

  const activeExtCount = filter.extensions?.length ?? 0;

  const toggleFavorite = () => {
    setFilter({ isFavorite: filter.isFavorite ? undefined : true });
    search();
  };

  const toggleKind = (kind: AssetKind) => {
    const current = filter.kinds ?? [];
    const next = current.includes(kind)
      ? current.filter((k) => k !== kind)
      : [...current, kind];
    setFilter({ kinds: next.length ? next : undefined });
    search();
  };

  const toggleUnorganized = () => {
    setFilter({ unorganized: filter.unorganized ? undefined : true });
    search();
  };

  const toggleColorLabel = (label: ColorLabel) => {
    const current = filter.colorLabels ?? [];
    const next = current.includes(label)
      ? current.filter((l) => l !== label)
      : [...current, label];
    setFilter({ colorLabels: next.length ? next : undefined });
    search();
  };

  const toggleSoftware = (sw: SoftwareFilter) => {
    const current = filter.extensions ?? [];
    const allPresent = sw.extensions.every((ext) => current.includes(ext));
    let next: string[];
    if (allPresent) {
      next = current.filter((ext) => !sw.extensions.includes(ext));
    } else {
      next = Array.from(new Set([...current, ...sw.extensions]));
    }
    setFilter({ extensions: next.length > 0 ? next : undefined });
    search();
  };

  const isSoftwareActive = (sw: SoftwareFilter) => {
    const current = filter.extensions ?? [];
    return sw.extensions.some((ext) => current.includes(ext));
  };

  const toggleExtension = (ext: string) => {
    const current = filter.extensions ?? [];
    const next = current.includes(ext)
      ? current.filter((e) => e !== ext)
      : [...current, ext];
    setFilter({ extensions: next.length > 0 ? next : undefined });
    search();
  };

  const clearExtensions = () => {
    setFilter({ extensions: undefined });
    search();
  };

  const clearFilters = () => {
    setFilter({
      kinds: undefined,
      colorLabels: undefined,
      rating: undefined,
      extensions: undefined,
      unorganized: undefined,
      isFavorite: undefined,
      dateRange: undefined,
      sizeRange: undefined,
      previewStatus: undefined,
    });
    search();
  };

  return (
    <div className={styles.filterContainer} id="filter-bar">
      {/* ── Row 1: Primary Category / Color / Rating / Sort ── */}
      <div className={styles.primaryRow}>
        <div className={styles.left}>
          <IconFilter size={14} />

          {/* Kind & Status pills */}
          <div className={styles.pills}>
            <button
              id="filter-favorites"
              className={clsx(styles.favoritePill, filter.isFavorite && styles.favoriteActive)}
              onClick={toggleFavorite}
              title="Show only favorite assets"
            >
              <IconHeartFilled size={12} className={clsx(filter.isFavorite ? styles.heartActive : styles.heartMuted)} />
              <span>Favorites</span>
            </button>
            <button
              id="filter-unorganized"
              className={clsx(styles.pill, filter.unorganized && styles.active)}
              onClick={toggleUnorganized}
              title="Filter assets not assigned to any collection (unorganized)"
            >
              <IconInbox size={12} />
              <span>Unorganized</span>
            </button>
            {KIND_OPTIONS.map(({ label, value }) => (
              <button
                key={value}
                id={`filter-kind-${value}`}
                className={clsx(styles.pill, filter.kinds?.includes(value) && styles.active)}
                onClick={() => toggleKind(value)}
              >
                {label}
              </button>
            ))}
          </div>

          <div className={styles.sep} />

          {/* Color labels */}
          <div className={styles.colorRow}>
            {COLOR_LABELS.map((label) => (
              <button
                key={label}
                id={`filter-color-${label}`}
                className={clsx(styles.colorBtn, filter.colorLabels?.includes(label) && styles.colorActive)}
                style={{ background: `var(--swatch-${label})` }}
                title={label}
                onClick={() => toggleColorLabel(label)}
              />
            ))}
          </div>

          <div className={styles.sep} />

          {/* Rating filter */}
          <select
            id="filter-rating"
            className={styles.select}
            value={filter.rating?.min ?? 0}
            onChange={(e) => {
              const min = parseInt(e.target.value);
              setFilter({ rating: min > 0 ? { min, max: 5 } : undefined });
              search();
            }}
          >
            <option value={0}>Any rating</option>
            <option value={1}>★+</option>
            <option value={2}>★★+</option>
            <option value={3}>★★★+</option>
            <option value={4}>★★★★+</option>
            <option value={5}>★★★★★</option>
          </select>
        </div>

        <div className={styles.right}>
          {/* Sort */}
          <select
            id="filter-sort-field"
            className={styles.select}
            value={sort.field}
            onChange={(e) => { setSort(e.target.value as never, sort.order); search(); }}
          >
            <option value="date">Date</option>
            <option value="name">Name</option>
            <option value="size">Size</option>
            <option value="rating">Rating</option>
            <option value="favorite">Favorites</option>
            <option value="kind">Kind</option>
          </select>
          <button
            id="filter-sort-order"
            className={styles.sortOrderBtn}
            onClick={() => { setSort(sort.field, sort.order === 'asc' ? 'desc' : 'asc'); search(); }}
            title={sort.order === 'asc' ? 'Ascending' : 'Descending'}
          >
            {sort.order === 'asc' ? '↑' : '↓'}
          </button>

          {hasActiveFilters && (
            <button
              id="filter-clear"
              className={styles.clearBtn}
              onClick={clearFilters}
              title="Clear all filters"
            >
              <IconX size={12} />
              Clear
            </button>
          )}
        </div>
      </div>

      {/* ── Row 2: Software / File Type Filter ── */}
      <div className={styles.secondaryRow} id="filter-row-software-ext">
        <div className={styles.left}>
          {/* Mode switch */}
          <div className={styles.modeSwitch}>
            <button
              id="filter-mode-software"
              className={clsx(styles.modeBtn, filterMode === 'software' && styles.modeActive)}
              onClick={() => setFilterMode('software')}
              title="Filter by software application"
            >
              <IconLayers size={13} />
              <span>Software</span>
            </button>
            <button
              id="filter-mode-extension"
              className={clsx(styles.modeBtn, filterMode === 'extension' && styles.modeActive)}
              onClick={() => setFilterMode('extension')}
              title="Filter by exact file extension"
            >
              <IconFileText size={13} />
              <span>File Type</span>
            </button>
          </div>

          <div className={styles.sep} />

          {/* All button */}
          <button
            id="filter-ext-all"
            className={clsx(styles.pill, activeExtCount === 0 && styles.active)}
            onClick={clearExtensions}
          >
            All
          </button>

          {/* Software Mode Pills */}
          {filterMode === 'software' && (
            <div className={styles.pills}>
              {SOFTWARE_OPTIONS.map((sw) => {
                const active = isSoftwareActive(sw);
                return (
                  <button
                    key={sw.id}
                    id={`filter-sw-${sw.id}`}
                    className={clsx(styles.swPill, active && styles.swActive)}
                    style={{
                      '--sw-active-bg': `${sw.color}22`,
                      '--sw-active-border': sw.color,
                      '--sw-active-color': '#ffffff',
                      '--sw-active-glow': `${sw.color}40`,
                    } as React.CSSProperties}
                    onClick={() => toggleSoftware(sw)}
                    title={`Filter by ${sw.label} (${sw.extLabel})`}
                  >
                    <span className={styles.swDot} style={{ background: sw.color }} />
                    <span>{sw.label}</span>
                    <span className={styles.swExtBadge}>{sw.extLabel}</span>
                  </button>
                );
              })}
            </div>
          )}

          {/* Extension Mode Pills */}
          {filterMode === 'extension' && (
            <div className={styles.pills}>
              {EXTENSION_OPTIONS.map(({ ext, label, softwareHint }) => {
                const active = filter.extensions?.includes(ext);
                return (
                  <button
                    key={ext}
                    id={`filter-ext-${ext}`}
                    className={clsx(styles.extPill, active && styles.extActive)}
                    onClick={() => toggleExtension(ext)}
                    title={`${label} (${softwareHint})`}
                  >
                    <span>{label}</span>
                    <span className={styles.extSoftwareHint}>{softwareHint}</span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className={styles.right}>
          {/* Quick Extension Select Dropdown */}
          <select
            id="filter-ext-select"
            className={styles.select}
            value={filter.extensions?.length === 1 ? filter.extensions[0] : ''}
            onChange={(e) => {
              const val = e.target.value;
              if (!val) clearExtensions();
              else {
                setFilter({ extensions: [val] });
                search();
              }
            }}
          >
            <option value="">Specific Extension…</option>
            <optgroup label="3D CAD & Models">
              <option value="max">.max (3ds Max)</option>
              <option value="3dm">.3dm (Rhino)</option>
              <option value="rvt">.rvt (Revit Project)</option>
              <option value="rfa">.rfa (Revit Family)</option>
              <option value="skp">.skp (SketchUp)</option>
              <option value="blend">.blend (Blender)</option>
              <option value="fbx">.fbx (Filmbox 3D)</option>
              <option value="obj">.obj (Wavefront OBJ)</option>
              <option value="3ds">.3ds (3D Studio)</option>
            </optgroup>
            <optgroup label="Materials & Shaders">
              <option value="vrmat">.vrmat (V-Ray Material)</option>
              <option value="mat">.mat (3ds Max Material)</option>
            </optgroup>
            <optgroup label="Vector & Layered">
              <option value="psd">.psd (Photoshop)</option>
              <option value="psb">.psb (Photoshop Big)</option>
              <option value="ai">.ai (Illustrator)</option>
              <option value="svg">.svg (Vector)</option>
            </optgroup>
            <optgroup label="Environment & HDRI">
              <option value="exr">.exr (OpenEXR)</option>
              <option value="hdr">.hdr (Radiance HDR)</option>
            </optgroup>
            <optgroup label="Textures">
              <option value="jpg">.jpg (JPEG)</option>
              <option value="png">.png (PNG)</option>
              <option value="tif">.tif (TIFF)</option>
            </optgroup>
          </select>

          {/* Active extension badge if filtered */}
          {activeExtCount > 0 && (
            <div className={styles.activeBadge}>
              <span>{activeExtCount} {activeExtCount === 1 ? 'type' : 'types'}</span>
              <button
                className={styles.clearMiniBtn}
                onClick={clearExtensions}
                title="Clear extension filter"
              >
                <IconX size={10} />
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
