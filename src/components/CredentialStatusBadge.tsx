import React, { useState, useEffect } from 'react';

type CredentialStatus = 'readable' | 'unreadable' | 'unchecked';

interface CredentialStatusBadgeProps {
  status: CredentialStatus;
  varName?: string;
  onShowExamples?: () => void;
}

export const CredentialStatusBadge: React.FC<CredentialStatusBadgeProps> = ({
  status,
  varName,
  onShowExamples,
}) => {
  const [showTip, setShowTip] = useState(false);
  const [tipDismissed, setTipDismissed] = useState(false);

  useEffect(() => {
    if (status === 'readable' && varName && !tipDismissed) {
      const dismissed = localStorage.getItem('credential-tip-dismissed');
      if (!dismissed) {
        setShowTip(true);
      }
    }
  }, [status, varName, tipDismissed]);

  const handleDismissTip = () => {
    setShowTip(false);
    setTipDismissed(true);
    localStorage.setItem('credential-tip-dismissed', 'true');
  };

  const config = {
    readable: {
      icon: '✓',
      color: '#2D7A4F',
      bg: '#F0F7F4',
      label: '本软件可读取',
      detail: '仅代表本软件，不代表 Droid',
    },
    unreadable: {
      icon: '⚠',
      color: '#C78A3B',
      bg: '#FFF9F0',
      label: '本软件读不到',
      detail: '建议先在 Droid 终端确认',
    },
    unchecked: {
      icon: '?',
      color: '#777A73',
      bg: '#F6F5F1',
      label: '未检测',
      detail: '直写模式',
    },
  }[status];

  return (
    <div style={{ position: 'relative', display: 'inline-block' }}>
      <div style={styles.badge}>
        <span style={{ ...styles.icon, color: config.color }}>{config.icon}</span>
        <div style={styles.text}>
          <div style={styles.label}>{config.label}</div>
          <div style={styles.detail}>{config.detail}</div>
        </div>
      </div>

      {showTip && (
        <div style={styles.tooltip}>
          <button onClick={handleDismissTip} style={styles.tooltipClose}>
            ×
          </button>
          <p style={styles.tooltipText}>
            环境变量引用只在启动 Droid 的终端设置了该变量时有效。
          </p>
          {onShowExamples && (
            <button onClick={onShowExamples} style={styles.tooltipLink}>
              查看设置方法
            </button>
          )}
        </div>
      )}
    </div>
  );
};

const styles: Record<string, React.CSSProperties> = {
  badge: {
    display: 'inline-flex',
    alignItems: 'center',
    gap: '8px',
    padding: '8px 12px',
    backgroundColor: '#FBF9F5',
    border: '1px solid #E3E2DC',
    borderRadius: '4px',
    fontSize: '13px',
    fontFamily: 'Inter, -apple-system, sans-serif',
  },
  icon: {
    fontSize: '14px',
    lineHeight: '1',
    fontWeight: '600',
  },
  text: {
    display: 'flex',
    flexDirection: 'column',
    gap: '2px',
  },
  label: {
    color: '#252623',
    fontWeight: '500',
    lineHeight: '1.3',
  },
  detail: {
    color: '#777A73',
    fontSize: '12px',
    lineHeight: '1.3',
  },
  tooltip: {
    position: 'absolute',
    top: 'calc(100% + 8px)',
    left: '0',
    width: '280px',
    backgroundColor: '#FFFFFF',
    border: '1px solid #E3E2DC',
    borderRadius: '8px',
    padding: '12px',
    boxShadow: '0 4px 12px rgba(0,0,0,0.08)',
    zIndex: 10,
  },
  tooltipClose: {
    position: 'absolute',
    top: '8px',
    right: '8px',
    background: 'none',
    border: 'none',
    fontSize: '16px',
    color: '#777A73',
    cursor: 'pointer',
    lineHeight: '1',
    padding: '2px',
  },
  tooltipText: {
    margin: '0 0 8px 0',
    fontSize: '13px',
    lineHeight: '1.5',
    color: '#252623',
    fontFamily: 'Inter, -apple-system, sans-serif',
  },
  tooltipLink: {
    background: 'none',
    border: 'none',
    color: '#B65B3F',
    fontSize: '13px',
    cursor: 'pointer',
    padding: '0',
    textDecoration: 'underline',
    fontFamily: 'inherit',
  },
};
