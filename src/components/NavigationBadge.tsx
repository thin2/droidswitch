import React from 'react';

interface NavigationBadgeProps {
  count: number;
}

export const NavigationBadge: React.FC<NavigationBadgeProps> = ({ count }) => {
  if (count === 0) return null;

  return (
    <span style={styles.badge}>
      {count > 99 ? '99+' : count}
    </span>
  );
};

const styles: Record<string, React.CSSProperties> = {
  badge: {
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    minWidth: '18px',
    height: '18px',
    padding: '0 5px',
    backgroundColor: '#B65B3F',
    color: '#FFFFFF',
    fontSize: '11px',
    fontWeight: '600',
    borderRadius: '9px',
    fontFamily: 'Inter, -apple-system, sans-serif',
    lineHeight: '1',
  },
};
