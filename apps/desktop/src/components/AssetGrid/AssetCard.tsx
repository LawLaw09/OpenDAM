import { memo, useCallback, useState, useEffect } from 'react';
import clsx from 'clsx';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Asset, AssetKind } from '../../types';
import { IconModel3D, IconImage, IconVideo, IconStar } from '../Icons';
import styles from './AssetCard.module.css';

export interface AssetCardProps {
  asset: Asset;
  selected: boolean;
  onClick: (asset: Asset, e: React.MouseEvent) => void;
}

const KIND_ICON: Record<AssetKind, React.FC<{ size?: number }>> = {
  '3d_model': IconModel3D,
  material: IconImage,
  texture: IconImage,
  hdri: IconImage,
  ies: IconImage,
  video: IconVideo,
  image: IconImage,
  document: IconImage,
  other: IconImage,
};

const KIND_COLOR: Record<AssetKind, string> = {
  '3d_model':  'hsl(220 85% 55%)',
  material:    'hsl(268 72% 60%)',
  texture:     'hsl(178 64% 46%)',
  hdri:        'hsl( 38 92% 56%)',
  ies:         'hsl( 45 95% 50%)',
  video:       'hsl(  4 80% 55%)',
  image:       'hsl(140 64% 46%)',
  document:    'hsl(213 84% 55%)',
  other:       'hsl(220  8% 50%)',
};

export const AssetCard = memo(function AssetCard({
  asset, selected, onClick,
}: AssetCardProps) {
  const [imgError, setImgError] = useState(false);

  useEffect(() => {
    setImgError(false);
  }, [asset.thumbnailPath, asset.filePath]);

  const KindIcon = KIND_ICON[asset.kind] ?? IconImage;
  const kindColor = KIND_COLOR[asset.kind];

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      onClick(asset, e);
    },
    [asset, onClick]
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
      id={`asset-${asset.id}`}
      className={clsx(styles.card, selected && styles.selected)}
      onClick={handleClick}
      draggable
      onDragStart={handleDragStart}
      title={asset.fileName}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && handleClick(e as unknown as React.MouseEvent)}
    >
      {/* Thumbnail / preview area */}
      <div className={styles.thumb}>
        {asset.thumbnailPath && !asset.thumbnailPath.endsWith('.glb') && !imgError ? (
          <img
            src={convertFileSrc(asset.thumbnailPath)}
            alt={asset.fileName}
            className={styles.thumbImg}
            loading="lazy"
            onError={() => setImgError(true)}
          />
        ) : (['image', 'texture'].includes(asset.kind) && asset.filePath && !imgError) ? (
          <img
            src={convertFileSrc(asset.filePath)}
            alt={asset.fileName}
            className={styles.thumbImg}
            loading="lazy"
            onError={() => setImgError(true)}
          />
        ) : (
          <div className={styles.thumbPlaceholder}>
            <KindIcon size={36} />
          </div>
        )}

        {/* Preview status badge */}
        {asset.previewStatus === 'generating' && (
          <div className={clsx(styles.previewBadge, styles.generating)}>
            <span className="animate-pulse">⟳</span>
          </div>
        )}
        {asset.previewStatus === 'error' && (
          <div className={clsx(styles.previewBadge, styles.error)}>!</div>
        )}

        {/* Color label stripe */}
        {asset.colorLabel !== 'none' && (
          <div
            className={styles.colorStripe}
            style={{ background: `var(--swatch-${asset.colorLabel})` }}
          />
        )}

        {/* Kind chip */}
        <div className={styles.kindChip} style={{ color: kindColor }}>
          <KindIcon size={10} />
          <span>{asset.extension.toUpperCase()}</span>
        </div>
      </div>

      {/* Card body */}
      <div className={styles.body}>
        <p className={clsx(styles.name, 'truncate')} title={asset.fileName}>
          {asset.fileName}
        </p>
        <div className={styles.meta}>
          {asset.rating > 0 && (
            <div className={styles.stars}>
              {Array.from({ length: 5 }).map((_, i) => (
                <IconStar
                  key={i}
                  size={10}
                  className={clsx(styles.star, i < asset.rating && styles.starFilled)}
                />
              ))}
            </div>
          )}
          <span className={styles.size}>{formatSize(asset.sizeBytes)}</span>
        </div>
      </div>
    </div>
  );
});

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes}B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)}KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)}MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(2)}GB`;
}
