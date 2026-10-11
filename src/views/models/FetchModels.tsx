import { useState } from "react";
import { request, errorText } from "../../lib/api";
import { useWorkspace } from "../../lib/workspace";
interface Result {
  ids: string[];
  elapsedMs: number;
  incomplete: boolean;
}
export function FetchModels({ onClose }: { onClose: () => void }) {
  const { workspace, mutate, busy } = useWorkspace();
  const [provider, setProvider] = useState(
      workspace?.catalog.providers[0]?.id ?? "",
    ),
    [result, setResult] = useState<Result | null>(null),
    [selected, setSelected] = useState<string[]>([]),
    [query, setQuery] = useState(""),
    [loading, setLoading] = useState(false),
    [error, setError] = useState("");
  const existing = new Set(
    workspace?.catalog.models
      .filter((m) => m.providerId === provider)
      .map((m) => m.apiModelId),
  );
  const visible =
    result?.ids.filter((id) =>
      id.toLowerCase().includes(query.toLowerCase()),
    ) ?? [];
  async function fetch() {
    setLoading(true);
    setError("");
    setResult(null);
    setSelected([]);
    try {
      setResult(
        await request<Result>("fetch_models", { providerId: provider }),
      );
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }
  return (
    <div className="modal-backdrop">
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="fetch-title"
      >
        <h2 id="fetch-title">获取模型列表</h2>
        <label>
          供应商
          <select
            disabled={loading}
            value={provider}
            onChange={(e) => {
              setProvider(e.target.value);
              setResult(null);
              setSelected([]);
            }}
          >
            {workspace?.catalog.providers.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
        <p>仅查询模型目录，不发送生成请求。已有模型参数保持不变。</p>
        <button disabled={loading || !provider} onClick={() => void fetch()}>
          {loading ? "正在获取…" : "获取列表"}
        </button>
        {error && <p role="alert">{error}</p>}
        {result && (
          <>
            <div className="remote-summary">
              <strong>找到 {result.ids.length} 个模型</strong>
              <span className="muted">
                耗时 {(result.elapsedMs / 1000).toFixed(1)} 秒
              </span>
            </div>
            {result.incomplete && (
              <p className="muted">供应商还有后续分页，本次仅显示当前页。</p>
            )}
            <div className="remote-toolbar">
              <input
                aria-label="筛选远程模型"
                placeholder="搜索模型 ID"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
              <button
                disabled={
                  !visible.some(
                    (id) => !existing.has(id) && !selected.includes(id),
                  )
                }
                onClick={() =>
                  setSelected([
                    ...new Set([
                      ...selected,
                      ...visible.filter((id) => !existing.has(id)),
                    ]),
                  ])
                }
              >
                全选结果
              </button>
              <button
                disabled={!selected.length}
                onClick={() => setSelected([])}
              >
                清空
              </button>
            </div>
            <p className="remote-selection" role="status">
              显示 {visible.length} 项 · 已选 {selected.length} 项
              {selected.some((id) => !visible.includes(id)) &&
                "（含筛选外已选项）"}
            </p>
            <div className="remote-models">
              {visible.map((id) => (
                <label className="checkbox" key={id}>
                  <input
                    type="checkbox"
                    disabled={existing.has(id)}
                    checked={existing.has(id) || selected.includes(id)}
                    onChange={(e) =>
                      setSelected((s) =>
                        e.target.checked
                          ? [...s, id]
                          : s.filter((v) => v !== id),
                      )
                    }
                  />
                  <code>{id}</code>
                  {existing.has(id) && <small>已添加</small>}
                </label>
              ))}
              {!visible.length && <p>没有可显示的模型，可手动添加。</p>}
            </div>
            <p className="muted">已添加的模型保留原有配置，不会重复导入。</p>
          </>
        )}
        <footer>
          <button disabled={loading} onClick={onClose}>
            关闭
          </button>
          <button
            className="primary"
            disabled={busy || loading || !selected.length}
            onClick={async () => {
              if (
                await mutate("import_models", {
                  providerId: provider,
                  ids: selected,
                })
              )
                onClose();
            }}
          >
            添加所选 {selected.length} 项
          </button>
        </footer>
      </section>
    </div>
  );
}
