import { useEffect, useState } from "react";
import { WorkspaceProvider, useWorkspace } from "./lib/workspace";
import { ProviderView } from "./views/providers/ProviderView";
import { ModelLibrary } from "./views/models/ModelLibrary";
import { UsageView } from "./views/usage/UsageView";
import { DialogFocus } from "./components/DialogFocus";
import { Backups } from "./views/Backups";
import { ChangeBar } from "./views/ChangeBar";
import { Settings } from "./views/Settings";
import { WorkspacePage } from "./views/WorkspacePage";
const pages = {
  models: "模型库",
  providers: "供应商",
  usage: "用量统计",
  backups: "配置备份",
  settings: "设置",
} as const;
export type Page = keyof typeof pages;
function currentPage(): Page {
  const v = window.location.hash.replace(/^#\/?/, "").split("?")[0];
  return Object.hasOwn(pages, v) ? (v as Page) : "models";
}
function Layout() {
  const [page, setPage] = useState<Page>(currentPage);
  const { workspace, busy, error, notice, reload } = useWorkspace();
  useEffect(() => {
    const sync = () => setPage(currentPage());
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);
  return (
    <div className="workspace">
      <a
        className="skip-link"
        href="#content"
        onClick={(e) => {
          e.preventDefault();
          document.getElementById("content")?.focus();
        }}
      >
        跳转到内容
      </a>
      <aside className="sidebar">
        <div className="brand">
          <img
            className="brand-mark"
            src={new URL("../assets/app-icon.svg", import.meta.url).href}
            alt=""
            aria-hidden="true"
          />
          <strong>Droid Switch</strong>
        </div>
        <p className="eyebrow">本地模型配置管理</p>
        <nav aria-label="主导航">
          {Object.entries(pages).map(([id, label], i) => (
            <a
              key={id}
              href={`#/${id}`}
              aria-current={id === page ? "page" : undefined}
            >
              <span className="nav-icon" aria-hidden="true">
                {["◇", "⌘", "▥", "▣", "⚙"][i]}
              </span>
              {label}
            </a>
          ))}
        </nav>
        <div className="sidebar-note">
          <span className="status-dot" /> 本地工作空间
          <br />
          <span>0.1.0 · 开发版</span>
        </div>
      </aside>
      <main id="content" tabIndex={-1}>
        <header>
          <div>
            <p className="eyebrow">DROID WORKSPACE</p>
            <h1>{pages[page]}</h1>
          </div>
          <span className="badge">{workspace ? "草稿已连接" : "等待连接"}</span>
        </header>
        {error && (
          <div className="error-banner" role="alert">
            {error}
            <button onClick={() => void reload()} disabled={busy}>
              重新载入草稿
            </button>
          </div>
        )}
        {notice && (
          <p className="notice" role="status">
            {notice}
          </p>
        )}
        {busy && !workspace && <p role="status">正在读取本地工作空间…</p>}
        {page === "providers" ? (
          <ProviderView />
        ) : page === "models" ? (
          <ModelLibrary />
        ) : page === "usage" ? (
          <UsageView />
        ) : page === "backups" ? (
          <Backups />
        ) : page === "settings" ? (
          <Settings />
        ) : (
          <WorkspacePage page={page} />
        )}
        <ChangeBar />
      </main>
    </div>
  );
}
export function App() {
  return (
    <WorkspaceProvider>
      <DialogFocus />
      <Layout />
    </WorkspaceProvider>
  );
}
