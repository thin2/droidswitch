import React from 'react';

type EmptyState = 'scanning' | 'no-data' | 'parse-error';

interface EmptyUsageStateProps {
  state: EmptyState;
  droidVersion?: string;
  onCheckPath?: () => void;
  onViewCompatibility?: () => void;
}

export const EmptyUsageState: React.FC<EmptyUsageStateProps> = ({
  state,
  droidVersion,
  onCheckPath,
  onViewCompatibility,
}) => {
  const config = {
    scanning: {
      title: '正在扫描会话日志',
      message: '首次扫描可能需要几秒钟，请稍候...',
      showActions: false,
    },
    'no-data': {
      title: '当前范围暂无可用用量',
      message: '可能原因：Droid 尚未运行新会话，或日志目录未找到。',
      showActions: true,
    },
    'parse-error': {
      title: '日志格式不支持',
      message: droidVersion
        ? `当前 Droid 版本 ${droidVersion} 的日志格式暂不支持。`
        : '当前 Droid 日志格式暂不支持。',
      showActions: true,
    },
  }[state];

  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <div style={styles.icon}>{state === 'scanning' ? '⏳' : '📊'}</div>
        <h3 style={styles.title}>{config.title}</h3>
        <p style={styles.message}>{config.message}</p>

        {config.showActions && (
          <div style={styles.actions}>
            {state === 'no-data' && onCheckPath && (
              <button onClick={onCheckPath} style={styles.button}>
                配置采集目录
              </button>
            )}
            {onViewCompatibility && (
              <button onClick={onViewCompatibility} style={styles.linkButton}>
                查看兼容性说明
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: '400px',
    padding: '32px',
  },
  card: {
    maxWidth: '400px',
    textAlign: 'center',
    fontFamily: 'Inter, -apple-system, sans-serif',
  },
  icon: {
    fontSize: '48px',
    marginBottom: '16px',
    lineHeight: '1',
  },
  title: {
    margin: '0 0 12px 0',
    fontSize: '18px',
    fontWeight: '600',
    color: '#252623',
    lineHeight: '1.4',
  },
  message: {
    margin: '0 0 24px 0',
    fontSize: '14px',
    lineHeight: '1.6',
    color: '#777A73',
  },
  actions: {
    display: 'flex',
    flexDirection: 'column',
    gap: '12px',
    alignItems: 'center',
  },
  button: {
    padding: '10px 24px',
    fontSize: '14px',
    fontWeight: '500',
    color: '#FFFFFF',
    backgroundColor: '#B65B3F',
    border: 'none',
    borderRadius: '8px',
    cursor: 'pointer',
    fontFamily: 'inherit',
  },
  linkButton: {
    background: 'none',
    border: 'none',
    color: '#B65B3F',
    fontSize: '14px',
    cursor: 'pointer',
    textDecoration: 'underline',
    fontFamily: 'inherit',
  },
};
