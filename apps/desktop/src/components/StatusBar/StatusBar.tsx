import { useAssetStore } from '../../store';
import styles from './StatusBar.module.css';

export function StatusBar() {
  const { total, loading, query } = useAssetStore();

  return (
    <footer className={styles.statusBar} id="status-bar">
      <div className={styles.left}>
        {loading ? (
          <span className={styles.item}>Indexing…</span>
        ) : (
          <span className={styles.item}>
            {total.toLocaleString()} asset{total !== 1 ? 's' : ''}
          </span>
        )}
        {query.text && (
          <span className={styles.item}>
            · Searching: <em>"{query.text}"</em>
          </span>
        )}
      </div>
      <div className={styles.right}>
        <span className={styles.item}>OpenDAM v0.1.0</span>
      </div>
    </footer>
  );
}
