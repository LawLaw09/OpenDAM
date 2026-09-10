import { useCallback } from 'react';
import clsx from 'clsx';
import { useUIStore, useAssetStore } from '../../store';
import { AssetCard } from './AssetCard';
import { AssetListRow } from './AssetListRow';
import { FilterBar } from './FilterBar';
import type { Asset } from '../../types';
import styles from './AssetGrid.module.css';

const GRID_COLS: Record<'sm' | 'md' | 'lg', number> = {
  sm: 180,
  md: 220,
  lg: 280,
};

export function AssetGrid() {
  const { viewMode, gridSize, selectedAssetIds, selectAsset, clearSelection } = useUIStore();
  const { assets, loading, total } = useAssetStore();

  const tileSize = GRID_COLS[gridSize];

  const handleBackdropClick = useCallback(
    (e: React.MouseEvent) => {
      if (e.target === e.currentTarget) clearSelection();
    },
    [clearSelection]
  );

  const handleAssetClick = useCallback(
    (asset: Asset, e: React.MouseEvent) => {
      selectAsset(asset.id, e.metaKey || e.ctrlKey);
    },
    [selectAsset]
  );

  if (loading && assets.length === 0) {
    return (
      <div className={styles.container}>
        <FilterBar />
        <div className={styles.loadingGrid} style={{ '--tile-size': `${tileSize}px` } as React.CSSProperties}>
          {Array.from({ length: 24 }).map((_, i) => (
            <div key={i} className={clsx(styles.skeletonCard, 'skeleton')} />
          ))}
        </div>
      </div>
    );
  }

  if (!loading && assets.length === 0) {
    return (
      <div className={styles.container}>
        <FilterBar />
        <EmptyState />
      </div>
    );
  }

  return (
    <div className={styles.container}>
      <FilterBar />
      <div className={styles.scrollArea} onClick={handleBackdropClick}>
        {viewMode === 'grid' ? (
          <div
            className={styles.grid}
            style={{ '--tile-size': `${tileSize}px` } as React.CSSProperties}
          >
            {assets.map((asset) => (
              <AssetCard
                key={asset.id}
                asset={asset}
                selected={selectedAssetIds.has(asset.id)}
                onClick={handleAssetClick}
              />
            ))}
          </div>
        ) : (
          <div className={styles.list}>
            {assets.map((asset) => (
              <AssetListRow
                key={asset.id}
                asset={asset}
                selected={selectedAssetIds.has(asset.id)}
                onClick={handleAssetClick}
              />
            ))}
          </div>
        )}
      </div>
      <div className={styles.footer}>
        <span className={styles.footerCount}>
          {assets.length} of {total.toLocaleString()} assets
        </span>
      </div>
    </div>
  );
}

function EmptyState() {
  return (
    <div className={styles.empty}>
      <div className={styles.emptyIcon}>
        <svg viewBox="0 0 80 80" fill="none" xmlns="http://www.w3.org/2000/svg">
          <rect x="10" y="20" width="60" height="48" rx="6"
            fill="hsl(220 13% 16%)" stroke="hsl(220 10% 25%)" strokeWidth="1.5"/>
          <path d="M28 36l8 8 8-8" stroke="hsl(220 85% 50%)" strokeWidth="2"
            strokeLinecap="round" strokeLinejoin="round"/>
          <circle cx="40" cy="12" r="8" fill="hsl(220 85% 50% / 0.15)"
            stroke="hsl(220 85% 50%)" strokeWidth="1.5"/>
          <path d="M40 8v8M36 12h8" stroke="hsl(220 85% 50%)" strokeWidth="1.5"
            strokeLinecap="round"/>
        </svg>
      </div>
      <h2 className={styles.emptyTitle}>No assets found</h2>
      <p className={styles.emptyText}>
        Add a library folder to start browsing your 3D assets, materials, and textures.
      </p>
      <button className={styles.emptyBtn} id="empty-add-library">
        + Add Library
      </button>
    </div>
  );
}
