import { useState, useEffect, useRef } from 'react';
import clsx from 'clsx';
import { IconTrash, IconX } from './Icons';
import styles from './ConfirmDeleteDialog.module.css';

interface ConfirmDeleteDialogProps {
  isOpen: boolean;
  itemType: 'tag' | 'collection' | 'library';
  itemName: string;
  onConfirm: () => void;
  onCancel: () => void;
}

export function ConfirmDeleteDialog({
  isOpen,
  itemType,
  itemName,
  onConfirm,
  onCancel,
}: ConfirmDeleteDialogProps) {
  const [typedText, setTypedText] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setTypedText('');
      setTimeout(() => {
        inputRef.current?.focus();
      }, 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const normalized = typedText.trim().toLowerCase();
  const isMatched = normalized === 'confirm' || normalized === '(confirm)';
  const isLibrary = itemType === 'library';
  const actionWord = isLibrary ? 'remove' : 'delete';
  const actionWordCap = isLibrary ? 'Remove' : 'Delete';

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (isMatched) {
      onConfirm();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      onCancel();
    }
  };

  return (
    <div className={styles.overlay} onClick={onCancel} onKeyDown={handleKeyDown}>
      <div
        className={styles.dialog}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
      >
        <div className={styles.header}>
          <div className={styles.titleArea}>
            <div className={styles.iconCircle}>
              <IconTrash size={18} />
            </div>
            <h3 className={styles.title}>{actionWordCap} {itemType}</h3>
          </div>
          <button className={styles.closeBtn} onClick={onCancel} title="Close">
            <IconX size={14} />
          </button>
        </div>

        <div className={styles.content}>
          <p className={styles.warningText}>
            Are you sure you want to {actionWord} the {itemType}{' '}
            <strong className={styles.targetName}>"{itemName}"</strong>?
          </p>
          {isLibrary ? (
            <p className={styles.subText}>
              This will remove the library from OpenDAM. <strong style={{ color: 'var(--color-text-main, #ffffff)' }}>Your original files on disk will NOT be deleted.</strong> To verify, type{' '}
              <code className={styles.confirmWord}>confirm</code> below:
            </p>
          ) : (
            <p className={styles.subText}>
              This action cannot be undone. To verify, type{' '}
              <code className={styles.confirmWord}>confirm</code> below:
            </p>
          )}

          <form onSubmit={handleSubmit} className={styles.form}>
            <input
              ref={inputRef}
              type="text"
              className={styles.input}
              placeholder='Type "confirm" to verify'
              value={typedText}
              onChange={(e) => setTypedText(e.target.value)}
              autoComplete="off"
              spellCheck={false}
            />

            <div className={styles.actions}>
              <button
                type="button"
                className={clsx(styles.btn, styles.cancelBtn)}
                onClick={onCancel}
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={!isMatched}
                className={clsx(
                  styles.btn,
                  styles.deleteBtn,
                  isMatched && styles.deleteBtnActive
                )}
              >
                {actionWordCap} {itemType}
              </button>
            </div>
          </form>
        </div>
      </div>
    </div>
  );
}

