import { request } from "../../lib/api";
import { useEffect, useState } from "react";
import { useWorkspace } from "../../lib/workspace";
import { ImportWizard } from "../import/ImportWizard";
import { SourcePlans } from "./SourcePlans";
import { TestModel } from "./TestModel";
import { ModelEditor } from "./ModelEditor";
import { FetchModels } from "./FetchModels";
import { protocols, type ModelEntry } from "../../lib/types";
import { OnboardingGuide } from "../../components";
function providerFromHash() {
  return (
    new URLSearchParams(window.location.hash.split("?")[1] ?? "").get(
      "provider",
    ) ?? ""
  );
}
export function ModelLibrary() {
  const { workspace, busy, mutate } = useWorkspace();
  const [importing, setImporting] = useState(false);
  const [browse, setBrowse] = useState<"official" | "api">("api");
  const [editing, setEditing] = useState<ModelEntry | null>(null),
    [testing, setTesting] = useState<ModelEntry | null>(null),
    [fetching, setFetching] = useState(false),
    [deleting, setDeleting] = useState<ModelEntry | null>(null);
  const [query, setQuery] = useState(""),
    [provider, setProvider] = useState(providerFromHash),
    [selected, setSelected] = useState("");
  const [tests, setTests] = useState<
      Record<string, { success: boolean; elapsedMs: number; testedAt: number }>
    >({}),
    [testRefresh, setTestRefresh] = useState(0);
  const [onboardingDismissed, setOnboardingDismissed] = useState(() => {
    try {
      return localStorage.getItem("onboardingDismissed") === "true";
    } catch {
      return false;
    }
  });
  useEffect(() => {
    if (workspace)
      void request<typeof tests>("latest_tests")
        .then(setTests)
        .catch(() => {});
  }, [workspace?.revision, testRefresh]);
  useEffect(() => {
    const sync = () => {
      setProvider(providerFromHash());
      setSelected("");
    };
    window.addEventListener("hashchange", sync);
    return () => window.removeEventListener("hashchange", sync);
  }, []);
  const models =
    workspace?.catalog.models.filter(
      (m) =>
        (!provider || m.providerId === provider) &&
        `${m.displayName} ${m.apiModelId}`
          .toLowerCase()
          .includes(query.toLowerCase()),
    ) ?? [];
  const detail = models.find((m) => m.id === selected);
  const drawerOpen = !!detail && browse === "api";
  useEffect(() => {
    if (!drawerOpen) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [drawerOpen]);

  // Show onboarding guide if no providers and not dismissed
  const showOnboarding =
    workspace &&
    browse === "api" &&
    workspace.catalog.providers.length === 0 &&
    !onboardingDismissed;

  const handleDismissOnboarding = () => {
    setOnboardingDismissed(true);
    try {
      localStorage.setItem("onboardingDismissed", "true");
    } catch {}
  };

  const handleOnboardingStep = (step: "provider" | "fetch" | "apply") => {
    handleDismissOnboarding();
    if (step === "provider") {
      window.location.hash = "#/providers";
    } else if (step === "fetch") {
      setFetching(true);
    } else if (step === "apply") {
      // Scroll to ChangeBar
      document
        .querySelector(".change-bar")
        ?.scrollIntoView({ behavior: "smooth" });
    }
  };
  return (
    <>
      <SourcePlans browse={browse} onBrowse={setBrowse} />
      {browse === "api" && (
        <>
          {showOnboarding ? (
            <OnboardingGuide
              onStepClick={handleOnboardingStep}
              onDismiss={handleDismissOnboarding}
            />
          ) : (
            <>
              <div className="page-actions model-actions">
                <p>
                  {workspace?.catalog.models.length ?? "—"} 个模型 · 管理你的
                  API 配置
                </p>
                <button
                  disabled={!workspace}
                  onClick={() => setImporting(true)}
                >
                  导入配置
                </button>
                <button
                  disabled={!workspace?.catalog.providers.length}
                  onClick={() => setFetching(true)}
                >
                  获取模型
                </button>
                <button
                  className="primary"
                  disabled={!workspace?.catalog.providers.length}
                  onClick={() =>
                    setEditing({
                      id: crypto.randomUUID(),
                      providerId:
                        provider || workspace!.catalog.providers[0].id,
                      apiModelId: "",
                      droidSelectorId: null,
                      displayName: "",
                      manufacturer: null,
                      protocolOverride: null,
                      enabled: true,
                      maxOutputTokens: null,
                      imageInput: false,
                      extraArgs: {},
                    })
                  }
                >
                  添加模型
                </button>
                <a className="button" href="#/providers">
                  管理供应商
                </a>
              </div>
              <div className="filters">
                <input
                  aria-label="搜索模型"
                  placeholder="搜索模型名称或 ID…"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                />
                <select
                  aria-label="筛选供应商"
                  value={provider}
                  onChange={(e) => {
                    window.location.hash = e.target.value
                      ? `#/models?provider=${encodeURIComponent(e.target.value)}`
                      : "#/models";
                  }}
                >
                  <option value="">全部供应商</option>
                  {workspace?.catalog.providers.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="model-layout">
                <section className="model-list">
                  {models.map((m) => (
                    <div className="model-item" key={m.id}>
                      <button
                        className={`model-row ${selected === m.id ? "selected" : ""}`}
                        key={m.id}
                        onClick={() => setSelected(m.id)}
                      >
                        <span className="model-identity">
                          <strong>{m.displayName}</strong>
                          <code>{m.apiModelId}</code>
                        </span>
                        <span className="model-supplier">
                          <span className="badge supplier-badge">
                            {
                              workspace?.catalog.providers.find(
                                (p) => p.id === m.providerId,
                              )?.name
                            }
                          </span>
                          <small>
                            {
                              protocols[
                                m.protocolOverride ??
                                  workspace!.catalog.providers.find(
                                    (p) => p.id === m.providerId,
                                  )!.protocol
                              ]
                            }
                          </small>
                        </span>
                        <span className="model-status">
                          <small>
                            {tests[m.id]
                              ? `${tests[m.id].success ? "测试通过" : "测试失败"} · ${tests[m.id].elapsedMs} ms · ${new Date(tests[m.id].testedAt * 1000).toLocaleString()}`
                              : "尚未测试"}
                          </small>
                          <span className="badge">
                            {m.enabled ? "已启用" : "已停用"}
                          </span>
                        </span>
                      </button>
                      <label className="model-toggle">
                        <input
                          type="checkbox"
                          role="switch"
                          aria-label={`启用 ${m.displayName}`}
                          checked={m.enabled}
                          disabled={busy}
                          onChange={(e) =>
                            void mutate("save_model", {
                              model: { ...m, enabled: e.target.checked },
                            })
                          }
                        />
                        <span>{m.enabled ? "启用" : "停用"}</span>
                      </label>
                    </div>
                  ))}
                  {!models.length && (
                    <div className="empty">
                      <span className="empty-symbol">◇</span>
                      <h2>{query ? "没有匹配的模型" : "模型库准备就绪"}</h2>
                      <p>先添加供应商，再添加或获取模型。</p>
                      <a href="#/providers" className="button">
                        前往供应商
                      </a>
                    </div>
                  )}
                </section>
              </div>
            </>
          )}
        </>
      )}
      {detail && browse === "api" && (
        <div className="model-drawer-backdrop">
          <aside
            className="model-drawer"
            role="dialog"
            aria-modal="true"
            aria-labelledby="detail-title"
            onKeyDown={(event) => {
              if (event.key === "Escape" && !busy && !testing && !deleting) {
                event.stopPropagation();
                setSelected("");
              }
            }}
          >
            <div className="model-drawer-header">
              <div>
                <h2 id="detail-title">模型详情</h2>
                <p>{detail.displayName}</p>
              </div>
              <button
                disabled={busy}
                title="使用已保存的模型配置进行测试"
                onClick={() => setTesting(detail)}
              >
                调用测试
              </button>
              <button
                disabled={busy}
                className="drawer-close"
                aria-label="关闭模型详情"
                onClick={() => setSelected("")}
              >
                ×
              </button>
            </div>
            <div className="model-drawer-body">
              <ModelEditor
                key={`${detail.id}:${workspace?.revision}`}
                model={detail}
                inline
                hideActions
                formId="detail-model-form"
                onClose={() => setSelected("")}
              />
            </div>
            <footer className="model-drawer-footer">
              <button
                disabled={busy}
                className="danger-text"
                onClick={() => setDeleting(detail)}
              >
                删除模型
              </button>
              <div className="drawer-save-actions">
                <button disabled={busy} onClick={() => setSelected("")}>
                  取消
                </button>
                <button
                  disabled={busy}
                  className="primary"
                  type="submit"
                  form="detail-model-form"
                >
                  保存草稿
                </button>
              </div>
            </footer>
          </aside>
        </div>
      )}
      {importing && <ImportWizard onClose={() => setImporting(false)} />}{" "}
      {testing && (
        <TestModel
          model={testing}
          onClose={() => {
            setTesting(null);
            setTestRefresh((n) => n + 1);
          }}
        />
      )}{" "}
      {editing && (
        <ModelEditor model={editing} onClose={() => setEditing(null)} />
      )}{" "}
      {fetching && <FetchModels onClose={() => setFetching(false)} />}{" "}
      {deleting && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="删除模型"
          >
            <h2>删除 {deleting.displayName}？</h2>
            <p>从草稿移除此模型，应用后才会影响 Droid 配置。</p>
            <footer>
              <button onClick={() => setDeleting(null)}>取消</button>
              <button
                disabled={busy}
                onClick={async () => {
                  if (await mutate("delete_model", { id: deleting.id })) {
                    setDeleting(null);
                    setSelected("");
                  }
                }}
              >
                确认删除
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
