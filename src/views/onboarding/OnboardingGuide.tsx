import React from 'react';

interface OnboardingGuideProps {
  onStepClick: (step: 'provider' | 'fetch' | 'apply') => void;
  onDismiss: () => void;
}

export const OnboardingGuide: React.FC<OnboardingGuideProps> = ({
  onStepClick,
  onDismiss,
}) => {
  return (
    <div style={styles.container}>
      <div style={styles.card}>
        <button
          onClick={onDismiss}
          style={styles.closeButton}
          aria-label="关闭引导"
        >
          ×
        </button>

        <h2 style={styles.headline}>
          管理 Droid 的 API 供应商与模型配置
        </h2>

        <p style={styles.instruction}>配置第一个模型需要:</p>

        <button
          onClick={() => onStepClick('provider')}
          style={styles.step}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = '#B65B3F';
            e.currentTarget.style.backgroundColor = '#FDFCFB';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = '#E3E2DC';
            e.currentTarget.style.backgroundColor = '#FFFFFF';
          }}
          onFocus={(e) => {
            e.currentTarget.style.outline = '2px solid #B65B3F';
            e.currentTarget.style.outlineOffset = '2px';
          }}
          onBlur={(e) => {
            e.currentTarget.style.outline = 'none';
          }}
        >
          <div style={styles.stepTitle}>添加供应商</div>
          <div style={styles.stepDetail}>
            连接到 API 或中转站
          </div>
        </button>

        <button
          onClick={() => onStepClick('fetch')}
          style={styles.step}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = '#B65B3F';
            e.currentTarget.style.backgroundColor = '#FDFCFB';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = '#E3E2DC';
            e.currentTarget.style.backgroundColor = '#FFFFFF';
          }}
          onFocus={(e) => {
            e.currentTarget.style.outline = '2px solid #B65B3F';
            e.currentTarget.style.outlineOffset = '2px';
          }}
          onBlur={(e) => {
            e.currentTarget.style.outline = 'none';
          }}
        >
          <div style={styles.stepTitle}>获取模型</div>
          <div style={styles.stepDetail}>
            从该 API 获取模型列表
          </div>
        </button>

        <button
          onClick={() => onStepClick('apply')}
          style={styles.step}
          onMouseEnter={(e) => {
            e.currentTarget.style.borderColor = '#B65B3F';
            e.currentTarget.style.backgroundColor = '#FDFCFB';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.borderColor = '#E3E2DC';
            e.currentTarget.style.backgroundColor = '#FFFFFF';
          }}
          onFocus={(e) => {
            e.currentTarget.style.outline = '2px solid #B65B3F';
            e.currentTarget.style.outlineOffset = '2px';
          }}
          onBlur={(e) => {
            e.currentTarget.style.outline = 'none';
          }}
        >
          <div style={styles.stepTitle}>应用到 Droid</div>
          <div style={styles.stepDetail}>写入配置到 ~/.factory</div>
        </button>
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
    position: 'relative',
    width: '100%',
    maxWidth: '560px',
    backgroundColor: '#FFFFFF',
    border: '1px solid #E3E2DC',
    borderRadius: '12px',
    padding: '40px',
    boxSizing: 'border-box',
  },
  closeButton: {
    position: 'absolute',
    top: '16px',
    right: '16px',
    background: 'none',
    border: 'none',
    fontSize: '24px',
    lineHeight: '1',
    color: '#777A73',
    cursor: 'pointer',
    padding: '4px 8px',
    borderRadius: '4px',
    transition: 'color 0.15s ease',
  },
  headline: {
    margin: '0 0 16px 0',
    fontSize: '20px',
    fontWeight: '500',
    lineHeight: '1.4',
    color: '#252623',
    fontFamily:
      'Inter, -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  },
  instruction: {
    margin: '0 0 24px 0',
    fontSize: '14px',
    lineHeight: '1.5',
    color: '#777A73',
    fontFamily:
      'Inter, -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  },
  step: {
    display: 'block',
    width: '100%',
    textAlign: 'left',
    backgroundColor: '#FFFFFF',
    border: '1px solid #E3E2DC',
    borderRadius: '8px',
    padding: '16px',
    marginBottom: '12px',
    cursor: 'pointer',
    transition: 'border-color 0.15s ease, background-color 0.15s ease',
    fontFamily:
      'Inter, -apple-system, BlinkMacSystemFont, "PingFang SC", "Microsoft YaHei", system-ui, sans-serif',
  },
  stepTitle: {
    fontSize: '14px',
    fontWeight: '500',
    color: '#252623',
    marginBottom: '4px',
    lineHeight: '1.4',
  },
  stepDetail: {
    fontSize: '14px',
    color: '#777A73',
    lineHeight: '1.4',
  },
};
