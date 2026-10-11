import React from 'react';

interface UsageHeaderProps {
  startDate: string;
  sessionCount: number;
}

export const UsageHeader: React.FC<UsageHeaderProps> = ({
  startDate,
  sessionCount,
}) => {
  return (
    <div style={styles.container}>
      <span style={styles.text}>
        数据范围：{startDate} 至今（已扫描 {sessionCount} 个会话）
      </span>
    </div>
  );
};

const styles: Record<string, React.CSSProperties> = {
  container: {
    padding: '12px 16px',
    backgroundColor: '#FBF9F5',
    border: '1px solid #E3E2DC',
    borderRadius: '8px',
    fontFamily: 'Inter, -apple-system, sans-serif',
  },
  text: {
    fontSize: '13px',
    color: '#777A73',
    lineHeight: '1.4',
  },
};
