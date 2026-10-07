import { useCallback, useState, useMemo, useEffect } from 'react';
import clsx from 'clsx';
import { useUIStore, useAssetStore, useTagStore, useCollectionStore } from '../../store';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Asset, ColorLabel, Collection } from '../../types';
import { IconStar, IconX, IconExternal, IconEye, IconInfo, IconCopy, IconCheck, IconRefresh, IconEdit, IconTrash, IconHeart, IconHeartFilled } from '../Icons';
import { ConfirmDeleteDialog } from '../ConfirmDeleteDialog';
import { api } from '../../api';
import { ThreeViewer } from './ThreeViewer';
import styles from './DetailPanel.module.css';

const COLOR_LABELS: ColorLabel[] = [
  'red', 'orange', 'yellow', 'green', 'teal', 'blue', 'purple', 'pink',
];

export function DetailPanel() {
  const focusedAssetId = useUIStore((s) => s.focusedAssetId);
  const asset = useAssetStore((s) =>
    focusedAssetId ? s.assets.find((a) => a.id === focusedAssetId) ?? s.assetMap.get(focusedAssetId) ?? null : null
  );
  const patchAsset = useAssetStore((s) => s.patchAsset);
  const deleteAsset = useAssetStore((s) => s.deleteAsset);
  const renameAsset = useAssetStore((s) => s.renameAsset);
  const tags = useTagStore((s) => s.tags);
  const collections = useCollectionStore((s) => s.collections);
  const addToCollection = useCollectionStore((s) => s.addToCollection);
  const removeFromCollection = useCollectionStore((s) => s.removeFromCollection);
  const [tab, setTab] = useState<'info' | 'tags' | 'collections' | 'preview'>('info');

  const [isEditingName, setIsEditingName] = useState(false);
  const [editingName, setEditingName] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [isDeleteDialogOpen, setIsDeleteDialogOpen] = useState(false);

  useEffect(() => {
    setIsEditingName(false);
    setNameError(null);
    if (asset) {
      setEditingName(asset.fileName);
    }
  }, [asset?.id]);

  const setRating = useCallback(
    (rating: number) => asset && patchAsset(asset.id, { rating }),
    [asset, patchAsset]
  );

  const setColorLabel = useCallback(
    (label: ColorLabel) => asset && patchAsset(asset.id, { colorLabel: label === asset.colorLabel ? 'none' : label }),
    [asset, patchAsset]
  );

  const toggleFavorite = useCallback(
    () => asset && patchAsset(asset.id, { isFavorite: !asset.isFavorite }),
    [asset, patchAsset]
  );

  const toggleTag = useCallback(
    (tagId: string) => {
      if (!asset) return;
      const next = asset.tags.includes(tagId)
        ? asset.tags.filter((t) => t !== tagId)
        : [...asset.tags, tagId];
      patchAsset(asset.id, { tags: next });
    },
    [asset, patchAsset]
  );

  const setPreviewStatus = useAssetStore((s) => s.setPreviewStatus);
  const handleGeneratePreview = useCallback(async () => {
    if (!asset) return;
    setPreviewStatus(asset.id, 'pending');
    try {
      await api.requestPreview([asset.id]);
    } catch (err) {
      console.error('Failed to request preview:', err);
      setPreviewStatus(asset.id, 'error');
    }
  }, [asset, setPreviewStatus]);

  const handleStartRename = () => {
    if (!asset) return;
    setIsEditingName(true);
    setEditingName(asset.fileName);
    setNameError(null);
  };

  const handleSaveRename = async () => {
    if (!asset) return;
    const trimmed = editingName.trim();
    if (!trimmed || trimmed === asset.fileName) {
      setIsEditingName(false);
      setNameError(null);
      return;
    }
    try {
      await renameAsset(asset.id, trimmed);
      setIsEditingName(false);
      setNameError(null);
    } catch (err: any) {
      setNameError(err?.message || String(err));
    }
  };

  const handleConfirmDelete = async () => {
    if (!asset) return;
    setIsDeleteDialogOpen(false);
    try {
      await deleteAsset(asset.id, true);
    } catch (err) {
      console.error('Failed to delete asset:', err);
    }
  };

  if (!asset) {
    return (
      <aside className={styles.panel} id="detail-panel">
        <div className={styles.empty}>
          <IconInfo size={32} />
          <p>Select an asset to see details</p>
        </div>
      </aside>
    );
  }

  const isPending = asset.previewStatus === 'pending' || asset.previewStatus === 'generating';
  const isError = asset.previewStatus === 'error';

  return (
    <aside className={styles.panel} id="detail-panel">
      {/* Preview thumbnail */}
      <div className={styles.thumbArea}>
        {['jpg', 'jpeg', 'png', 'webp', 'bmp', 'gif', 'svg'].includes(asset.extension.toLowerCase()) ? (
          <img src={convertFileSrc(asset.filePath)} alt={asset.fileName} className={styles.thumbImg} />
        ) : asset.thumbnailPath && !asset.thumbnailPath.endsWith('.glb') ? (
          <img src={convertFileSrc(asset.thumbnailPath)} alt={asset.fileName} className={styles.thumbImg} />
        ) : (['image', 'texture'].includes(asset.kind) && asset.filePath) ? (
          <img src={convertFileSrc(asset.filePath)} alt={asset.fileName} className={styles.thumbImg} />
        ) : (asset.kind === 'video' && asset.filePath) ? (
          <video
            src={convertFileSrc(asset.filePath)}
            preload="metadata"
            muted
            playsInline
            controls
            className={styles.thumbImg}
            style={{ objectFit: 'contain' }}
            onLoadedMetadata={(e) => {
              try { e.currentTarget.currentTime = 0.1; } catch {}
            }}
          />
        ) : (asset.metadata?.preview_model || asset.thumbnailPath?.endsWith('.glb')) ? (
          <ThreeViewer modelPath={(asset.metadata?.preview_model as string) || asset.thumbnailPath!} />
        ) : (
          <div className={styles.thumbPlaceholder}>
            <span className={styles.thumbExt}>.{asset.extension}</span>
            <button
              className={styles.thumbGenerateBtn}
              onClick={handleGeneratePreview}
              disabled={isPending}
              title={isError ? "Preview failed. Click to retry" : "Generate preview"}
            >
              <IconRefresh size={12} className={clsx(isPending && 'animate-spin')} />
              <span>{isPending ? 'Generating…' : isError ? 'Retry' : 'Generate'}</span>
            </button>
          </div>
        )}
      </div>

      {/* File name + actions */}
      <div className={styles.header}>
        {isEditingName ? (
          <form
            className={styles.nameEditForm}
            onSubmit={(e) => {
              e.preventDefault();
              handleSaveRename();
            }}
          >
            <input
              type="text"
              autoFocus
              className={styles.nameInput}
              value={editingName}
              onChange={(e) => {
                setEditingName(e.target.value);
                if (nameError) setNameError(null);
              }}
              onKeyDown={(e) => {
                if (e.key === 'Escape') {
                  setIsEditingName(false);
                  setNameError(null);
                }
              }}
            />
            <button
              type="submit"
              className={clsx(styles.nameActionBtn, styles.nameActionBtnConfirm)}
              title="Save name"
            >
              <IconCheck size={13} />
            </button>
            <button
              type="button"
              className={clsx(styles.nameActionBtn, styles.nameActionBtnCancel)}
              onClick={() => {
                setIsEditingName(false);
                setNameError(null);
              }}
              title="Cancel"
            >
              <IconX size={13} />
            </button>
          </form>
        ) : (
          <p
            className={clsx(styles.fileName, 'truncate')}
            title="Double-click or click edit icon to rename"
            onDoubleClick={handleStartRename}
            style={{ cursor: 'pointer' }}
          >
            {asset.fileName}
          </p>
        )}

        <div className={styles.headerActions}>
          <button
            id="detail-favorite"
            className={clsx(styles.actionBtn, asset.isFavorite && styles.actionBtnFavorite)}
            title={asset.isFavorite ? 'Remove from favorites' : 'Add to favorites'}
            onClick={toggleFavorite}
          >
            {asset.isFavorite ? (
              <IconHeartFilled size={14} className={styles.favoriteHeartFilled} />
            ) : (
              <IconHeart size={14} />
            )}
          </button>
          <button
            id="detail-rename"
            className={styles.actionBtn}
            title="Rename file (F2)"
            onClick={handleStartRename}
          >
            <IconEdit size={14} />
          </button>
          <button
            id="detail-gen-preview-header"
            className={clsx(styles.actionBtn, isPending && styles.actionBtnLoading)}
            title={isPending ? "Generating preview…" : "Generate/Regenerate preview"}
            onClick={handleGeneratePreview}
            disabled={isPending}
          >
            <IconRefresh size={14} className={clsx(isPending && 'animate-spin')} />
          </button>
          <button
            id="detail-open-external"
            className={styles.actionBtn}
            title="Open with default app"
            onClick={async () => {
              try {
                await api.openWithDefault(asset.filePath);
              } catch (err) {
                console.error('Failed to open with default app:', err);
              }
            }}
          >
            <IconExternal size={14} />
          </button>
          <button
            id="detail-reveal"
            className={styles.actionBtn}
            title="Reveal in Explorer"
            onClick={async () => {
              try {
                await api.revealInExplorer(asset.filePath);
              } catch (err) {
                console.error('Failed to reveal in explorer:', err);
              }
            }}
          >
            <IconEye size={14} />
          </button>
          <button
            id="detail-copy-path"
            className={styles.actionBtn}
            title="Copy path"
            onClick={() => navigator.clipboard.writeText(asset.filePath)}
          >
            <IconCopy size={14} />
          </button>
          <button
            id="detail-delete"
            className={clsx(styles.actionBtn, styles.actionBtnDanger)}
            title="Delete file (Move to Recycle Bin)"
            onClick={() => setIsDeleteDialogOpen(true)}
          >
            <IconTrash size={14} />
          </button>
        </div>
      </div>

      {nameError && (
        <div style={{ color: '#ef4444', fontSize: '11px', padding: '2px 16px', background: 'rgba(239, 68, 68, 0.1)' }}>
          {nameError}
        </div>
      )}

      {/* Tabs */}
      <div className={styles.tabs}>
        {(['info', 'tags', 'collections', 'preview'] as const).map((t) => (
          <button
            key={t}
            id={`detail-tab-${t}`}
            className={clsx(styles.tab, tab === t && styles.tabActive)}
            onClick={() => setTab(t)}
          >
            {t.charAt(0).toUpperCase() + t.slice(1)}
          </button>
        ))}
      </div>

      <div className={styles.tabContent}>
        {tab === 'info' && (
          <InfoTab
            asset={asset}
            onRatingChange={setRating}
            onColorLabelChange={setColorLabel}
            onFavoriteChange={toggleFavorite}
          />
        )}
        {tab === 'tags' && (
          <TagsTab asset={asset} tags={tags} onToggle={toggleTag} />
        )}
        {tab === 'collections' && (
          <CollectionsTab
            asset={asset}
            collections={collections}
            onAdd={async (colId) => {
              await addToCollection(colId, [asset.id]);
              useCollectionStore.getState().fetchCollections();
            }}
            onRemove={async (colId) => {
              await removeFromCollection(colId, [asset.id]);
              useCollectionStore.getState().fetchCollections();
            }}
          />
        )}
        {tab === 'preview' && (
          <PreviewTab asset={asset} onGeneratePreview={handleGeneratePreview} />
        )}
      </div>

      <ConfirmDeleteDialog
        isOpen={isDeleteDialogOpen}
        itemType="asset"
        itemName={asset.fileName}
        onConfirm={handleConfirmDelete}
        onCancel={() => setIsDeleteDialogOpen(false)}
      />
    </aside>
  );
}

