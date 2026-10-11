import React, { useState } from 'react';

interface ApplySuccessGuideProps {
  configPath?: string;
  onDismiss: () => void;
}

export const ApplySuccessGuide: React.FC<ApplySuccessGuideProps> = ({
  configPath = '~/.factory/settings.json',
  onDismiss,
}) => {
  const [expanded, setExpanded] = useState(false);

  return (
    <div style={styles.container}>
      <div style={styles.content}>
        <div style={styles.mainLine}>
          <span style={styles.label}>配置已写入</span>
          <code style={styles.path}>{configPath}</code>
        </div>

        <p style={styles.instruction}>
          请关闭所有 Droid 会话并重启，然后在新会话的{' '}
          <code style={styles.inlineCode}>/model</code> 中验证。
        </p>

        <button
          onClick={() => setExpanded(!expanded)}
          style={styles.expandButton}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = '#252623';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = '#777A73';
          }}
        >
          {expanded ? '收起详细步骤 ▲' : '展开查看详细步骤 ▼'}
        </button>

        {expanded && (
          <ol style={styles.steps}>
            <li style={styles.step}>关闭所有正在运行的 Droid 终端</li>
            <li style={styles.step}>重新启动 Droid</li>
            <li style={styles.step}>
              在新会话输入 <code style={styles.inlineCode}>/model</code>{' '}
              验证模型列表
            </li>
          </ol>
        )}
      </div>

      <button
        onClick={onDismiss}
        style={styles.closeButton}
        aria-label="关闭提示"
      >
        ×
      </button>
    </div>
  );
};

const styles: Record<string, React.CSSProperties> = {
  container: {
    position: 'relative',
    backgroundColor: '#FBF9F5',
    border: '1px solid #E3E2DC',
    borderRadius: '8px',
    padding: '16px 48px 16px 16px',
    marginTop: '16px',
    fontFamily:
      'Inter, -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  },
  content: {
    maxWidth: '100%',
  },
  mainLine: {
    display: 'flex',
    alignItems: 'baseline',
    flexWrap: 'wrap',
    gap: '8px',
    marginBottom: '8px',
  },
  label: {
    fontSize: '14px',
    fontWeight: '500',
    color: '#252623',
    lineHeight: '1.4',
  },
  path: {
    fontSize: '14px',
    fontFamily:
      '"SF Mono", Monaco, "Cascadia Code", "Roboto Mono", Consolas, "Courier New", monospace',
    color: '#252623',
    lineHeight: '1.4',
  },
  instruction: {
    margin: '0 0 12px 0',
    fontSize: '14px',
    lineHeight: '1.5',
    color: '#252623',
  },
  inlineCode: {
    fontSize: '14px',
    fontFamily:
      '"SF Mono", Monaco, "Cascadia Code", "Roboto Mono", Consolas, "Courier New", monospace',
    color: '#252623',
    backgroundColor: '#EEECE6',
    padding: '2px 4px',
    borderRadius: '3px',
  },
  expandButton: {
    background: 'none',
    border: 'none',
    padding: '0',
    fontSize: '14px',
    color: '#777A73',
    cursor: 'pointer',
    lineHeight: '1.4',
    transition: 'color 0.15s ease',
    fontFamily: 'inherit',
  },
  steps: {
    margin: '12px 0 0 0',
    padding: '0 0 0 20px',
    listStyle: 'decimal',
    fontSize: '14px',
    lineHeight: '1.6',
    color: '#252623',
  },
  step: {
    marginBottom: '6px',
  },
  closeButton: {
    position: 'absolute',
    top: '12px',
    right: '12px',
    background: 'none',
    border: 'none',
    fontSize: '20px',
    lineHeight: '1',
    color: '#777A73',
    cursor: 'pointer',
    padding: '4px',
    transition: 'color 0.15s ease',
  },
};
