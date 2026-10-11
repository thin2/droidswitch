import { useState } from "react";
import { request, errorText } from "../../lib/api";
import { useWorkspace } from "../../lib/workspace";
import { protocols, type Protocol } from "../../lib/types";
interface Candidate {
  key: string;
  hash: string;
  app: string;
  name: string;
  baseUrl: string | null;
  protocol: Protocol | null;
  models: string[];
  hasCredential: boolean;
  issue: string | null;
  duplicate: "new" | "same" | "changed";
}
interface Choice {
  key: string;
  hash: string;
  action: string;
  credentialMode: "environment" | "direct";
  environment: string;
  baseUrl: string;
  protocol: Protocol | null;
}
export function ImportWizard({ onClose }: { onClose: () => void }) {
  const { busy, mutate, setNotice } = useWorkspace();
  const [path, setPath] = useState(""),
    [loadedPath, setLoadedPath] = useState(""),
    [items, setItems] = useState<Candidate[] | null>(null),
    [choices, setChoices] = useState<Record<string, Choice>>({}),
    [app, setApp] = useState(""),
    [error, setError] = useState(""),
    [loading, setLoading] = useState(false);
  const visible = items?.filter((i) => !app || i.app === app) ?? [];
  const chosen = Object.values(choices).filter((c) => c.action !== "skip");
  const needsDirect = chosen.some((c) => c.credentialMode === "direct");
  function change(key: string, patch: Partial<Choice>) {
    setChoices((v) => ({ ...v, [key]: { ...v[key], ...patch } }));
  }
  async function scan() {
    setLoading(true);
    setError("");
    try {
      const result = await request<Candidate[]>("preview_ccswitch", { path });
      setItems(result);
      setLoadedPath(path);
      setChoices(
        Object.fromEntries(
          result.map((i) => [
            i.key,
            {
              key: i.key,
              hash: i.hash,
              action: "skip",
              credentialMode: i.hasCredential ? "direct" : "environment",
              environment: "",
              baseUrl: "",
              protocol: null,
            },
          ]),
        ),
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
        className="modal wide-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="import-title"
      >
        <h2 id="import-title">从 CC Switch 导入</h2>
        <p>
          默认同时导入 CC Switch 中已有的 API
          Key，无需重复填写。源数据库保持不变，不导入历史用量。
        </p>
        <label>
          数据库或数据目录
          <input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="默认 ~/.cc-switch/cc-switch.db"
          />
        </label>
        <button disabled={loading || busy} onClick={() => void scan()}>
          {loading ? "读取中…" : "读取供应商"}
        </button>
        {error && (
          <p role="alert">
            {error} 可关闭 CC Switch 后重试，或选择完整数据库备份。
          </p>
        )}
        {items && (
          <>
            <label>
              来源应用
              <select value={app} onChange={(e) => setApp(e.target.value)}>
                <option value="">全部来源</option>
                <option value="claude">Claude Code</option>
                <option value="codex">Codex</option>
              </select>
            </label>
            {!visible.length && <p>此来源没有供应商记录。</p>}
            <div className="import-list">
              {visible.map((i) => {
                const c = choices[i.key];
                const supported = ["claude", "codex"].includes(i.app);
                return (
                  <article className="import-item" key={i.key}>
                    <h3>
                      {i.name}{" "}
                      <small>
                        {i.app} ·{" "}
                        {i.duplicate === "same"
                          ? "源配置未变化，可更新密钥"
                          : i.duplicate === "changed"
                            ? "源配置有变化"
                            : "新记录"}
                      </small>
                    </h3>
                    <p>
                      <code>{i.baseUrl ?? "未提供 API 地址"}</code> ·{" "}
                      {i.protocol ? protocols[i.protocol] : "协议待补充"}
                    </p>
                    <p>
                      {i.models.length
                        ? `${i.models.length} 个模型：${i.models.join("、")}`
                        : "没有模型：先导入供应商，再获取列表或手动添加。"}
                    </p>
                    {i.issue && <p>{i.issue}</p>}
                    <label>
                      处理方式
                      <select
                        disabled={!supported}
                        value={c.action}
                        onChange={(e) =>
                          change(i.key, { action: e.target.value })
                        }
                      >
                        <option value="skip">跳过</option>
                        {i.duplicate === "new" ? (
                          <option value="add">添加到草稿</option>
                        ) : (
                          <>
                            <option value="update">更新对应记录</option>
                            <option value="copy">另存为新连接</option>
                          </>
                        )}
                      </select>
                    </label>
                    {c.action !== "skip" && (
                      <div className="import-fields">
                        <label>
                          凭据保存
                          <select
                            value={c.credentialMode}
                            onChange={(e) =>
                              change(i.key, {
                                credentialMode: e.target
                                  .value as Choice["credentialMode"],
                              })
                            }
                          >
                            <option value="direct" disabled={!i.hasCredential}>
                              同时导入 API Key（默认）
                            </option>
                            <option value="environment">
                              仅引用环境变量（自行配置）
                            </option>
                          </select>
                        </label>
                        <p className="muted">
                          {c.credentialMode === "direct"
                            ? "已检测到 API Key，确认后将一并导入，不显示密钥内容。"
                            : i.hasCredential
                              ? "此方式不复制 API Key，需要自行配置已有环境变量。"
                              : "源配置没有 API Key。请填写已配置的环境变量，或导入后在供应商中填写密钥。"}
                        </p>
                        {c.credentialMode === "environment" && (
                          <label>
                            环境变量名
                            <input
                              value={c.environment}
                              onChange={(e) =>
                                change(i.key, { environment: e.target.value })
                              }
                            />
                            <small>
                              请自行在启动 Droid
                              的终端设置此变量，导入不会修改系统环境。
                            </small>
                          </label>
                        )}
                        <label>
                          兼容接口覆盖（可选）
                          <input
                            value={c.baseUrl}
                            onChange={(e) =>
                              change(i.key, { baseUrl: e.target.value })
                            }
                            placeholder="保持原地址；缺失时必须补充"
                          />
                        </label>
                        <label>
                          协议覆盖
                          <select
                            value={c.protocol ?? ""}
                            onChange={(e) =>
                              change(i.key, {
                                protocol: (e.target.value ||
                                  null) as Protocol | null,
                              })
                            }
                          >
                            <option value="">沿用映射协议</option>
                            {Object.entries(protocols).map(([k, v]) => (
                              <option key={k} value={k}>
                                {v}
                              </option>
                            ))}
                          </select>
                        </label>
                      </div>
                    )}
                  </article>
                );
              })}
            </div>
          </>
        )}
        {needsDirect && (
          <p className="hint">
            确认导入将把所选 API Key
            保存到本机私有数据库；应用配置时也会写入配置文件及相关备份。
          </p>
        )}
        <footer>
          <button disabled={busy} onClick={onClose}>
            取消
          </button>
          <button
            className="primary"
            disabled={
              busy ||
              loading ||
              !chosen.length ||
              chosen.some(
                (c) =>
                  c.credentialMode === "environment" && !c.environment.trim(),
              ) ||
              path !== loadedPath
            }
            onClick={async () => {
              if (
                await mutate("import_ccswitch", {
                  path: loadedPath,
                  choices: Object.values(choices),
                  allowPlaintext: needsDirect,
                })
              ) {
                setNotice(
                  `已导入 ${chosen.length} 项连接，其中 ${chosen.filter((c) => c.credentialMode === "direct").length} 项已带入 API Key、${chosen.filter((c) => c.credentialMode === "environment").length} 项使用环境变量。仅保存草稿；无模型的连接请获取列表或手动添加。`,
                );
                onClose();
              }
            }}
          >
            确认导入{needsDirect ? "（含密钥）" : "草稿"} · {chosen.length} 项
          </button>
        </footer>
      </section>
    </div>
  );
}