// ── Info Tab ──────────────────────────────────────────────────────────────
function InfoTab({
  asset, onRatingChange, onColorLabelChange, onFavoriteChange,
}: {
  asset: Asset;
  onRatingChange: (r: number) => void;
  onColorLabelChange: (l: ColorLabel) => void;
  onFavoriteChange: () => void;
}) {
  const [hoverRating, setHoverRating] = useState(0);

  return (
    <div className={styles.infoGrid}>
      {/* Favorite */}
      <label className={styles.infoLabel}>Favorite</label>
      <div className={styles.favoriteRow}>
        <button
          id="detail-info-favorite"
          type="button"
          className={clsx(styles.infoFavoriteBtn, asset.isFavorite && styles.infoFavoriteActive)}
          onClick={onFavoriteChange}
        >
          {asset.isFavorite ? (
            <>
              <IconHeartFilled size={14} className={styles.favoriteHeartFilled} />
              <span>In Favorites</span>
            </>
          ) : (
            <>
              <IconHeart size={14} />
              <span>Add to Favorites</span>
            </>
          )}
        </button>
      </div>
      {/* Rating */}
      <label className={styles.infoLabel}>Rating</label>
      <div className={styles.starRow}>
        {Array.from({ length: 5 }).map((_, i) => {
          const n = i + 1;
          return (
            <button
              key={n}
              id={`detail-star-${n}`}
              className={clsx(
                styles.starBtn,
                (hoverRating || asset.rating) >= n && styles.starOn
              )}
              onMouseEnter={() => setHoverRating(n)}
              onMouseLeave={() => setHoverRating(0)}
              onClick={() => onRatingChange(asset.rating === n ? 0 : n)}
            >
              <IconStar size={16} />
            </button>
          );
        })}
      </div>

      {/* Color label */}
      <label className={styles.infoLabel}>Label</label>
      <div className={styles.colorRow}>
        <button
          className={clsx(styles.colorNoneBtn, asset.colorLabel === 'none' && styles.colorNoneActive)}
          onClick={() => onColorLabelChange('none')}
          title="No label"
        >
          <IconX size={10} />
        </button>
        {COLOR_LABELS.map((label) => (
          <button
            key={label}
            id={`detail-color-${label}`}
            className={clsx(styles.colorDot, asset.colorLabel === label && styles.colorDotActive)}
            style={{ background: `var(--swatch-${label})` }}
            title={label}
            onClick={() => onColorLabelChange(label)}
          />
        ))}
      </div>

      {/* File info */}
      <label className={styles.infoLabel}>Path</label>
      <span
        className={clsx(styles.infoValue, styles.mono, 'truncate')}
        title={`${asset.filePath} (Click to reveal in Explorer)`}
        style={{ cursor: 'pointer' }}
        onClick={async () => {
          try {
            await api.revealInExplorer(asset.filePath);
          } catch (err) {
            console.error('Failed to reveal in explorer:', err);
          }
        }}
      >
        {asset.filePath}
      </span>

      <label className={styles.infoLabel}>Format</label>
      <span className={styles.infoValue}>.{asset.extension.toUpperCase()}</span>

      <label className={styles.infoLabel}>Kind</label>
      <span className={styles.infoValue}>{asset.kind.replace('_', ' ')}</span>

      <label className={styles.infoLabel}>Size</label>
      <span className={clsx(styles.infoValue, styles.mono)}>
        {(asset.sizeBytes / 1024 / 1024).toFixed(2)} MB
      </span>

      <label className={styles.infoLabel}>Modified</label>
      <span className={styles.infoValue}>
        {new Date(asset.modifiedAt).toLocaleDateString()}
      </span>

      <label className={styles.infoLabel}>Added</label>
      <span className={styles.infoValue}>
        {new Date(asset.indexedAt).toLocaleDateString()}
      </span>

      {/* Extracted metadata */}
      {Object.entries(asset.metadata).length > 0 && (
        <>
          <div className={styles.metaDivider} />
          <label className={clsx(styles.infoLabel, styles.metaHeader)}>Metadata</label>
          <div />
          {Object.entries(asset.metadata).map(([k, v]) => (
            <>
              <label key={`k-${k}`} className={clsx(styles.infoLabel, styles.metaKey)}>{k}</label>
              <span key={`v-${k}`} className={clsx(styles.infoValue, styles.mono)}>
                {String(v)}
              </span>
            </>
          ))}
        </>
      )}
    </div>
  );
}

