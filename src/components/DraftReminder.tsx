import React from 'react';

interface DraftReminderProps {
  draftCount: number;
  onView: () => void;
  onApply: () => void;
  onDiscard: () => void;
  onDismiss: () => void;
}

export const DraftReminder: React.FC<DraftReminderProps> = ({
  draftCount,
  onView,
  onApply,
  onDiscard,
  onDismiss,
}) => {
  return (
    <div style={styles.container}>
      <div style={styles.content}>
        <span style={styles.text}>您有 {draftCount} 条待应用的更改。</span>
        <div style={styles.actions}>
          <button onClick={onView} style={styles.link}>
            查看
          </button>
          <span style={styles.separator}>·</span>
          <button onClick={onApply} style={{ ...styles.link, color: '#B65B3F' }}>
            应用
          </button>
          <span style={styles.separator}>·</span>
          <button onClick={onDiscard} style={styles.link}>
            放弃
          </button>
        </div>
      </div>
      <button onClick={onDismiss} style={styles.close} aria-label="关闭提醒">
        ×
      </button>
    </div>
  );
};

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    backgroundColor: '#FFF9F0',
    border: '1px solid #F4E4C8',
    borderRadius: '8px',
    padding: '12px 40px 12px 16px',
    position: 'relative',
    fontFamily: 'Inter, -apple-system, sans-serif',
  },
  content: {
    display: 'flex',
    alignItems: 'center',
    gap: '12px',
    flexWrap: 'wrap',
  },
  text: {
    fontSize: '14px',
    color: '#252623',
    lineHeight: '1.4',
  },
  actions: {
    display: 'flex',
    alignItems: 'center',
    gap: '8px',
  },
  link: {
    background: 'none',
    border: 'none',
    color: '#252623',
    fontSize: '14px',
    cursor: 'pointer',
    padding: '0',
    textDecoration: 'underline',
    fontFamily: 'inherit',
  },
  separator: {
    color: '#777A73',
    fontSize: '14px',
  },
  close: {
    position: 'absolute',
    top: '8px',
    right: '8px',
    background: 'none',
    border: 'none',
    fontSize: '18px',
    color: '#777A73',
    cursor: 'pointer',
    lineHeight: '1',
    padding: '4px',
  },
};
