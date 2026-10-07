import { memo, useCallback } from 'react';
import clsx from 'clsx';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Asset } from '../../types';
import { useAssetStore } from '../../store';
import { IconImage, IconVideo, IconStar, IconHeart, IconHeartFilled } from '../Icons';
import styles from './AssetListRow.module.css';

interface AssetListRowProps {
  asset: Asset;
  selected: boolean;
  onClick: (asset: Asset, e: React.MouseEvent) => void;
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)}GB`;
}

function formatDate(ms: number): string {
  const d = new Date(ms);
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
}

export const AssetListRow = memo(function AssetListRow({
  asset, selected, onClick,
}: AssetListRowProps) {
  const patchAsset = useAssetStore((s) => s.patchAsset);

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onClick(asset, e);
    },
    [asset, onClick]
  );

  const handleToggleFavorite = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      patchAsset(asset.id, { isFavorite: !asset.isFavorite });
    },
    [asset.id, asset.isFavorite, patchAsset]
  );

  const handleDragStart = useCallback(
    (e: React.DragEvent) => {
      e.dataTransfer.setData('text/plain', asset.filePath);
      e.dataTransfer.effectAllowed = 'copy';
    },
    [asset.filePath]
  );

  return (
    <div
      id={`asset-row-${asset.id}`}
      className={clsx(styles.row, selected && styles.selected)}
      onClick={handleClick}
      draggable
      onDragStart={handleDragStart}
      role="button"
      tabIndex={0}
    >
      {/* Favorite Button */}
      <button
        id={`row-fav-${asset.id}`}
        type="button"
        className={clsx(styles.favoriteBtn, asset.isFavorite && styles.favoriteActive)}
        onClick={handleToggleFavorite}
        title={asset.isFavorite ? 'Remove from favorites' : 'Add to favorites'}
        aria-label={asset.isFavorite ? 'Remove from favorites' : 'Add to favorites'}
      >
        {asset.isFavorite ? (
          <IconHeartFilled size={14} className={styles.favoriteHeartFilled} />
        ) : (
          <IconHeart size={14} />
        )}
      </button>

      {/* Thumbnail */}
      <div className={styles.thumb}>
        {asset.thumbnailPath && !asset.thumbnailPath.endsWith('.glb') ? (
          <img src={convertFileSrc(asset.thumbnailPath)} alt={asset.fileName} className={styles.thumbImg} loading="lazy" />
        ) : (['image', 'texture'].includes(asset.kind) && asset.filePath) ? (
          <img src={convertFileSrc(asset.filePath)} alt={asset.fileName} className={styles.thumbImg} loading="lazy" />
        ) : (
          <div className={styles.thumbPlaceholder}>
            {asset.kind === 'video' ? <IconVideo size={14} /> : <IconImage size={14} />}
          </div>
        )}
      </div>

      {/* Color label */}
      {asset.colorLabel !== 'none' && (
        <div
          className={styles.colorDot}
          style={{ background: `var(--swatch-${asset.colorLabel})` }}
        />
      )}

      {/* Name */}
      <div className={styles.nameCol}>
        <span className={clsx(styles.name, 'truncate')}>{asset.fileName}</span>
        <span className={clsx(styles.path, 'truncate')}>{asset.filePath}</span>
      </div>

      {/* Extension */}
      <span className={styles.ext}>{asset.extension.toUpperCase()}</span>

      {/* Rating */}
      <div className={styles.stars}>
        {Array.from({ length: 5 }).map((_, i) => (
          <IconStar key={i} size={10} className={clsx(styles.star, i < asset.rating && styles.starFilled)} />
        ))}
      </div>

      {/* Size */}
      <span className={styles.size}>{formatSize(asset.sizeBytes)}</span>

      {/* Date */}
      <span className={styles.date}>{formatDate(asset.modifiedAt)}</span>
    </div>
  );
});
