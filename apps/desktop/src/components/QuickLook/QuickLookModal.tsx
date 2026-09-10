import { memo } from 'react';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { Asset } from '../../types';
import { ThreeViewer } from '../DetailPanel/ThreeViewer';
import { IconStar, IconModel3D, IconImage, IconVideo, IconFileText } from '../Icons';
import styles from './QuickLookModal.module.css';

interface QuickLookModalProps {
  asset: Asset | null;
  isOpen: boolean;
}

export const QuickLookModal = memo(function QuickLookModal({
  asset,
  isOpen,
}: QuickLookModalProps) {
  if (!isOpen || !asset) return null;

  const modelPath =
    (asset.metadata?.preview_model as string) ||
    (asset.thumbnailPath?.endsWith('.glb') ? asset.thumbnailPath : undefined);

  const isNativeImg = ['jpg', 'jpeg', 'png', 'webp', 'bmp', 'gif', 'svg'].includes(
    asset.extension.toLowerCase()
  );

  const imagePath = isNativeImg
    ? asset.filePath
    : asset.thumbnailPath && !asset.thumbnailPath.endsWith('.glb')
    ? asset.thumbnailPath
    : ['image', 'texture'].includes(asset.kind)
    ? asset.filePath
    : undefined;

  const formatSize = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  };

  return (
    <div className={styles.overlay} aria-modal="true" role="dialog">
      <div className={styles.container}>
        {/* Top Floating Header */}
        <div className={styles.header}>
          <div className={styles.headerLeft}>
            <span className={styles.extBadge}>.{asset.extension.toUpperCase()}</span>
            <span className={styles.fileName} title={asset.fileName}>
              {asset.fileName}
            </span>
          </div>
          <div className={styles.headerRight}>
            {asset.colorLabel !== 'none' && (
              <span
                className={styles.colorDot}
                style={{ background: `var(--swatch-${asset.colorLabel})` }}
                title={`Label: ${asset.colorLabel}`}
              />
            )}
            {asset.rating > 0 && (
              <div className={styles.ratingStars}>
                {Array.from({ length: asset.rating }).map((_, i) => (
                  <IconStar key={i} size={13} className={styles.starFilled} />
                ))}
              </div>
            )}
            <span className={styles.sizeBadge}>{formatSize(asset.sizeBytes)}</span>
          </div>
        </div>

        {/* Center Main Preview */}
        <div className={styles.previewContent}>
          {modelPath ? (
            <div className={styles.threeWrapper}>
              <ThreeViewer modelPath={modelPath} />
            </div>
          ) : imagePath ? (
            <div className={styles.imgWrapper}>
              <img
                src={convertFileSrc(imagePath)}
                alt={asset.fileName}
                className={styles.largeImg}
              />
            </div>
          ) : (
            <div className={styles.placeholder}>
              {['3d_model'].includes(asset.kind) ? (
                <IconModel3D size={64} />
              ) : ['video'].includes(asset.kind) ? (
                <IconVideo size={64} />
              ) : ['document'].includes(asset.kind) ? (
                <IconFileText size={64} />
              ) : (
                <IconImage size={64} />
              )}
              <p className={styles.placeholderExt}>.{asset.extension.toUpperCase()}</p>
              <p className={styles.placeholderText}>No preview generated yet</p>
            </div>
          )}
        </div>

        {/* Bottom Floating Footer / Hint */}
        <div className={styles.footer}>
          <span className={styles.filePath} title={asset.filePath}>
            {asset.filePath}
          </span>
          <div className={styles.hintBadge}>
            <kbd className={styles.kbd}>Space</kbd>
            <span>Release to close</span>
          </div>
        </div>
      </div>
    </div>
  );
});
