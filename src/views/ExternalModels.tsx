import { useState } from "react";
import { useWorkspace } from "../lib/workspace";
export function ExternalModels() {
  const { workspace, busy, mutate } = useWorkspace();
  const [confirm, setConfirm] = useState<number | null>(null),
    [allow, setAllow] = useState(false);
  const [release, setRelease] = useState<string | null>(null);
  const raw = workspace?.baseline?.document.customModels;
  const items = Array.isArray(raw) ? raw : [];
  const external = items
    .map((v, index) => ({ v, index }))
    .filter(
      ({ v }) =>
        !workspace?.bindings?.some(
          (b) => JSON.stringify(b.original) === JSON.stringify(v),
        ),
    );
  const conflicts =
    workspace?.bindings?.filter(
      (b) =>
        !items.some((v) => JSON.stringify(v) === JSON.stringify(b.original)),
    ) ?? [];
  return (
    <section className="panel">
      <h2>外部配置模型</h2>
      <p>载入后仅查看外部条目；明确接管后才能编辑。其他条目保持原样。</p>
      {!workspace?.baseline ? (
        <button
          disabled={busy}
          onClick={() => void mutate("reload_configuration")}
        >
          载入当前配置
        </button>
      ) : !external.length ? (
        <p>没有未接管的模型。</p>
      ) : (
        external.map(({ v, index }) => (
          <article className="backup-row" key={index}>
            <span>
              {typeof v?.displayName === "string"
                ? v.displayName
                : typeof v?.model === "string"
                  ? v.model
                  : `未识别条目 ${index + 1}`}
            </span>
            <button
              disabled={busy}
              onClick={() => {
                setConfirm(index);
                setAllow(false);
              }}
            >
              接管
            </button>
          </article>
        ))
      )}
      {conflicts.map((b) => (
        <article className="backup-row" key={b.modelId}>
          <span>
            {workspace?.catalog.models.find((m) => m.id === b.modelId)
              ?.displayName ?? b.modelId}
            ：原接管条目已在磁盘变更或移除
          </span>
          <button disabled={busy} onClick={() => setRelease(b.modelId)}>
            处理旧接管关系
          </button>
        </article>
      ))}
      {release && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="解除旧接管"
          >
            <h2>解除旧接管关系？</h2>
            <p>
              将删除此旧模型的管理记录与对应草稿，磁盘内容保持不变。可先在设置页导出脱敏草稿，再接管磁盘上的新条目。若默认方案仍引用此模型，需先清除默认选择。
            </p>
            <footer>
              <button onClick={() => setRelease(null)}>取消</button>
              <button
                disabled={busy}
                onClick={async () => {
                  if (await mutate("release_binding", { id: release }))
                    setRelease(null);
                }}
              >
                确认解除
              </button>
            </footer>
          </section>
        </div>
      )}
      {confirm !== null && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="takeover-title"
          >
            <h2 id="takeover-title">确认接管外部模型</h2>
            <p>
              将复制此条目到管理数据库并建立对应关系。接管本身不改写 Droid
              文件。
            </p>
            <label className="checkbox">
              <input
                type="checkbox"
                checked={allow}
                onChange={(e) => setAllow(e.target.checked)}
              />
              若条目含明文密钥，我同意将其保存到私有数据库和备份
            </label>
            <footer>
              <button disabled={busy} onClick={() => setConfirm(null)}>
                取消
              </button>
              <button
                disabled={busy}
                onClick={async () => {
                  if (
                    await mutate("takeover_model", {
                      index: confirm,
                      allowPlaintext: allow,
                    })
                  )
                    setConfirm(null);
                }}
              >
                确认接管
              </button>
            </footer>
          </section>
        </div>
      )}
    </section>
  );
}
