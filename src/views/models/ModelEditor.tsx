import { useState, type FormEvent } from "react";
import { useWorkspace } from "../../lib/workspace";
import { protocols, type ModelEntry, type Protocol } from "../../lib/types";
export function ModelEditor({
  model,
  onClose,
  inline = false,
  formId,
  hideActions = false,
}: {
  model: ModelEntry;
  onClose: () => void;
  inline?: boolean;
  formId?: string;
  hideActions?: boolean;
}) {
  const { workspace, busy, mutate } = useWorkspace();
  const [draft, setDraft] = useState(model),
    [extra, setExtra] = useState(JSON.stringify(model.extraArgs, null, 2)),
    [error, setError] = useState("");
  function field<K extends keyof ModelEntry>(key: K, value: ModelEntry[K]) {
    setDraft((d) => ({ ...d, [key]: value }));
  }
  async function submit(e: FormEvent) {
    e.preventDefault();
    setError("");
    try {
      const args: unknown = JSON.parse(extra);
      if (!args || Array.isArray(args) || typeof args !== "object")
        throw new Error("extraArgs 必须是 JSON 对象");
      if (await mutate("save_model", { model: { ...draft, extraArgs: args } }))
        onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : "参数无效");
    }
  }
  return (
    <div className={inline ? "inline-editor" : "modal-backdrop"}>
      <section
        role={inline ? undefined : "dialog"}
        aria-modal={inline ? undefined : true}
        aria-labelledby={inline ? undefined : "model-title"}
        className={inline ? "" : "modal"}
      >
        <form id={formId} onSubmit={submit}>
          {!hideActions && (
            <h2 id={inline ? undefined : "model-title"}>模型配置</h2>
          )}
          <label>
            供应商
            <select
              required
              value={draft.providerId}
              onChange={(e) => field("providerId", e.target.value)}
            >
              {workspace?.catalog.providers.map((p) => (
                <option value={p.id} key={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            API 模型 ID
            <input
              required
              value={draft.apiModelId}
              onChange={(e) => field("apiModelId", e.target.value)}
            />
          </label>
          <label>
            显示名称
            <input
              required
              value={draft.displayName}
              onChange={(e) => field("displayName", e.target.value)}
            />
          </label>
          <label>
            模型厂商
            <input
              value={draft.manufacturer ?? ""}
              onChange={(e) => field("manufacturer", e.target.value || null)}
              placeholder="可选，与供应商连接独立"
            />
          </label>
          <label>
            API 协议
            <select
              value={draft.protocolOverride ?? ""}
              onChange={(e) =>
                field(
                  "protocolOverride",
                  (e.target.value || null) as Protocol | null,
                )
              }
            >
              <option value="">沿用供应商协议</option>
              {Object.entries(protocols).map(([k, v]) => (
                <option key={k} value={k}>
                  {v}
                </option>
              ))}
            </select>
          </label>
          <label>
            最大输出 Token
            <input
              type="number"
              min="1"
              max="4294967295"
              step="1"
              placeholder="留空：不覆盖"
              value={draft.maxOutputTokens ?? ""}
              onChange={(e) =>
                field(
                  "maxOutputTokens",
                  e.target.value === "" ? null : Number(e.target.value),
                )
              }
            />
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={draft.imageInput}
              onChange={(e) => field("imageInput", e.target.checked)}
            />
            支持图片输入
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={draft.enabled}
              onChange={(e) => field("enabled", e.target.checked)}
            />
            启用此模型
          </label>
          <label>
            额外参数（JSON）
            <textarea
              rows={4}
              value={extra}
              onChange={(e) => setExtra(e.target.value)}
              spellCheck={false}
            />
          </label>
          {error && <p role="alert">{error}</p>}
          <p className="muted">保存为本地草稿，应用前不会更改 Droid 配置。</p>
          {!hideActions && (
            <footer>
              <button type="button" onClick={onClose}>
                取消
              </button>
              <button className="primary" disabled={busy}>
                保存草稿
              </button>
            </footer>
          )}
        </form>
      </section>
    </div>
  );
}
