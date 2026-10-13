import React from 'react';

interface SwitchConfirmDialogProps {
  targetSource: 'official' | 'api';
  customModelCount: number;
  onConfirm: () => void;
  onCancel: () => void;
}

export const SwitchConfirmDialog: React.FC<SwitchConfirmDialogProps> = ({
  targetSource,
  customModelCount,
  onConfirm,
  onCancel,
}) => {
  const isToOfficial = targetSource === 'official';

  return (
    <div style={styles.overlay} onClick={onCancel}>
      <div style={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h3 style={styles.title}>
          即将切换到{isToOfficial ? 'Factory 订阅' : '自定义 API'}
        </h3>

        <p style={styles.message}>
          {isToOfficial
            ? `您的自定义 API 配置（${customModelCount} 个模型）将被保留，不会被删除。新会话将默认使用官方模型。`
            : `您的Factory 订阅配置将被保留。新会话将默认使用自定义 API 模型。`}
        </p>

        <div style={styles.unchanged}>
          <div style={styles.unchangedTitle}>以下配置不受此次切换影响:</div>
          <ul style={styles.list}>
            <li style={styles.listItem}>
              {isToOfficial
                ? `自定义 API 模型 (${customModelCount} 个)`
                : 'Factory 订阅模型选择'}
            </li>
            <li style={styles.listItem}>Factory 登录凭据</li>
            <li style={styles.listItem}>项目级覆盖设置</li>
          </ul>
        </div>

        <div style={styles.actions}>
          <button onClick={onCancel} style={styles.cancelButton}>
            取消
          </button>
          <button onClick={onConfirm} style={styles.confirmButton}>
            确认切换
          </button>
        </div>
      </div>
    </div>
  );
};

const styles: Record<string, React.CSSProperties> = {
  overlay: {
    position: 'fixed',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: 'rgba(0, 0, 0, 0.4)',
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    zIndex: 1000,
  },
  dialog: {
    width: '90%',
    maxWidth: '480px',
    backgroundColor: '#FFFFFF',
    borderRadius: '12px',
    padding: '24px',
    boxShadow: '0 8px 24px rgba(0,0,0,0.12)',
    fontFamily: 'Inter, -apple-system, sans-serif',
  },
  title: {
    margin: '0 0 16px 0',
    fontSize: '18px',
    fontWeight: '600',
    color: '#252623',
    lineHeight: '1.4',
  },
  message: {
    margin: '0 0 20px 0',
    fontSize: '14px',
    lineHeight: '1.6',
    color: '#252623',
  },
  unchanged: {
    backgroundColor: '#FBF9F5',
    border: '1px solid #E3E2DC',
    borderRadius: '8px',
    padding: '12px',
    marginBottom: '24px',
  },
  unchangedTitle: {
    fontSize: '13px',
    fontWeight: '500',
    color: '#777A73',
    marginBottom: '8px',
    lineHeight: '1.4',
  },
  list: {
    margin: '0',
    padding: '0 0 0 20px',
    listStyle: 'disc',
  },
  listItem: {
    fontSize: '14px',
    color: '#252623',
    lineHeight: '1.6',
    marginBottom: '4px',
  },
  actions: {
    display: 'flex',
    gap: '12px',
    justifyContent: 'flex-end',
  },
  cancelButton: {
    padding: '10px 20px',
    fontSize: '14px',
    fontWeight: '500',
    color: '#252623',
    backgroundColor: '#FFFFFF',
    border: '1px solid #E3E2DC',
    borderRadius: '8px',
    cursor: 'pointer',
    fontFamily: 'inherit',
  },
  confirmButton: {
    padding: '10px 20px',
    fontSize: '14px',
    fontWeight: '500',
    color: '#FFFFFF',
    backgroundColor: '#B65B3F',
    border: 'none',
    borderRadius: '8px',
    cursor: 'pointer',
    fontFamily: 'inherit',
  },
};
