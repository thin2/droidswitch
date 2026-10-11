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
            <p>
              {result.ids.length} 个模型 · {result.elapsedMs} ms
              {result.incomplete
                ? " · 供应商还有后续分页，本次仅显示当前页"
                : ""}
            </p>
            <input
              aria-label="筛选远程模型"
              placeholder="搜索模型 ID"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <button
              onClick={() =>
                setSelected([
                  ...new Set([
                    ...selected,
                    ...visible.filter((id) => !existing.has(id)),
                  ]),
                ])
              }
            >
              选择当前搜索结果
            </button>
            <button onClick={() => setSelected([])}>清空选择</button>
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
            <p>远程列表中消失的模型不会从本地删除。</p>
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
