import { useState } from "react";
import { useWorkspace } from "../../lib/workspace";

export function BatchModels({
  providerId,
  onClose,
}: {
  providerId: string;
  onClose: () => void;
}) {
  const { workspace, mutate, busy } = useWorkspace();
  const [input, setInput] = useState("");
  const [failed, setFailed] = useState(false);
  const provider = workspace?.catalog.providers.find(
    (p) => p.id === providerId,
  );
  const lines = input
    .split(/\r?\n/)
    .map((id) => id.trim())
    .filter(Boolean);
  const unique = [...new Set(lines)];
  const existing = new Set(
    workspace?.catalog.models
      .filter((m) => m.providerId === providerId)
      .map((m) => m.apiModelId),
  );
  const ids = unique.filter((id) => !existing.has(id));
  const tooMany = unique.length > 10000;
  return (
    <div className="modal-backdrop">
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="batch-title"
      >
        <h2 id="batch-title">
          手动添加模型 · {provider?.name ?? "供应商已移除"}
        </h2>
        <label>
          模型 ID（每行一个）
          <textarea
            rows={6}
            value={input}
            spellCheck={false}
            placeholder={"claude-opus-4-6\ngpt-5"}
            onChange={(e) => {
              setInput(e.target.value);
              setFailed(false);
            }}
          />
        </label>
        <p className="muted">
          填写供应商支持的准确
          ID，使用此供应商的协议和凭据。添加后可在模型详情调整参数。
        </p>
        <p role="status">
          待添加 {ids.length} 项 · 已存在 {unique.length - ids.length} 项 ·
          重复输入 {lines.length - unique.length} 项
        </p>
        {tooMany && (
          <p role="alert">单次最多添加 10000 个不同模型 ID，请分批添加。</p>
        )}
        <div className="remote-models batch-preview">
          {unique.slice(0, 100).map((id) => (
            <div key={id}>
              <code>{id}</code>
              <span className="badge">
                {existing.has(id) ? "已存在，跳过" : "新增"}
              </span>
            </div>
          ))}
        </div>
        {unique.length > 100 && (
          <p className="muted">仅预览前 100 项，保存包含全部待添加项。</p>
        )}
        {failed && (
          <p role="alert">保存失败，输入已保留。请检查页面错误提示后重试。</p>
        )}
        <footer>
          <span className="muted">只保存草稿，预览并应用后生效。</span>
          <button disabled={busy} onClick={onClose}>
            取消
          </button>
          <button
            className="primary"
            disabled={busy || !provider || !ids.length || tooMany}
            onClick={async () => {
              if (await mutate("import_models", { providerId, ids })) onClose();
              else setFailed(true);
            }}
          >
            添加 {ids.length} 个模型
          </button>
        </footer>
      </section>
    </div>
  );
}
