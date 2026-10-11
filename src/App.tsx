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
import { DraftReminder } from "./components/DraftReminder";
import { NavigationBadge } from "./components/NavigationBadge";
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
  const [draftDismissed, setDraftDismissed] = useState(() => {
    try {
      return localStorage.getItem("draftReminderDismissed") === "true";
    } catch {
      return false;
    }
  });

  useEffect(() => {
    const sync = () => setPage(currentPage());
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);

  // Calculate pending changes - check if catalog or scheme differs from applied versions
  const hasChanges =
    workspace &&
    (JSON.stringify(workspace.catalog) !==
      JSON.stringify(workspace.appliedCatalog ?? { providers: [], models: [] }) ||
      JSON.stringify(workspace.scheme) !== JSON.stringify(workspace.appliedScheme));

  // Calculate approximate change count for display
  const draftCount = workspace
    ? (workspace.catalog.providers.length -
        (workspace.appliedCatalog?.providers.length ?? 0)) +
      (workspace.catalog.models.length - (workspace.appliedCatalog?.models.length ?? 0))
    : 0;
  const displayCount = Math.abs(draftCount) || (hasChanges ? 1 : 0);
  const showDraftReminder = hasChanges && !draftDismissed;

  const handleDismissDraft = () => {
    setDraftDismissed(true);
    try {
      localStorage.setItem("draftReminderDismissed", "true");
    } catch {}
  };

  // Reset dismissed state when changes are applied or discarded
  useEffect(() => {
    if (!hasChanges) {
      setDraftDismissed(false);
      try {
        localStorage.removeItem("draftReminderDismissed");
      } catch {}
    }
  }, [hasChanges]);
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
          {Object.entries(pages).map(([id, label], i) => {
            // Calculate badge count for each page
            let badgeCount = 0;
            if (id === "providers" && workspace) {
              // Count providers needing attention (could be expanded)
              badgeCount = 0;
            } else if (id === "models" && workspace) {
              // Count disabled models or models without tests
              badgeCount = 0;
            }

            return (
              <a
                key={id}
                href={`#/${id}`}
                aria-current={id === page ? "page" : undefined}
                style={{ position: "relative", display: "flex", alignItems: "center", gap: "8px" }}
              >
                <span className="nav-icon" aria-hidden="true">
                  {["◇", "⌘", "▥", "▣", "⚙"][i]}
                </span>
                {label}
                {badgeCount > 0 && <NavigationBadge count={badgeCount} />}
              </a>
            );
          })}
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
        {showDraftReminder && (
          <div style={{ marginBottom: "16px" }}>
            <DraftReminder
              draftCount={displayCount}
              onView={() => {
                // Scroll to ChangeBar or focus it
                document.querySelector(".change-bar")?.scrollIntoView({ behavior: "smooth" });
              }}
              onApply={() => {
                // Trigger apply action - click the preview button in ChangeBar
                const applyBtn = document.querySelector('.change-bar button.primary') as HTMLButtonElement;
                applyBtn?.click();
              }}
              onDiscard={() => {
                // Trigger discard action
                const buttons = Array.from(document.querySelectorAll('.change-bar button'));
                const discardBtn = buttons.find(btn => btn.textContent?.includes('放弃草稿')) as HTMLButtonElement;
                discardBtn?.click();
              }}
              onDismiss={handleDismissDraft}
            />
          </div>
        )}
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
