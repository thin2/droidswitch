import { useEffect, useMemo, useState } from "react";
import { request, errorText } from "../../lib/api";
import { useWorkspace } from "../../lib/workspace";
import {
  costs,
  csv,
  download,
  total,
  type UsageData,
  type UsageEvent,
  type Report,
} from "./data";
import { EmptyUsageState } from "./EmptyUsageState";
import { UsageHeader } from "./UsageHeader";
function dateString(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}
export function UsageView() {
  const { workspace, mutate, busy } = useWorkspace();
  const [start, setStart] = useState(() => {
      const d = new Date();
      d.setDate(d.getDate() - 6);
      return dateString(d);
    }),
    [end, setEnd] = useState(() => dateString(new Date())),
    [source, setSource] = useState(""),
    [provider, setProvider] = useState(""),
    [model, setModel] = useState(""),
    [data, setData] = useState<UsageData>({
      events: [],
      report: null,
      truncated: false,
    }),
    [loading, setLoading] = useState(false),
    [error, setError] = useState(""),
    [tab, setTab] = useState("models"),
    [settings, setSettings] = useState(false),
    [price, setPrice] = useState(false),
    [cleanup, setCleanup] = useState(false);
  async function load() {
    setLoading(true);
    setError("");
    try {
      const from = new Date(`${start}T00:00:00`);
      const to = new Date(`${end}T00:00:00`);
      to.setDate(to.getDate() + 1);
      if (!Number.isFinite(+from) || !Number.isFinite(+to) || to <= from)
        throw new Error("请选择有效时间范围");
      setData(
        await request<UsageData>("load_usage", {
          from: Math.floor(+from / 1000),
          to: Math.floor(+to / 1000),
        }),
      );
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    void load();
  }, [start, end]);
  const rows = useMemo(
    () =>
      data.events.filter(
        (r) =>
          (!source || r.source === source) &&
          (!provider || r.provider === provider) &&
          (!model || r.model === model),
      ),
    [data, source, provider, model],
  );
  const groups = new Map<string, UsageEvent[]>();
  for (const r of rows) {
    const key = (tab === "providers" ? r.provider : r.model) ?? "未知";
    groups.set(key, [...(groups.get(key) ?? []), r]);
  }
  const daily = new Map<string, number>();
  for (const r of rows) {
    if (r.output !== null) {
      const date = new Date(r.occurredAt * 1000).toLocaleDateString();
      daily.set(date, (daily.get(date) ?? 0) + r.output);
    }
  }
  const max = Math.max(1, ...daily.values());
  function range(days: number) {
    const now = new Date();
    setEnd(dateString(now));
    now.setDate(now.getDate() - days + 1);
    setStart(dateString(now));
  }
  return (
    <>
      {data.report && rows.length > 0 && (
        <UsageHeader
          startDate={start}
          sessionCount={data.report.files}
        />
      )}
      <div className="page-actions">
        <p>
          仅本地统计元数据 · 本机时区{" "}
          {Intl.DateTimeFormat().resolvedOptions().timeZone}
        </p>
        <button onClick={() => setSettings(true)}>采集设置</button>
        <button onClick={() => setPrice(true)}>设置单价</button>
        <button
          disabled={loading}
          onClick={async () => {
            setLoading(true);
            setError("");
            try {
              await request<Report>("scan_usage");
              await load();
            } catch (e) {
              setError(errorText(e));
            } finally {
              setLoading(false);
            }
          }}
        >
          {loading ? "读取中…" : "刷新采集"}
        </button>
        <button
          disabled={!rows.length}
          onClick={() =>
            download(csv(rows), "droid-usage.csv", "text/csv;charset=utf-8")
          }
        >
          导出 CSV
        </button>
      </div>
      <div className="filters usage-filters">
        <button onClick={() => range(1)}>今日</button>
        <button onClick={() => range(7)}>近 7 天</button>
        <button onClick={() => range(30)}>近 30 天</button>
        <input
          aria-label="开始日期"
          type="date"
          value={start}
          onChange={(e) => setStart(e.target.value)}
        />
        <input
          aria-label="结束日期"
          type="date"
          value={end}
          onChange={(e) => setEnd(e.target.value)}
        />
        <select
          aria-label="来源"
          value={source}
          onChange={(e) => setSource(e.target.value)}
        >
          <option value="">全部来源</option>
          <option value="official">官方</option>
          <option value="api">API</option>
          <option value="unknown">未知来源</option>
        </select>
        <select
          aria-label="供应商"
          value={provider}
          onChange={(e) => setProvider(e.target.value)}
        >
          <option value="">全部供应商</option>
          {[
            ...new Set(
              data.events
                .map((r) => r.provider)
                .filter((v): v is string => !!v),
            ),
          ].map((v) => (
            <option key={v}>{v}</option>
          ))}
        </select>
        <select
          aria-label="模型"
          value={model}
          onChange={(e) => setModel(e.target.value)}
        >
          <option value="">全部模型</option>
          {[
            ...new Set(
              data.events.map((r) => r.model).filter((v): v is string => !!v),
            ),
          ].map((v) => (
            <option key={v}>{v}</option>
          ))}
        </select>
      </div>
      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      <p className="coverage">
        覆盖范围：
        {data.report
          ? `${data.report.files} 个文件；最近新增 ${data.report.imported} 条；不支持 ${data.report.unsupported} 行；无效 ${data.report.malformed} 行；不可读 ${data.report.unreadable} 个文件；跳过不明确累计 ${data.report.skippedStreams} 条。更新于 ${new Date(data.report.updatedAt * 1000).toLocaleString()}`
          : "尚未采集。真实 Droid 日志格式需在 Windows 实测确认。"}
        {data.truncated &&
          " 当前范围超过 5 万条，仅展示及导出前 5 万条，请缩小日期范围。"}
      </p>
      {!rows.length ? (
        <EmptyUsageState
          state={
            loading
              ? "scanning"
              : data.report && (data.report.unsupported > 0 || data.report.malformed > 0)
                ? "parse-error"
                : "no-data"
          }
          onCheckPath={() => setSettings(true)}
          onViewCompatibility={() => setSettings(true)}
        />
      ) : (
        <>
          <div className="metric-grid">
            {(
              [
                ["普通输入", "input"],
                ["缓存读取", "cacheRead"],
                ["缓存写入", "cacheWrite"],
                ["输出", "output"],
              ] as const
            ).map(([label, key]) => (
              <section className="panel" key={key}>
                <small>{label} Token</small>
                <h2>{total(rows, key)}</h2>
              </section>
            ))}
          </div>
          <div className="panel">
            <p>
              API 估算费用：{costs(rows.filter((r) => r.source === "api"))}（
              {rows.filter((r) => r.source === "api" && r.cost === null).length}{" "}
              条未定价或数据不全）
            </p>
            <p>
              官方积分：
              {total(
                rows.filter((r) => r.source === "official"),
                "credits",
              )}{" "}
              · 未知来源记录 {rows.filter((r) => r.source === "unknown").length}{" "}
              条
            </p>
          </div>
          <section className="panel">
            <h2>每日已知输出 Token</h2>
            {[...daily].reverse().map(([day, n]) => (
              <div className="trend-row" key={day}>
                <span>{day}</span>
                <div style={{ width: `${(n / max) * 70}%` }} />
                <small>{n.toLocaleString()}</small>
              </div>
            ))}
            <p>
              仅展示已知输出分项；缺失输入或缓存包含关系不明时，不展示总 Token。
            </p>
          </section>
          <section className="panel">
            <div className="source-segments">
              {[
                ["models", "模型统计"],
                ["providers", "供应商统计"],
                ["records", "使用记录"],
              ].map(([key, label]) => (
                <button
                  key={key}
                  aria-pressed={tab === key}
                  onClick={() => setTab(key)}
                >
                  {label}
                </button>
              ))}
            </div>
            <div className="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>{tab === "records" ? "时间 / 模型" : "分组"}</th>
                    <th>记录</th>
                    <th>普通输入</th>
                    <th>缓存读 / 写</th>
                    <th>输出</th>
                    <th>API 估算</th>
                  </tr>
                </thead>
                <tbody>
                  {tab === "records"
                    ? rows.slice(0, 500).map((r) => (
                        <tr key={r.id}>
                          <td>
                            {new Date(r.occurredAt * 1000).toLocaleString()}
                            <br />
                            {r.model ?? "未知模型"}
                          </td>
                          <td>
                            {r.kind} · {r.source}
                          </td>
                          <td>{r.input ?? "未知"}</td>
                          <td>
                            {r.cacheRead ?? "未知"} / {r.cacheWrite ?? "未知"}
                          </td>
                          <td>{r.output ?? "未知"}</td>
                          <td>
                            {r.cost === null
                              ? "未定价"
                              : `${r.currency} ${r.cost}`}
                          </td>
                        </tr>
                      ))
                    : [...groups].map(([name, group]) => (
                        <tr key={name}>
                          <td>{name}</td>
                          <td>{group.length}</td>
                          <td>{total(group, "input")}</td>
                          <td>
                            {total(group, "cacheRead")} /{" "}
                            {total(group, "cacheWrite")}
                          </td>
                          <td>{total(group, "output")}</td>
                          <td>
                            {costs(group.filter((r) => r.source === "api"))}
                          </td>
                        </tr>
                      ))}
                </tbody>
              </table>
            </div>
            {tab === "records" && rows.length > 500 && (
              <p>表格展示前 500 条，CSV 包含当前筛选全部可用记录。</p>
            )}
          </section>
        </>
      )}
      <button onClick={() => setCleanup(true)}>清理过期统计</button>
      {settings && (
        <UsageSettingsDialog
          onClose={() => setSettings(false)}
          busy={busy}
          initial={
            workspace?.usageSettings ?? {
              path: "~/.factory/projects",
              retentionDays: 90,
            }
          }
          save={async (value) => {
            if (await mutate("save_usage_settings", { settings: value }))
              setSettings(false);
          }}
        />
      )}
      {price && (
        <PriceDialog
          onClose={() => setPrice(false)}
          models={[
            ...new Set(
              data.events.map((r) => r.model).filter((v): v is string => !!v),
            ),
          ]}
        />
      )}
      {cleanup && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="清理统计"
          >
            <h2>清理保留期之外的统计？</h2>
            <p>
              只删除本应用超过 {workspace?.usageSettings?.retentionDays ?? 90}{" "}
              天的统计记录，不删除 Droid 原始会话。
            </p>
            <footer>
              <button onClick={() => setCleanup(false)}>取消</button>
              <button
                onClick={async () => {
                  try {
                    await request("clear_usage", {
                      before:
                        Math.floor(Date.now() / 1000) -
                        (workspace?.usageSettings?.retentionDays ?? 90) * 86400,
                      confirmed: true,
                    });
                    setCleanup(false);
                    await load();
                  } catch (e) {
                    setError(errorText(e));
                  }
                }}
              >
                确认清理
              </button>
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
function UsageSettingsDialog({
  onClose,
  initial,
  save,
  busy,
}: {
  onClose: () => void;
  initial: { path: string; retentionDays: number };
  save: (v: { path: string; retentionDays: number }) => Promise<void>;
  busy: boolean;
}) {
  const [value, setValue] = useState(initial);
  return (
    <div className="modal-backdrop">
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="采集设置"
      >
        <h2>本地采集设置</h2>
        <label>
          会话目录或 JSONL 文件
          <input
            value={value.path}
            onChange={(e) => setValue({ ...value, path: e.target.value })}
          />
        </label>
        <label>
          统计保留天数
          <input
            type="number"
            min="1"
            max="3650"
            value={value.retentionDays}
            onChange={(e) =>
              setValue({ ...value, retentionDays: Number(e.target.value) })
            }
          />
        </label>
        <p>
          当前支持 droid-switch-usage-v1 元数据契约，以及含稳定 ID/时间的 SDK
          result 导出。其他真实日志格式会报告不支持，需完成 Windows
          样本适配。只保存计数、归属与时间，不保存对话正文。
        </p>
        <footer>
          <button onClick={onClose}>取消</button>
          <button disabled={busy} onClick={() => void save(value)}>
            保存设置
          </button>
        </footer>
      </section>
    </div>
  );
}
function PriceDialog({
  onClose,
  models,
}: {
  onClose: () => void;
  models: string[];
}) {
  const [model, setModel] = useState(models[0] ?? ""),
    [currency, setCurrency] = useState("USD"),
    [values, setValues] = useState(["", "", "", ""]),
    [effective, setEffective] = useState(() => {
      const d = new Date();
      return `${dateString(d)}T${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
    }),
    [error, setError] = useState("");
  return (
    <div className="modal-backdrop">
      <section
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-label="单价设置"
      >
        <form
          onSubmit={async (e) => {
            e.preventDefault();
            try {
              await request("save_price", {
                price: {
                  model,
                  currency,
                  input: values[0] || null,
                  cacheRead: values[1] || null,
                  cacheWrite: values[2] || null,
                  output: values[3] || null,
                  effectiveAt: Math.floor(+new Date(effective) / 1000),
                },
              });
              onClose();
            } catch (e) {
              setError(errorText(e));
            }
          }}
        >
          <h2>每百万 Token 单价</h2>
          <label>
            历史模型 ID
            <input
              required
              list="usage-models"
              value={model}
              onChange={(e) => setModel(e.target.value)}
            />
            <datalist id="usage-models">
              {models.map((m) => (
                <option key={m} value={m} />
              ))}
            </datalist>
          </label>
          <label>
            币种
            <input
              required
              maxLength={3}
              minLength={3}
              value={currency}
              onChange={(e) => setCurrency(e.target.value.toUpperCase())}
            />
          </label>
          {["普通输入", "缓存读", "缓存写", "输出"].map((label, index) => (
            <label key={label}>
              {label}
              <input
                type="number"
                min="0"
                step="0.000001"
                placeholder="留空：未定价"
                value={values[index]}
                onChange={(e) =>
                  setValues((v) =>
                    v.map((s, i) => (i === index ? e.target.value : s)),
                  )
                }
              />
            </label>
          ))}
          <label>
            生效时间
            <input
              type="datetime-local"
              required
              value={effective}
              onChange={(e) => setEffective(e.target.value)}
            />
          </label>
          <p>
            手动单价仅用于后续导入事件的估算，不改写已存历史金额；不同币种分开统计。
          </p>
          {error && <p role="alert">{error}</p>}
          <footer>
            <button type="button" onClick={onClose}>
              取消
            </button>
            <button className="primary">保存单价</button>
          </footer>
        </form>
      </section>
    </div>
  );
}
