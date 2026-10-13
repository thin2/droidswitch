import { useState, type FormEvent } from "react";
import { useWorkspace } from "../../lib/workspace";
import { protocols, type Provider, type Protocol } from "../../lib/types";
import { CredentialStatusBadge } from "../../components/CredentialStatusBadge";
import { FetchModels } from "../models/FetchModels";
import { BatchModels } from "../models/BatchModels";
const blank = (): Provider => ({
  id: crypto.randomUUID(),
  name: "",
  baseUrl: "",
  protocol: "openai",
  credential: { mode: "direct", value: "" },
  modelListUrl: null,
  extraHeaders: {},
});
export function ProviderView() {
  const { workspace, busy, mutate } = useWorkspace();
  const [fetching, setFetching] = useState<string | null>(null);
  const [adding, setAdding] = useState<string | null>(null);
  const [edit, setEdit] = useState<Provider | null>(null),
    [headers, setHeaders] = useState("{}"),
    [confirmed, setConfirmed] = useState(false),
    [formError, setFormError] = useState(""),
    [removing, setRemoving] = useState<Provider | null>(null);
  function select(p: Provider) {
    setEdit(structuredClone(p));
    setHeaders(JSON.stringify(p.extraHeaders, null, 2));
    setConfirmed(false);
    setFormError("");
  }
  async function submit(e: FormEvent) {
    e.preventDefault();
    if (!edit) return;
    try {
      const parsed: unknown = JSON.parse(headers);
      if (
        !parsed ||
        Array.isArray(parsed) ||
        typeof parsed !== "object" ||
        Object.values(parsed).some((v) => typeof v !== "string")
      )
        throw new Error("附加请求头必须是字符串对象");
      if (
        await mutate("save_provider", {
          provider: {
            ...edit,
            extraHeaders: parsed,
            modelListUrl: edit.modelListUrl || null,
          },
          allowPlaintext: confirmed,
        })
      )
        setEdit(null);
    } catch (e) {
      setFormError(e instanceof Error ? e.message : "JSON 格式错误");
    }
  }
  const affected =
    workspace?.catalog.models.filter((m) => m.providerId === removing?.id)
      .length ?? 0;
  return (
    <>
      <div className="page-actions">
        <p>连接与协议独立保存，同一服务可添加多个连接。</p>
        <button
          className="primary"
          disabled={!workspace || busy}
          onClick={() => select(blank())}
        >
          ＋ 添加供应商
        </button>
      </div>
      <div className="provider-grid">
        {workspace?.catalog.providers.map((p) => (
          <article className="provider-card" key={p.id}>
            <div className="card-top">
              <span className="provider-avatar">{p.name.slice(0, 1)}</span>
              <span className="badge">{protocols[p.protocol]}</span>
            </div>
            <h2>{p.name}</h2>
            <p className="mono ellipsis">{p.baseUrl}</p>
            <p className="muted">
              {
                workspace.catalog.models.filter((m) => m.providerId === p.id)
                  .length
              }{" "}
              个模型 ·{" "}
              {p.credential.mode === "environment" ? "环境变量" : "兼容模式"}
            </p>
            {p.credential.mode === "environment" && (
              <div style={{ marginTop: "8px" }}>
                <CredentialStatusBadge
                  status="unchecked"
                  varName={p.credential.value}
                />
              </div>
            )}
            <div className="provider-model-actions">
              <button disabled={busy} onClick={() => setFetching(p.id)}>获取模型</button>
              <button disabled={busy} onClick={() => setAdding(p.id)}>手动添加模型</button>
              <a className="button" href={`#/models?provider=${encodeURIComponent(p.id)}`}>查看模型（{workspace.catalog.models.filter(m => m.providerId === p.id).length}）</a>
            </div>
            <div className="row-actions">
              <button onClick={() => select(p)} disabled={busy}>
                编辑
              </button>
              <button
                onClick={() => void mutate("copy_provider", { id: p.id })}
                disabled={busy}
              >
                复制
              </button>
              <button
                className="danger-text"
                onClick={() => setRemoving(p)}
                disabled={busy}
              >
                删除
              </button>
            </div>
          </article>
        ))}
      </div>
      {workspace && !workspace.catalog.providers.length && (
        <div className="empty">
          <span className="empty-symbol">⌘</span>
          <h2>连接你的第一个供应商</h2>
          <p>添加 API 地址和凭据引用，再把模型加入模型库。</p>
          <button onClick={() => select(blank())}>添加供应商</button>
        </div>
      )}
      {fetching && <FetchModels providerId={fetching} onClose={() => setFetching(null)} onManual={() => { setAdding(fetching); setFetching(null); }} />}
      {adding && <BatchModels providerId={adding} onClose={() => setAdding(null)} />}
      {edit && (
        <div className="modal-backdrop">
          <section
            className="dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="provider-title"
          >
            <div className="section-heading">
              <h2 id="provider-title">
                {workspace?.catalog.providers.some((p) => p.id === edit.id)
                  ? "编辑"
                  : "添加"}
                供应商
              </h2>
              <button aria-label="关闭供应商编辑" onClick={() => setEdit(null)}>
                ×
              </button>
            </div>
            <form onSubmit={submit}>
              <label>
                连接名称
                <input
                  required
                  autoFocus
                  value={edit.name}
                  onChange={(e) => setEdit({ ...edit, name: e.target.value })}
                />
              </label>
              <label>
                API 基础地址
                <input
                  required
                  type="url"
                  placeholder={
                    edit.protocol === "google-gemini"
                      ? "https://generativelanguage.googleapis.com/v1beta"
                      : "https://api.example.com/v1"
                  }
                  value={edit.baseUrl}
                  onChange={(e) =>
                    setEdit({ ...edit, baseUrl: e.target.value })
                  }
                />
                <small>按服务商要求填写，地址不会自动补删 /v1。</small>
                {edit.protocol === "google-gemini" && (
                  <div className="hint" style={{ marginTop: "8px", padding: "12px", background: "#f8f9fa", borderRadius: "4px", fontSize: "13px" }}>
                    <strong>Google Gemini API 配置:</strong>
                    <ul style={{ marginTop: "8px", marginBottom: "0", paddingLeft: "20px" }}>
                      <li>默认端点: https://generativelanguage.googleapis.com/v1beta</li>
                      <li>从 <a href="https://aistudio.google.com/apikey" target="_blank" rel="noopener noreferrer">AI Studio</a> 获取 API Key</li>
                      <li>支持模型: gemini-2.0-flash, gemini-1.5-pro 等</li>
                    </ul>
                    <a 
                      href="https://ai.google.dev/gemini-api/docs" 
                      target="_blank" 
                      rel="noopener noreferrer"
                      style={{ fontSize: "12px", marginTop: "8px", display: "inline-block" }}
                    >
                      查看 Gemini API 文档 →
                    </a>
                  </div>
                )}
              </label>
              <label>
                默认 API 协议
                <select
                  value={edit.protocol}
                  onChange={(e) =>
                    setEdit({ ...edit, protocol: e.target.value as Protocol })
                  }
                >
                  {Object.entries(protocols).map(([id, label]) => (
                    <option key={id} value={id}>
                      {label}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                凭据方式
                <select
                  value={edit.credential.mode}
                  onChange={(e) => {
                    setEdit({
                      ...edit,
                      credential: {
                        mode: e.target.value as "environment" | "direct",
                        value: "",
                      },
                    });
                    setConfirmed(false);
                  }}
                >
                  <option value="direct">直接写入 API Key（默认）</option>
                  <option value="environment">环境变量引用</option>
                </select>
              </label>
              <label>
                {edit.credential.mode === "environment"
                  ? "环境变量名"
                  : "API Key"}
                <input
                  required
                  type={edit.credential.mode === "direct" ? "password" : "text"}
                  autoComplete="off"
                  spellCheck={false}
                  placeholder={
                    edit.credential.mode === "environment"
                      ? "PROVIDER_API_KEY"
                      : "API Key"
                  }
                  value={edit.credential.value}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      credential: { ...edit.credential, value: e.target.value },
                    })
                  }
                />
              </label>
              {edit.credential.mode === "environment" ? (
                <>
                  <p className="hint">
                    Droid
                    必须由能读取该变量的终端启动。软件自身能读取变量，不代表其他终端也可以。
                  </p>
                  {edit.protocol === "google-gemini" && (
                    <p className="hint" style={{ fontSize: "12px", marginTop: "4px" }}>
                      推荐变量名: GOOGLE_API_KEY 或 GEMINI_API_KEY
                    </p>
                  )}
                </>
              ) : (
                <label className="check warning">
                  <input
                    type="checkbox"
                    checked={confirmed}
                    onChange={(e) => setConfirmed(e.target.checked)}
                  />
                  我理解本地数据库、Droid 配置和备份可能包含明文密钥。
                </label>
              )}
              <details>
                <summary>高级连接设置</summary>
                <label>
                  模型列表完整 URL（可选）
                  <input
                    type="url"
                    value={edit.modelListUrl ?? ""}
                    onChange={(e) =>
                      setEdit({ ...edit, modelListUrl: e.target.value })
                    }
                  />
                </label>
                <label>
                  附加请求头 JSON
                  <textarea
                    value={headers}
                    onChange={(e) => setHeaders(e.target.value)}
                    spellCheck={false}
                  />
                </label>
              </details>
              {formError && (
                <p role="alert" className="error">
                  {formError}
                </p>
              )}
              <div className="dialog-actions">
                <span className="muted">只保存草稿，不修改 Droid。</span>
                <button type="button" onClick={() => setEdit(null)}>
                  取消
                </button>
                <button
                  type="submit"
                  disabled={
                    busy || (edit.credential.mode === "direct" && !confirmed)
                  }
                >
                  保存草稿
                </button>
              </div>
            </form>
          </section>
        </div>
      )}
      {removing && (
        <div className="modal-backdrop">
          <section
            className="dialog small-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="delete-provider-title"
          >
            <h2 id="delete-provider-title">删除 {removing.name}？</h2>
            <p>
              将从草稿中移除连接及 {affected}{" "}
              个模型。正式配置只会在预览并应用后修改。
            </p>
            <div className="dialog-actions">
              <button onClick={() => setRemoving(null)}>取消</button>
              <button
                className="danger"
                disabled={busy}
                onClick={async () => {
                  if (
                    await mutate("delete_provider", {
                      id: removing.id,
                      confirmModels: affected,
                    })
                  )
                    setRemoving(null);
                }}
              >
                确认移除草稿记录
              </button>
            </div>
          </section>
        </div>
      )}
    </>
  );
}