// ── Tags Tab ──────────────────────────────────────────────────────────────
function TagsTab({
  asset, tags, onToggle,
}: {
  asset: Asset;
  tags: import('../../types').Tag[];
  onToggle: (id: string) => void;
}) {
  const assigned = tags.filter((t) => asset.tags.includes(t.id));
  const unassigned = tags.filter((t) => !asset.tags.includes(t.id));

  return (
    <div className={styles.tagsSection}>
      {tags.length === 0 && (
        <p className={styles.noTags}>No tags yet. Create tags in the sidebar.</p>
      )}

      {assigned.length > 0 && (
        <>
          <p className={styles.tagsGroupLabel}>Assigned</p>
          <div className={styles.tagsGrid}>
            {assigned.map((tag) => (
              <button
                key={tag.id}
                id={`detail-tag-${tag.id}`}
                className={clsx(styles.tagPill, styles.tagActive)}
                onClick={() => onToggle(tag.id)}
                title="Click to remove"
              >
                <span
                  className={styles.tagDot}
                  style={{ background: tag.color ? `var(--swatch-${tag.color})` : 'var(--color-text-muted)' }}
                />
                <span className={styles.tagPillName}>{tag.name}</span>
                <IconCheck size={11} className={styles.tagCheckIcon} />
              </button>
            ))}
          </div>
        </>
      )}

      {unassigned.length > 0 && (
        <>
          <p className={styles.tagsGroupLabel}>{assigned.length > 0 ? 'Available' : 'All Tags'}</p>
          <div className={styles.tagsGrid}>
            {unassigned.map((tag) => (
              <button
                key={tag.id}
                id={`detail-tag-${tag.id}`}
                className={styles.tagPill}
                onClick={() => onToggle(tag.id)}
                title="Click to assign"
              >
                <span
                  className={styles.tagDot}
                  style={{ background: tag.color ? `var(--swatch-${tag.color})` : 'var(--color-text-muted)' }}
                />
                <span className={styles.tagPillName}>{tag.name}</span>
              </button>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

// ── Collections Tab ───────────────────────────────────────────────────────
function CollectionsTab({
  asset, collections, onAdd, onRemove,
}: {
  asset: Asset;
  collections: Collection[];
  onAdd: (collectionId: string) => void;
  onRemove: (collectionId: string) => void;
}) {
  const regularCollections = useMemo(
    () => collections.filter((c: Collection) => !c.isSmart),
    [collections]
  );

  const getCollectionPath = (col: Collection): string => {
    const parts = [col.name];
    let curr = col;
    while (curr.parentId) {
      const parent = regularCollections.find((c: Collection) => c.id === curr.parentId);
      if (!parent) break;
      parts.unshift(parent.name);
      curr = parent;
    }
    return parts.join(' / ');
  };

  return (
    <div className={styles.tagsGrid}>
      {regularCollections.length === 0 && (
        <p className={styles.noTags}>No collections yet. Create collections in the sidebar.</p>
      )}
      {regularCollections.map((col: Collection) => {
        const isMember = (col.assetIds && col.assetIds.includes(asset.id)) || (asset.collections?.includes(col.id) ?? false);
        const pathName = getCollectionPath(col);
        return (
          <button
            key={col.id}
            id={`detail-col-${col.id}`}
            className={clsx(styles.tagPill, isMember && styles.tagActive)}
            onClick={() => isMember ? onRemove(col.id) : onAdd(col.id)}
            title={pathName}
          >
            <span className={styles.tagDot} style={{ background: 'var(--color-accent)' }} />
            {pathName}
            <span className={styles.tagCount}>({col.assetIds.length})</span>
          </button>
        );
      })}
    </div>
  );
}

// ── Preview Tab ───────────────────────────────────────────────────────────
function PreviewTab({
  asset,
  onGeneratePreview,
}: {
  asset: Asset;
  onGeneratePreview: () => void;
}) {
  const modelPath = (asset.metadata?.preview_model as string) || (asset.thumbnailPath?.endsWith('.glb') ? asset.thumbnailPath : undefined);
  const imagePath = asset.thumbnailPath && !asset.thumbnailPath.endsWith('.glb') ? asset.thumbnailPath : (['image', 'texture'].includes(asset.kind) ? asset.filePath : undefined);
  const isPending = asset.previewStatus === 'pending' || asset.previewStatus === 'generating';
  const isError = asset.previewStatus === 'error';

  return (
    <div className={styles.previewArea}>
      {modelPath ? (
        <div className={styles.previewViewerWrapper}>
          <ThreeViewer modelPath={modelPath} />
          <button
            className={styles.reGenerateOverlayBtn}
            onClick={onGeneratePreview}
            disabled={isPending}
            title="Re-generate preview"
          >
            <IconRefresh size={13} className={clsx(isPending && 'animate-spin')} />
            <span>{isPending ? 'Generating…' : 'Regenerate Preview'}</span>
          </button>
        </div>
      ) : (asset.kind === 'video' && asset.filePath) ? (
        <div className={styles.previewViewerWrapper}>
          <video
            src={convertFileSrc(asset.filePath)}
            controls
            autoPlay
            muted
            playsInline
            className={styles.previewImg}
            style={{ width: '100%', height: '100%', objectFit: 'contain', background: '#000' }}
          />
        </div>
      ) : imagePath ? (
        <div className={styles.previewViewerWrapper}>
          <img src={convertFileSrc(imagePath)} alt={asset.fileName} className={styles.previewImg} />
          <button
            className={styles.reGenerateOverlayBtn}
            onClick={onGeneratePreview}
            disabled={isPending}
            title="Re-generate preview"
          >
            <IconRefresh size={13} className={clsx(isPending && 'animate-spin')} />
            <span>{isPending ? 'Generating…' : 'Regenerate Preview'}</span>
          </button>
        </div>
      ) : (
        <div className={styles.previewEmpty}>
          <div className={styles.emptyStatusIcon}>
            {isPending ? (
              <span className={clsx(styles.spinnerBig, 'animate-spin')}>⟳</span>
            ) : isError ? (
              <span className={styles.errorExclamation}>!</span>
            ) : (
              <IconEye size={36} />
            )}
          </div>
          <p className={styles.emptyStatusTitle}>
            {isPending
              ? 'Generating preview…'
              : isError
              ? 'Preview generation failed previously'
              : 'No preview available'}
          </p>
          <p className={styles.emptyStatusSub}>
            {isPending
              ? 'Please wait while the worker renders the model'
              : 'Click below to run the preview engine worker for this file'}
          </p>
          <button
            id="detail-generate-preview"
            className={clsx(styles.generateBtn, isPending && styles.generateBtnLoading)}
            onClick={onGeneratePreview}
            disabled={isPending}
          >
            <IconRefresh size={14} className={clsx(isPending && 'animate-spin')} />
            <span>{isPending ? 'Generating…' : isError ? 'Retry Generate Preview' : 'Generate Preview'}</span>
          </button>
        </div>
      )}
    </div>
  );
}
