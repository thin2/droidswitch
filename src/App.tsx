import { useEffect, useState } from 'react';
import { WorkspacePage } from './views/WorkspacePage';

const pages = {
  models: '模型库', providers: '供应商', usage: '用量统计', backups: '配置备份', settings: '设置',
} as const;
export type Page = keyof typeof pages;
function currentPage(): Page {
  const value = window.location.hash.replace(/^#\/?/, '');
  return Object.hasOwn(pages, value) ? value as Page : 'models';
}

export function App() {
  const [page, setPage] = useState<Page>(currentPage);
  useEffect(() => {
    const sync = () => setPage(currentPage());
    window.addEventListener('hashchange', sync);
    return () => window.removeEventListener('hashchange', sync);
  }, []);
  return <div className="workspace">
    <a className="skip-link" href="#content" onClick={(event) => { event.preventDefault(); document.getElementById('content')?.focus(); }}>跳转到内容</a>
    <aside className="sidebar">
      <div className="brand"><span className="brand-mark" aria-hidden="true">D</span><strong>Droid Switch</strong></div>
      <p className="eyebrow">本地模型配置管理</p>
      <nav aria-label="主导航">{Object.entries(pages).map(([id, label]) =>
        <a key={id} href={`#/${id}`} aria-current={id === page ? 'page' : undefined}>{label}</a>
      )}</nav>
      <p className="sidebar-note">开发预览 · 0.1.0<br />配置内核正在接入</p>
    </aside>
    <main id="content" tabIndex={-1}>
      <header><p className="eyebrow">工作空间</p><h1>{pages[page]}</h1></header>
      <WorkspacePage page={page} />
    </main>
  </div>;
}
