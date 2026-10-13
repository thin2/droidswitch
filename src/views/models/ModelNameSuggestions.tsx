import { useEffect, useState } from "react";
import { request, errorText } from "../../lib/api";
import { useWorkspace } from "../../lib/workspace";
interface Suggestion {
  modelId: string;
  providerName: string;
  before: string;
  after: string;
}
export function ModelNameSuggestions() {
  const { workspace, busy, mutate } = useWorkspace();
  const [result, setResult] = useState<{
    revision: number;
    items: Suggestion[];
  } | null>(null);
  const [error, setError] = useState("");
  const [open, setOpen] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let active = true;
    setResult(null);
    setError("");
    setOpen(false);
    if (workspace) {
      const revision = workspace.revision;
      request<Suggestion[]>("suggest_model_names", { revision }).then(
        (items) => {
          if (active) setResult({ revision, items });
        },
        (e) => {
          if (active) setError(errorText(e));
        },
      );
    }
    return () => {
      active = false;
    };
  }, [workspace, attempt]);
  const suggestions =
    result?.revision === workspace?.revision ? (result?.items ?? []) : [];
  if (error)
    return (
      <p role="alert">
        显示名检查失败：{error}{" "}
        <button disabled={busy} onClick={() => setAttempt((a) => a + 1)}>
          重试命名检查
        </button>
      </p>
    );
  if (!suggestions.length) return null;
  return (
    <>
      <section
        className="name-suggestion-notice"
        aria-label="CLI 模型显示名建议"
      >
        <div>
          <strong>{suggestions.length} 个模型建议补充供应商名称</strong>
          <p>方便在 Droid /model 中区分，API 模型 ID 不变。</p>
        </div>
        <button disabled={busy} onClick={() => setOpen(true)}>
          查看命名建议
        </button>
      </section>
      {open && (
        <div className="modal-backdrop">
          <section
            className="modal name-suggestion-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="name-suggestions-title"
          >
            <h2 id="name-suggestions-title">确认模型显示名称</h2>
            <p>
              这些名称会显示在 Droid 的 /model
              列表中。仅保存草稿，预览并应用后生效。
            </p>
            <div className="remote-models">
              <table>
                <thead>
                  <tr>
                    <th>供应商</th>
                    <th>当前名称</th>
                    <th>建议名称</th>
                  </tr>
                </thead>
                <tbody>
                  {suggestions.map((s) => (
                    <tr key={s.modelId}>
                      <td>{s.providerName}</td>
                      <td>{s.before}</td>
                      <td>{s.after}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <p>
              已有唯一名称保留。供应商同名时会增加编号，也可以在模型编辑器自定义名称。
            </p>
            <footer>
              <button disabled={busy} onClick={() => setOpen(false)}>
                取消
              </button>
              <button
                className="primary"
                disabled={busy}
                onClick={async () => {
                  if (await mutate("apply_model_names")) setOpen(false);
                }}
              >
                采用建议并保存草稿
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
