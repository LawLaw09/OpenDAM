import { useCallback, useState } from 'react';
import { useUIStore, useAssetStore, useLibraryStore } from '../../store';
import { api } from '../../api';
import { open } from '@tauri-apps/plugin-dialog';
import {
  IconGrid, IconList, IconSearch, IconSidebar,
  IconPanel, IconRefresh, IconPlus,
} from '../Icons';
import styles from './Toolbar.module.css';
import clsx from 'clsx';

export function Toolbar() {
  const { viewMode, gridSize, sidebarOpen, detailPanelOpen,
          setViewMode, setGridSize, toggleSidebar, toggleDetailPanel } = useUIStore();
  const { query, setQuery, search, loading } = useAssetStore();
  const { addLibrary } = useLibraryStore();
  const [isRefreshing, setIsRefreshing] = useState(false);

  const handleRefresh = async () => {
    setIsRefreshing(true);
    try {
      await api.refreshAllLibraries();
      await search();
    } catch (err) {
      console.error('Failed to refresh libraries:', err);
    } finally {
      setIsRefreshing(false);
    }
  };

  const handleAddLibrary = async () => {
    try {
      const selectedPath = await open({
        directory: true,
        multiple: false,
        title: 'Select Library Folder'
      });
      if (selectedPath && typeof selectedPath === 'string') {
        const name = selectedPath.split(/[/\\]/).pop() || 'New Library';
        await addLibrary(name, [selectedPath]);
      }
    } catch (err) {
      console.error('Failed to add library:', err);
    }
  };

  const handleSearch = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setQuery({ text: e.target.value });
      search();
    },
    [setQuery, search]
  );

  return (
    <header className={styles.toolbar} data-tauri-drag-region>
      {/* App logo + name */}
      <div className={styles.brand}>
        <div className={styles.logo}>
          <svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path d="M12 2L4 6v12l8 4 8-4V6L12 2Z" fill="hsl(220 85% 50%)" opacity="0.9"/>
            <path d="M12 2v18M4 6l8 4 8-4" stroke="hsl(220 85% 70%)" strokeWidth="1.5" strokeLinecap="round"/>
          </svg>
        </div>
        <span className={styles.appName}>OpenDAM</span>
      </div>

      {/* Search area with recursive subcollection filter toggle */}
      <div className={styles.searchContainer}>
        <label
          className={clsx(
            styles.subcollectionsCheckboxLabel,
            query.includeSubcollections !== false && styles.subcollectionsCheckboxActive
          )}
          title={
            query.includeSubcollections !== false
              ? 'Filtering includes child collections (Click to filter selected collection only)'
              : 'Filtering only includes selected collection (Click to include children too)'
          }
        >
          <input
            id="toggle-include-subcollections"
            type="checkbox"
            className={styles.subcollectionsCheckboxInput}
            checked={query.includeSubcollections !== false}
            onChange={(e) => {
              setQuery({ includeSubcollections: e.target.checked });
              search();
            }}
          />
          <span className={styles.subcollectionsCheckboxCustom}>
            {query.includeSubcollections !== false && (
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={3} strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            )}
          </span>
          <span className={styles.subcollectionsCheckboxText}>Include subcollections</span>
        </label>

        {/* Search bar */}
        <div className={styles.searchWrap}>
          <IconSearch className={styles.searchIcon} />
          <input
            id="toolbar-search"
            type="search"
            placeholder="Search assets, tags, metadata…"
            className={styles.searchInput}
            value={query.text}
            onChange={handleSearch}
            aria-label="Search assets"
          />
          {loading && <div className={clsx(styles.spinner, 'animate-spin')} />}
        </div>
      </div>

      {/* Right controls */}
      <div className={styles.controls}>
        {/* Grid size */}
        {viewMode === 'grid' && (
          <div className={styles.buttonGroup}>
            {(['sm', 'md', 'lg'] as const).map((size) => (
              <button
                key={size}
                id={`grid-size-${size}`}
                className={clsx(styles.iconBtn, gridSize === size && styles.active)}
                onClick={() => setGridSize(size)}
                title={`Grid size: ${size}`}
              >
                <span className={styles.gridSizeDot} style={{ fontSize: size === 'sm' ? 8 : size === 'md' ? 10 : 12 }}>⬛</span>
              </button>
            ))}
          </div>
        )}

        {/* View mode */}
        <div className={styles.buttonGroup}>
          <button
            id="view-grid"
            className={clsx(styles.iconBtn, viewMode === 'grid' && styles.active)}
            onClick={() => setViewMode('grid')}
            title="Grid view"
          >
            <IconGrid />
          </button>
          <button
            id="view-list"
            className={clsx(styles.iconBtn, viewMode === 'list' && styles.active)}
            onClick={() => setViewMode('list')}
            title="List view"
          >
            <IconList />
          </button>
        </div>

        <div className={styles.divider} />

        <button
          id="toggle-sidebar"
          className={clsx(styles.iconBtn, !sidebarOpen && styles.dimmed)}
          onClick={toggleSidebar}
          title="Toggle sidebar"
        >
          <IconSidebar />
        </button>
        <button
          id="toggle-detail"
          className={clsx(styles.iconBtn, !detailPanelOpen && styles.dimmed)}
          onClick={toggleDetailPanel}
          title="Toggle detail panel"
        >
          <IconPanel />
        </button>

        <div className={styles.divider} />

        <button
          id="refresh-library"
          className={styles.iconBtn}
          onClick={handleRefresh}
          title="Refresh and scan libraries"
        >
          <IconRefresh className={loading || isRefreshing ? 'animate-spin' : undefined} />
        </button>

        <button
          id="add-library"
          className={styles.primaryBtn}
          title="Add Library"
          onClick={handleAddLibrary}
        >
          <IconPlus />
          <span>Add Library</span>
        </button>
      </div>
    </header>
  );
}
