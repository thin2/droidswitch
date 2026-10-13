import { useEffect, useState } from "react";
import { useWorkspace } from "../lib/workspace";
import { request, errorText } from "../lib/api";

interface Diagnosis {
  revision: number;
  bindings: {
    modelId: string;
    status: "matched" | "ambiguous" | "changed_or_missing";
    matches: number[];
    candidates: { index: number; fields: string[] }[];
  }[];
  external: {
    index: number;
    existingModelIds: string[];
    selectable: boolean;
  }[];
}
const fieldLabels: Record<string, string> = {
  apiKey: "API Key（已变化）",
  extraHeaders: "附加请求头（已变化）",
  extraArgs: "额外参数（已变化）",
  noImageSupport: "图片支持",
  displayName: "显示名称",
  baseUrl: "接口地址",
  provider: "API 协议",
  model: "API 模型 ID",
  maxOutputTokens: "最大输出 Token",
};
export function ExternalModels() {
  const { workspace, busy, mutate } = useWorkspace();
  const [report, setReport] = useState<Diagnosis | null>(null);
  const [error, setError] = useState("");
  const [confirm, setConfirm] = useState<number | null>(null);
  const [existing, setExisting] = useState("");
  const [allow, setAllow] = useState(false);
  const [release, setRelease] = useState<string | null>(null);
  const [resolve, setResolve] = useState<string | null>(null);
  const [target, setTarget] = useState("");
  const [resolution, setResolution] = useState<"file" | "app">("file");
  useEffect(() => {
    let active = true;
    setReport(null);
    setError("");
    setConfirm(null);
    setRelease(null);
    setResolve(null);
    if (workspace?.baseline) {
      request<Diagnosis>("inspect_configuration", {
        revision: workspace.revision,
      }).then(
        (value) => {
          if (active) setReport(value);
        },
        (e) => {
          if (active) setError(errorText(e));
        },
      );
    }
    return () => {
      active = false;
    };
  }, [workspace]);
  const diagnosis = report?.revision === workspace?.revision ? report : null;
  const raw = workspace?.baseline?.document.customModels;
  const items: unknown[] = Array.isArray(raw) ? raw : [];
  function label(index: number) {
    const value = items[index];
    const v =
      value && typeof value === "object"
        ? (value as Record<string, unknown>)
        : {};
    return `条目 ${index + 1} · ${typeof v.displayName === "string" ? v.displayName : typeof v.model === "string" ? v.model : "未识别模型"}`;
  }
  function modelLabel(id: string) {
    const m = workspace?.catalog.models.find((m) => m.id === id);
    const p = workspace?.catalog.providers.find((p) => p.id === m?.providerId);
    return m ? `${m.displayName} · ${p?.name ?? "未知供应商"}` : id;
  }
  const conflicts =
    diagnosis?.bindings.filter((b) => b.status !== "matched") ?? [];
  const selectedExternal = diagnosis?.external.find((v) => v.index === confirm);
  const unbound =
    selectedExternal?.existingModelIds.filter(
      (id) => !workspace?.bindings?.some((b) => b.modelId === id),
    ) ?? [];
  const consent = (
    <label className="checkbox">
      <input
        type="checkbox"
        checked={allow}
        onChange={(e) => setAllow(e.target.checked)}
      />
      若条目含明文密钥，我同意将其保存到私有数据库和备份
    </label>
  );
  return (
    <section className="panel">
      <h2>外部配置模型</h2>
      <p>
        重新载入后查看磁盘变化，草稿会保留。接管和冲突处理先保存到应用，写入
        Droid 仍需预览并确认应用。
      </p>
      <button
        disabled={busy || !workspace}
        onClick={() => void mutate("reload_configuration")}
      >
        {workspace?.baseline ? "重新载入当前配置" : "载入当前配置"}
      </button>
      {error && <p role="alert">{error}</p>}
      {workspace?.baseline && !diagnosis && !error && (
        <p role="status">正在检查接管状态…</p>
      )}
      {diagnosis && !diagnosis.external.length && <p>没有未接管的模型。</p>}
      {diagnosis?.external.map((entry) => (
        <article className="backup-row" key={entry.index}>
          <span>
            {label(entry.index)}
            {!entry.selectable && "：条目无效或重复，请先整理文件"}
          </span>
          <button
            disabled={busy || !entry.selectable}
            onClick={() => {
              setConfirm(entry.index);
              setExisting("");
              setAllow(false);
            }}
          >
            接管
          </button>
        </article>
      ))}
      {conflicts.map((b) => (
        <article className="panel" key={b.modelId}>
          <h3>{modelLabel(b.modelId)}</h3>
          <p>
            {b.status === "ambiguous"
              ? "接管条目存在歧义，请核对磁盘条目。"
              : "原接管条目已变化或移除。"}
          </p>
          {b.candidates.map((c) => (
            <p key={c.index}>
              {label(c.index)}：
              {c.fields.length
                ? `变化字段：${c.fields.map((f) => fieldLabels[f] ?? f).join("、")}`
                : "存在重复或重叠接管"}
            </p>
          ))}
          {!b.candidates.length && (
            <p>
              没有找到同 ID 的条目。可明确选择对应的磁盘条目，或解除旧接管。
            </p>
          )}
          <button
            disabled={busy || !diagnosis?.external.some((e) => e.selectable)}
            onClick={() => {
              setResolve(b.modelId);
              setTarget("");
              setResolution("file");
              setAllow(false);
            }}
          >
            选择恢复方式
          </button>{" "}
          <button disabled={busy} onClick={() => setRelease(b.modelId)}>
            解除旧接管
          </button>
        </article>
      ))}
      {resolve && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="恢复接管关系"
          >
            <h2>恢复接管关系</h2>
            <p>{modelLabel(resolve)}</p>
            <label>
              对应的磁盘条目
              <select
                value={target}
                onChange={(e) => setTarget(e.target.value)}
              >
                <option value="">请选择并核对对应条目</option>
                {diagnosis?.external
                  .filter((e) => e.selectable)
                  .map((e) => (
                    <option key={e.index} value={String(e.index)}>
                      {label(e.index)}
                    </option>
                  ))}
              </select>
            </label>
            <label>
              恢复方式
              <select
                value={resolution}
                onChange={(e) =>
                  setResolution(e.target.value as "file" | "app")
                }
              >
                <option value="file">采用文件配置</option>
                <option value="app">使用应用草稿</option>
              </select>
            </label>
            <p>
              {resolution === "file"
                ? "用所选文件条目更新此模型，覆盖它尚未应用的修改；保留模型身份和默认选择。连接变化时单独保存，不修改其他模型的供应商。"
                : "保留此模型的应用草稿，下一次预览会显示将覆盖的字段，确认应用后才写入文件。"}
            </p>
            <p>
              所选条目将与此模型建立接管关系。丢弃草稿会恢复到本次选定的文件配置。
            </p>
            {consent}
            <footer>
              <button disabled={busy} onClick={() => setResolve(null)}>
                取消
              </button>
              <button
                disabled={busy || target === ""}
                onClick={async () => {
                  if (
                    await mutate("resolve_binding", {
                      id: resolve,
                      index: Number(target),
                      resolution,
                      allowPlaintext: allow,
                    })
                  )
                    setResolve(null);
                }}
              >
                确认恢复
              </button>
            </footer>
          </section>
        </div>
      )}
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
              将删除此模型的管理记录与对应草稿，磁盘内容保持不变。若默认方案仍引用此模型，需先清除默认选择。
            </p>
            <footer>
              <button disabled={busy} onClick={() => setRelease(null)}>
                取消
              </button>
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
            aria-label="确认接管外部模型"
          >
            <h2>确认接管外部模型</h2>
            <p>{label(confirm)}</p>
            {selectedExternal?.existingModelIds.length ? (
              <>
                <label>
                  关联已有模型
                  <select
                    value={existing}
                    onChange={(e) => setExisting(e.target.value)}
                  >
                    <option value="">请选择已有记录</option>
                    <option value="__separate__">明确新建独立连接与模型</option>
                    {unbound.map((id) => (
                      <option key={id} value={id}>
                        {modelLabel(id)}
                      </option>
                    ))}
                  </select>
                </label>
                <p>
                  同接口、同协议和同 API
                  模型已有记录。关联会用文件配置更新该记录，保留模型身份和默认选择。
                </p>
                {!unbound.length && (
                  <p role="alert">
                    已有记录已接管。修复该记录请使用“选择恢复方式”；仅在确需另一个独立连接时选择新建。
                  </p>
                )}
              </>
            ) : (
              <p>
                将创建模型管理记录。完全相同的供应商连接会复用，接管本身不改写
                Droid 文件。
              </p>
            )}
            {consent}
            <footer>
              <button disabled={busy} onClick={() => setConfirm(null)}>
                取消
              </button>
              <button
                disabled={
                  busy ||
                  (!!selectedExternal?.existingModelIds.length && !existing)
                }
                onClick={async () => {
                  if (
                    await mutate("takeover_model", {
                      index: confirm,
                      allowPlaintext: allow,
                      existingModelId:
                        existing === "__separate__" ? null : existing || null,
                      createSeparate: existing === "__separate__",
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
