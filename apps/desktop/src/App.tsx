import { useEffect, useState, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { Sidebar } from './components/Sidebar/Sidebar';
import { Toolbar } from './components/Toolbar/Toolbar';
import { AssetGrid } from './components/AssetGrid/AssetGrid';
import { DetailPanel } from './components/DetailPanel/DetailPanel';
import { StatusBar } from './components/StatusBar/StatusBar';
import { QuickLookModal } from './components/QuickLook/QuickLookModal';
import { IconCheck } from './components/Icons';
import { useUIStore } from './store';
import { useLibraryStore, useTagStore, useCollectionStore, useAssetStore } from './store';
import { api } from './api';
import styles from './App.module.css';

function isEditingText(): boolean {
  const active = document.activeElement;
  if (!active) return false;
  const tag = active.tagName.toLowerCase();
  return tag === 'input' || tag === 'textarea' || (active as HTMLElement).isContentEditable;
}

export default function App() {
  const { sidebarOpen, detailPanelOpen, focusedAssetId } = useUIStore();
  const focusedAsset = useAssetStore((s) =>
    focusedAssetId ? s.assets.find((a) => a.id === focusedAssetId) ?? s.assetMap.get(focusedAssetId) ?? null : null
  );
  const fetchLibraries = useLibraryStore((s) => s.fetchLibraries);
  const fetchTags = useTagStore((s) => s.fetchTags);
  const fetchCollections = useCollectionStore((s) => s.fetchCollections);
  const search = useAssetStore((s) => s.search);

  const [quickLookOpen, setQuickLookOpen] = useState(false);
  const [copyToast, setCopyToast] = useState<string | null>(null);
  const toastTimeoutRef = useRef<number | null>(null);

  useEffect(() => {
    // Bootstrap data on mount
    Promise.all([fetchLibraries(), fetchTags(), fetchCollections()]).then(
      () => search()
    );

    // Real-time preview updates from background engine
    const unlistenDone = listen<{ assetId: string; thumbnailPath?: string; previewModel?: string }>(
      'preview:done',
      (event) => {
        useAssetStore.getState().updateAssetPreview(
          event.payload.assetId,
          event.payload.thumbnailPath,
          event.payload.previewModel
        );
      }
    );

    const unlistenError = listen<{ assetId: string }>(
      'preview:error',
      (event) => {
        useAssetStore.getState().setPreviewStatus(event.payload.assetId, 'error');
      }
    );

    const unlistenLib = listen('library:updated', () => {
      useAssetStore.getState().search();
      useLibraryStore.getState().fetchLibraries();
    });

    // Keyboard shortcuts: F5, Space hold for QuickLook, Ctrl+C to copy path
    const handleKeyDown = (e: KeyboardEvent) => {
      // Refresh
      if (e.key === 'F5' || (e.ctrlKey && e.key.toLowerCase() === 'r')) {
        e.preventDefault();
        api.refreshAllLibraries().then(() => {
          useAssetStore.getState().search();
          useLibraryStore.getState().fetchLibraries();
        }).catch(console.error);
        return;
      }

      // Copy path with Ctrl+C / Cmd+C when asset is selected
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c' && !isEditingText()) {
        const selectedIds = useUIStore.getState().selectedAssetIds;
        const focusedId = useUIStore.getState().focusedAssetId;
        const ids = selectedIds.size > 0 ? Array.from(selectedIds) : focusedId ? [focusedId] : [];
        if (ids.length > 0) {
          const paths = ids
            .map((id) => useAssetStore.getState().getById(id)?.filePath)
            .filter(Boolean) as string[];
          if (paths.length > 0) {
            e.preventDefault();
            navigator.clipboard.writeText(paths.join('\n'));
            setCopyToast(
              paths.length === 1
                ? 'Copied file path to clipboard'
                : `Copied ${paths.length} file paths to clipboard`
            );
            if (toastTimeoutRef.current) clearTimeout(toastTimeoutRef.current);
            toastTimeoutRef.current = window.setTimeout(() => setCopyToast(null), 2200);
          }
        }
      }

      // Hold Spacebar for Quick Look preview
      if (e.code === 'Space' && !isEditingText()) {
        const focusedId = useUIStore.getState().focusedAssetId;
        if (focusedId) {
          e.preventDefault();
          if (!e.repeat) {
            setQuickLookOpen(true);
          }
        }
      }

      // Escape closes Quick Look
      if (e.key === 'Escape') {
        setQuickLookOpen(false);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      if (e.code === 'Space') {
        setQuickLookOpen(false);
      }
    };

    const handleBlur = () => {
      setQuickLookOpen(false);
    };

    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('keyup', handleKeyUp);
    window.addEventListener('blur', handleBlur);

    return () => {
      unlistenDone.then((fn) => fn());
      unlistenError.then((fn) => fn());
      unlistenLib.then((fn) => fn());
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keyup', handleKeyUp);
      window.removeEventListener('blur', handleBlur);
      if (toastTimeoutRef.current) clearTimeout(toastTimeoutRef.current);
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className={styles.layout}>
      <Toolbar />
      <div className={styles.body}>
        {sidebarOpen && <Sidebar />}
        <main className={styles.main}>
          <AssetGrid />
        </main>
        {detailPanelOpen && <DetailPanel />}
      </div>
      <StatusBar />

      {/* Spacebar Quick Look Modal */}
      <QuickLookModal asset={focusedAsset} isOpen={quickLookOpen} />

      {/* Copy Path Feedback Toast */}
      {copyToast && (
        <div className={styles.copyToast}>
          <IconCheck size={14} className={styles.toastCheck} />
          <span>{copyToast}</span>
        </div>
      )}
    </div>
  );
}
