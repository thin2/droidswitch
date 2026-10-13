import { useEffect, useState, useRef } from "react";
import { request, errorText } from "../../lib/api";
import {
  download,
  formatCost,
  formatTokens,
  tokenTotal,
  type UsagePayload,
  type Filter,
  type RecordRow,
  type UsageSettings,
} from "./data";
import { useWorkspace } from "../../lib/workspace";
import { EmptyUsageState } from "./EmptyUsageState";

function dateString(d: Date) {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

export function UsageView() {
  const { mutate } = useWorkspace();
  const loadSequence = useRef(0);
  const [source, setSource] = useState("");
  const [granularity, setGranularity] = useState("");
  const [history, setHistory] = useState(false);
  const [start, setStart] = useState(() => {
    const d = new Date();
    d.setDate(d.getDate() - 6);
    return dateString(d);
  });
  const [end, setEnd] = useState(() => dateString(new Date()));
  const [provider, setProvider] = useState("");
  const [model, setModel] = useState("");
  const [data, setData] = useState<UsagePayload | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [tab, setTab] = useState<
    "records" | "providers" | "models" | "pricing"
  >("records");
  const [settings, setSettings] = useState(false);
  const [price, setPrice] = useState(false);
  const [reprice, setReprice] = useState<{
    priceId: string;
    filter: Filter;
    token: string;
    records: number;
    completeRecords: number;
    estimate: string;
    currency: string;
  } | null>(null);
  const [cleanup, setCleanup] = useState(false);
  const [selectedRecord, setSelectedRecord] = useState<RecordRow | null>(null);
  const [limit] = useState(100);
  const [offset, setOffset] = useState(0);

  function currentFilter(): Filter {
    const from = new Date(`${start}T00:00:00`);
    const to = new Date(`${end}T00:00:00`);
    to.setDate(to.getDate() + 1);
    if (!Number.isFinite(+from) || !Number.isFinite(+to) || to <= from)
      throw new Error("请选择有效时间范围");
    return {
      from: Math.floor(+from / 1000),
      to: Math.floor(+to / 1000),
      provider,
      model,
      source,
      granularity,
      includeUnknownTime: history,
      bucket: "day",
      offsetMinutes: -from.getTimezoneOffset(),
    };
  }

  async function load() {
    const sequence = ++loadSequence.current;
    setLoading(true);
    setError("");
    try {
      const filter = currentFilter();
      const payload = await request<UsagePayload>("load_usage", {
        filter,
        limit,
        offset,
      });
      if (sequence === loadSequence.current) setData(payload);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    setOffset(0);
  }, [start, end, provider, model, source, granularity, history]);

  useEffect(() => {
    void load();
  }, [start, end, provider, model, offset, source, granularity, history]);

  useEffect(() => {
    const timer = setInterval(() => {
      void load();
    }, 15000);
    return () => clearInterval(timer);
  }, [start, end, provider, model, offset, source, granularity, history]);

  useEffect(() => {
    if (!selectedRecord) return;
    const previous = document.activeElement as HTMLElement | null;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setSelectedRecord(null);
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      previous?.focus();
    };
  }, [selectedRecord]);

  async function scan() {
    setLoading(true);
    setError("");
    try {
      await request("scan_usage");
      await load();
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }

  function range(days: number) {
    const now = new Date();
    setEnd(dateString(now));
    now.setDate(now.getDate() - days + 1);
    setStart(dateString(now));
  }

  const page = data?.page;
  const hasData = page && page.summary.records > 0;

  return (
    <div className="usage-page">
      <div className="usage-toolbar">
        <small>
          本地用量 ·{" "}
          {data?.report
            ? `更新于 ${new Date(data.report.updatedAt * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`
            : "等待采集"}
        </small>
        <button
          className="primary"
          disabled={loading || data?.scanning}
          onClick={scan}
        >
          {loading || data?.scanning ? "采集中…" : "刷新采集"}
        </button>
        <details className="usage-more">
          <summary>更多操作</summary>
          <div className="usage-secondary-actions">
            <button onClick={() => setSettings(true)}>采集设置</button>
            <button onClick={() => setPrice(true)}>定价管理</button>
            <button onClick={() => setCleanup(true)}>清理统计</button>
            <button
              disabled={!hasData}
              onClick={async () => {
                try {
                  const filter = currentFilter();
                  const exported = await request<{ csv: string; rows: number }>(
                    "export_usage",
                    { filter },
                  );
                  download(
                    exported.csv,
                    "droid-usage.csv",
                    "text/csv;charset=utf-8",
                  );
                } catch (e) {
                  setError(errorText(e));
                }
              }}
            >
              导出 CSV
            </button>
          </div>
        </details>
      </div>

      <div className="usage-filters">
        <div className="usage-date-range">
          <button onClick={() => range(1)}>今日</button>
          <button onClick={() => range(7)}>近 7 天</button>
          <button onClick={() => range(30)}>近 30 天</button>
          <label className="usage-date-label">
            <span>从</span>
            <input
              aria-label="开始日期"
              type="date"
              value={start}
              onChange={(e) => setStart(e.target.value)}
            />
          </label>
          <label className="usage-date-label">
            <span>至</span>
            <input
              aria-label="结束日期"
              type="date"
              value={end}
              onChange={(e) => setEnd(e.target.value)}
            />
          </label>
        </div>
        <div className="usage-select-row">
          <select
            aria-label="供应商"
            value={provider}
            onChange={(e) => {
              setProvider(e.target.value);
              setModel("");
            }}
          >
            <option value="">全部供应商</option>
            <option value="unknown">未知供应商</option>
            {page?.providersFilter
              .filter((p): p is string => p !== null)
              .map((p) => (
                <option key={p} value={p}>
                  {page?.providerLabels?.[p] ?? p}
                </option>
              ))}
          </select>
          <select
            aria-label="模型"
            value={model}
            onChange={(e) => setModel(e.target.value)}
          >
            <option value="">全部模型</option>
            {page?.modelsFilter.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
          <details className="usage-advanced">
            <summary>
              更多筛选{source || granularity || history ? " · 已启用" : ""}
            </summary>
            <div className="usage-advanced-fields">
              <select
                aria-label="来源"
                value={source}
                onChange={(e) => setSource(e.target.value)}
              >
                <option value="">全部来源</option>
                <option value="api">自定义 API</option>
                <option value="official">官方订阅</option>
                <option value="unknown">未知来源</option>
              </select>
              <select
                aria-label="记录类型"
                value={granularity}
                onChange={(e) => setGranularity(e.target.value)}
              >
                <option value="">全部类型</option>
                <option value="request">请求</option>
                <option value="observed_delta">观察差额</option>
                <option value="historical_baseline">历史累计</option>
              </select>
              <label>
                <input
                  type="checkbox"
                  checked={history}
                  onChange={(e) => setHistory(e.target.checked)}
                />
                包含时间未知的历史累计（不进入趋势）
              </label>
              <button
                onClick={() => {
                  setStart("1970-01-01");
                  setEnd(dateString(new Date()));
                  setHistory(true);
                }}
              >
                全部时间
              </button>
            </div>
          </details>
        </div>
      </div>

      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}

      <details className="usage-diagnostics">
        <summary>采集详情</summary>
        <p>
          仅保存统计元数据 · 时区{" "}
          {Intl.DateTimeFormat().resolvedOptions().timeZone}
        </p>
        {data?.notice && <p>{data.notice}</p>}
        {data?.report && (
          <p>
            最近一次采集：新增 {data.report.imported} 条 · 重复{" "}
            {data.report.duplicates} 条 · 不支持 {data.report.unsupported} 行 ·
            格式错误 {data.report.malformed} 行 · 不可读{" "}
            {data.report.unreadable} 个文件 · 计数重置{" "}
            {data.report.counterResets} 条 · 供应商待确认{" "}
            {data.report.logUnpaired} 条
          </p>
        )}
      </details>

      {!hasData ? (
        <EmptyUsageState
          state={
            loading || data?.scanning
              ? "scanning"
              : data?.report &&
                  (data.report.unsupported > 0 || data.report.malformed > 0)
                ? "parse-error"
                : "no-data"
          }
          onCheckPath={() => setSettings(true)}
          onViewCompatibility={() => setSettings(true)}
        />
      ) : page ? (
        <>
          <div className="metric-grid">
            <section className="panel">
              <small>缓存命中率</small>
              <h2>
                {page.summary.tokens.unknownInput +
                  page.summary.tokens.unknownCacheRead +
                  page.summary.tokens.unknownCacheWrite >
                  0 ||
                page.summary.tokens.input +
                  page.summary.tokens.cacheRead +
                  page.summary.tokens.cacheWrite ===
                  0
                  ? "未知"
                  : `${((100 * page.summary.tokens.cacheRead) / (page.summary.tokens.input + page.summary.tokens.cacheRead + page.summary.tokens.cacheWrite)).toFixed(1)}%`}
              </h2>
            </section>
            <section className="panel">
              <small>会话数</small>
              <h2>{page.summary.sessions.toLocaleString()}</h2>
            </section>
            <section className="panel">
              <small>已知 Token</small>
              <h2>
                {formatTokens(
                  tokenTotal(page.summary.tokens),
                  page.summary.tokens.unknownInput +
                    page.summary.tokens.unknownCacheRead +
                    page.summary.tokens.unknownCacheWrite +
                    page.summary.tokens.unknownOutput +
                    page.summary.tokens.unknownThinking,
                )}
              </h2>
            </section>
            <section className="panel">
              <small>估算费用 · 参考价</small>
              <h2 className="usage-cost-total">
                {page.summary.estimates.length
                  ? page.summary.estimates.map((e) => (
                      <span key={e.currency}>{formatCost([e])}</span>
                    ))
                  : "未定价"}
              </h2>
              <small>
                完整定价 {page.summary.pricedRecords} 条 · 部分估算{" "}
                {page.summary.partialRecords} 条 · 未定价{" "}
                {page.summary.billableRecords} 条
              </small>
            </section>
          </div>

          <details className="panel usage-token-details">
            <summary>Token 分项</summary>
            <table>
              <thead>
                <tr>
                  <th>类型</th>
                  <th>数量</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <td>普通输入</td>
                  <td>
                    {formatTokens(
                      page.summary.tokens.input,
                      page.summary.tokens.unknownInput,
                    )}
                  </td>
                </tr>
                <tr>
                  <td>缓存读取</td>
                  <td>
                    {formatTokens(
                      page.summary.tokens.cacheRead,
                      page.summary.tokens.unknownCacheRead,
                    )}
                  </td>
                </tr>
                <tr>
                  <td>缓存写入</td>
                  <td>
                    {formatTokens(
                      page.summary.tokens.cacheWrite,
                      page.summary.tokens.unknownCacheWrite,
                    )}
                  </td>
                </tr>
                <tr>
                  <td>输出</td>
                  <td>
                    {formatTokens(
                      page.summary.tokens.output,
                      page.summary.tokens.unknownOutput,
                    )}
                  </td>
                </tr>
                <tr>
                  <td>思考</td>
                  <td>
                    {formatTokens(
                      page.summary.tokens.thinking,
                      page.summary.tokens.unknownThinking,
                    )}
                  </td>
                </tr>
              </tbody>
            </table>
          </details>
          <details className="usage-diagnostics">
            <summary>统计口径与价格说明</summary>
            <p>
              未设置自定义单价时，自动采用 2026-10-13
              核对的官方参考价估算，历史记录也按此参考价计算，不代表渠道实际账单。Claude
              按标准模式、5 分钟缓存写入，Haiku 5.5 按不超过 100K 的短上下文档；OpenAI 按标准短上下文（不含长上下文加价）；DeepSeek
              按高峰价估算（空闲价为一半）。思考按输出价计费，不含工具附加费、税费和渠道折扣。单价可在“定价”中覆盖。
            </p>
            <p>
              <a
                href="https://platform.claude.com/docs/en/about-claude/pricing"
                target="_blank"
                rel="noreferrer"
              >
                Claude 官方价格
              </a>{" "}
              ·{" "}
              <a
                href="https://api-docs.deepseek.com/zh-cn/quick_start/pricing/"
                target="_blank"
                rel="noreferrer"
              >
                DeepSeek 官方价格
              </a>{" "}
              ·{" "}
              <a
                href="https://developers.openai.com/api/docs/pricing"
                target="_blank"
                rel="noreferrer"
              >
                OpenAI 官方价格
              </a>
            </p>
            <p className="coverage">
              未分配到趋势的 Token：
              {tokenTotal(page.unassignedTokens).toLocaleString()} · 官方积分：
              {page.summary.creditsRecords
                ? page.summary.credits.toLocaleString()
                : "未知"}
            </p>
          </details>

          {page.trends.length > 0 && (
            <section className="panel">
              <h2>每日趋势</h2>
              <div className="usage-trend-scroll">
                {page.trends.map((bucket) => {
                  const total = tokenTotal(bucket.tokens);
                  const max = Math.max(
                    ...page.trends.map((b) => tokenTotal(b.tokens)),
                  );
                  return (
                    <div className="trend-row" key={bucket.start}>
                      <span>
                        {new Date(bucket.start * 1000).toLocaleDateString()}
                      </span>
                      <div style={{ width: `${(total / (max || 1)) * 70}%` }} />
                      <small>{total.toLocaleString()}</small>
                    </div>
                  );
                })}
              </div>
            </section>
          )}

          <section className="panel">
            <div className="source-segments">
              {[
                ["records", "使用记录"],
                ["providers", "供应商"],
                ["models", "模型"],
                ["pricing", "定价"],
              ].map(([key, label]) => (
                <button
                  key={key}
                  aria-pressed={tab === key}
                  onClick={() => setTab(key as typeof tab)}
                >
                  {label}
                </button>
              ))}
            </div>

            {tab === "records" && (
              <>
                <div className="table-scroll">
                  <table>
                    <thead>
                      <tr>
                        <th>时间</th>
                        <th>供应商</th>
                        <th>API 模型</th>
                        <th>输入</th>
                        <th>缓存读/写</th>
                        <th>输出</th>
                        <th>思考</th>
                        <th>费用</th>
                      </tr>
                    </thead>
                    <tbody>
                      {page.records.map((r) => (
                        <tr
                          key={r.id}
                          onClick={() => setSelectedRecord(r)}
                          style={{ cursor: "pointer" }}
                          tabIndex={0}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") {
                              event.preventDefault();
                              event.stopPropagation();
                              setSelectedRecord(r);
                            }
                          }}
                        >
                          <td>
                            {r.timeConfidence === "unknown"
                              ? "历史累计 · 时间未知"
                              : new Date(r.occurredAt * 1000).toLocaleString()}
                            <small className="record-kind">
                              {r.granularity === "request"
                                ? "请求"
                                : r.granularity === "observed_delta"
                                  ? "观察差额"
                                  : "会话汇总"}
                            </small>
                          </td>
                          <td>{r.providerLabel}</td>
                          <td>{r.apiModel ?? "未知"}</td>
                          <td>{r.input?.toLocaleString() ?? "—"}</td>
                          <td>
                            {r.cacheRead?.toLocaleString() ?? "—"} /{" "}
                            {r.cacheWrite?.toLocaleString() ?? "—"}
                          </td>
                          <td>{r.output?.toLocaleString() ?? "—"}</td>
                          <td>{r.thinking?.toLocaleString() ?? "—"}</td>
                          <td>
                            {r.cost && r.currency
                              ? `${r.currency} ${r.cost}${r.complete ? "" : "（部分估算）"}`
                              : "未定价"}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <div className="pagination">
                  <button
                    disabled={offset === 0}
                    onClick={() => setOffset(Math.max(0, offset - limit))}
                  >
                    上一页
                  </button>
                  <span>
                    {offset + 1}–{Math.min(offset + limit, page.totalRecords)} /{" "}
                    {page.totalRecords}
                  </span>
                  <button
                    disabled={offset + limit >= page.totalRecords}
                    onClick={() => setOffset(offset + limit)}
                  >
                    下一页
                  </button>
                </div>
              </>
            )}

            {tab === "providers" && (
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>供应商</th>
                      <th>记录</th>
                      <th>总 Token</th>
                      <th>费用</th>
                    </tr>
                  </thead>
                  <tbody>
                    {page.providers.map((g) => (
                      <tr key={g.key}>
                        <td>{g.label}</td>
                        <td>{g.records.toLocaleString()}</td>
                        <td>{tokenTotal(g.tokens).toLocaleString()}</td>
                        <td>{formatCost(g.estimates)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {tab === "models" && (
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>模型</th>
                      <th>记录</th>
                      <th>总 Token</th>
                      <th>费用</th>
                    </tr>
                  </thead>
                  <tbody>
                    {page.models.map((g) => (
                      <tr key={g.key}>
                        <td>{g.label}</td>
                        <td>{g.records.toLocaleString()}</td>
                        <td>{tokenTotal(g.tokens).toLocaleString()}</td>
                        <td>{formatCost(g.estimates)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}

            {tab === "pricing" && (
              <div>
                <div className="usage-pricing-intro">
                  <p>
                    自定义单价优先；参考价用于自动补齐未定价记录。单位：每百万
                    Token。
                  </p>
                  <button onClick={() => setPrice(true)}>添加自定义单价</button>
                </div>
                <div className="table-scroll">
                  <table>
                    <thead>
                      <tr>
                        <th>供应商</th>
                        <th>API 模型</th>
                        <th>币种</th>
                        <th>生效时间</th>
                        <th>输入/1M</th>
                        <th>缓存读/1M</th>
                        <th>缓存写/1M</th>
                        <th>输出/1M</th>
                        <th>思考/1M</th>
                        <th>操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {page.prices.map((p) => (
                        <tr key={p.id}>
                          <td>
                            {p.providerId
                              ? (page.providerLabels?.[p.providerId] ??
                                p.providerId)
                              : "通用价格"}
                          </td>
                          <td>{p.apiModelId}</td>
                          <td>{p.currency}</td>
                          <td>
                            {p.source === "reference"
                              ? "参考价 · 2026-10-13"
                              : new Date(p.effectiveAt * 1000).toLocaleString()}
                          </td>
                          <td>{p.input}</td>
                          <td>{p.cacheRead}</td>
                          <td>{p.cacheWrite}</td>
                          <td>{p.output}</td>
                          <td>{p.thinking}</td>
                          <td>
                            {p.source === "reference" ? (
                              <small>自动参考 · 可添加自定义单价覆盖</small>
                            ) : (
                              <>
                                <button
                                  onClick={async () => {
                                    try {
                                      const filter = currentFilter();
                                      const preview = await request<{
                                        token: string;
                                        records: number;
                                        completeRecords: number;
                                        estimate: string;
                                        currency: string;
                                      }>("preview_usage_reprice", {
                                        priceId: p.id,
                                        filter,
                                      });
                                      setReprice({
                                        ...preview,
                                        priceId: p.id,
                                        filter,
                                      });
                                    } catch (e) {
                                      setError(errorText(e));
                                    }
                                  }}
                                >
                                  重新估算
                                </button>
                                <button
                                  onClick={async () => {
                                    try {
                                      await request("delete_price", {
                                        id: p.id,
                                      });
                                      await load();
                                    } catch (e) {
                                      setError(errorText(e));
                                    }
                                  }}
                                >
                                  删除
                                </button>
                              </>
                            )}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            )}
          </section>
        </>
      ) : null}

      {settings && data && (
        <UsageSettingsDialog
          onClose={() => setSettings(false)}
          initial={data.settings}
          save={async (value) => {
            try {
              if (!(await mutate("save_usage_settings", { settings: value })))
                throw new Error("保存失败，请检查工作区提示并重试");
              setSettings(false);
              await load();
            } catch (e) {
              setError(errorText(e));
            }
          }}
        />
      )}

      {price && page && (
        <PriceDialog
          onClose={() => {
            setPrice(false);
            void load();
          }}
          providers={page.providersFilter.filter(
            (p): p is string => p !== null,
          )}
          models={page.modelsFilter}
          labels={page.providerLabels}
        />
      )}

      {cleanup && data && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="清理统计"
          >
            <h2>清理保留期之外的统计？</h2>
            <p>
              只删除本应用超过 {data.settings.retentionDays}{" "}
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
                        data.settings.retentionDays * 86400,
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

      {reprice && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="重新估算预览"
          >
            <h2>重新估算预览</h2>
            <p>
              按选中的单价版本重新估算当前筛选范围内的 {reprice.records}{" "}
              条匹配记录，覆盖原有估算金额。
            </p>
            <p>
              新估算：{reprice.currency} {reprice.estimate} · 完整定价{" "}
              {reprice.completeRecords} 条。Token 不变。
            </p>
            <footer>
              <button onClick={() => setReprice(null)}>取消</button>
              <button
                className="primary"
                disabled={!reprice.records}
                onClick={async () => {
                  try {
                    await request("apply_usage_reprice", {
                      priceId: reprice.priceId,
                      filter: reprice.filter,
                      token: reprice.token,
                    });
                    setReprice(null);
                    await load();
                  } catch (e) {
                    setError(errorText(e));
                    setReprice(null);
                  }
                }}
              >
                确认重新估算
              </button>
            </footer>
          </section>
        </div>
      )}

      {selectedRecord && (
        <div className="modal-backdrop" onClick={() => setSelectedRecord(null)}>
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="记录详情"
            onClick={(e) => e.stopPropagation()}
          >
            <h2>记录详情</h2>
            <table>
              <tbody>
                <tr>
                  <td>时间</td>
                  <td>
                    {selectedRecord.timeConfidence === "unknown"
                      ? "历史累计 · 时间未知"
                      : new Date(
                          selectedRecord.occurredAt * 1000,
                        ).toLocaleString()}
                  </td>
                </tr>
                <tr>
                  <td>会话</td>
                  <td>{selectedRecord.session}</td>
                </tr>
                <tr>
                  <td>粒度</td>
                  <td>{selectedRecord.granularity}</td>
                </tr>
                <tr>
                  <td>时间置信度</td>
                  <td>{selectedRecord.timeConfidence}</td>
                </tr>
                <tr>
                  <td>来源</td>
                  <td>{selectedRecord.source}</td>
                </tr>
                <tr>
                  <td>供应商</td>
                  <td>{selectedRecord.providerLabel}</td>
                </tr>
                <tr>
                  <td>选择器</td>
                  <td>{selectedRecord.selector ?? "—"}</td>
                </tr>
                <tr>
                  <td>API 模型</td>
                  <td>{selectedRecord.apiModel ?? "—"}</td>
                </tr>
                <tr>
                  <td>请求 ID</td>
                  <td>{selectedRecord.requestId ?? "—"}</td>
                </tr>
                <tr>
                  <td>重试次数</td>
                  <td>{selectedRecord.attempt ?? "—"}</td>
                </tr>
                <tr>
                  <td>输入</td>
                  <td>{selectedRecord.input?.toLocaleString() ?? "—"}</td>
                </tr>
                <tr>
                  <td>缓存读</td>
                  <td>{selectedRecord.cacheRead?.toLocaleString() ?? "—"}</td>
                </tr>
                <tr>
                  <td>缓存写</td>
                  <td>{selectedRecord.cacheWrite?.toLocaleString() ?? "—"}</td>
                </tr>
                <tr>
                  <td>输出</td>
                  <td>{selectedRecord.output?.toLocaleString() ?? "—"}</td>
                </tr>
                <tr>
                  <td>思考</td>
                  <td>{selectedRecord.thinking?.toLocaleString() ?? "—"}</td>
                </tr>
                <tr>
                  <td>费用</td>
                  <td>
                    {selectedRecord.cost && selectedRecord.currency
                      ? `${selectedRecord.currency} ${selectedRecord.cost}${selectedRecord.complete ? "" : "（部分估算）"}`
                      : selectedRecord.complete
                        ? "未定价"
                        : "部分定价"}
                  </td>
                </tr>
              </tbody>
            </table>
            <footer>
              <button onClick={() => setSelectedRecord(null)}>关闭</button>
            </footer>
          </section>
        </div>
      )}
    </div>
  );
}

function UsageSettingsDialog({
  onClose,
  initial,
  save,
}: {
  onClose: () => void;
  initial: UsageSettings;
  save: (v: UsageSettings) => Promise<void>;
}) {
  const [value, setValue] = useState(initial);
  const [error, setError] = useState("");
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
          会话目录
          <input
            value={value.path}
            onChange={(e) => setValue({ ...value, path: e.target.value })}
          />
        </label>
        <label>
          日志目录
          <input
            value={value.logsPath}
            onChange={(e) => setValue({ ...value, logsPath: e.target.value })}
          />
        </label>
        <label>
          导出目录
          <input
            value={value.exportsPath}
            onChange={(e) =>
              setValue({ ...value, exportsPath: e.target.value })
            }
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
        <label>
          <input
            type="checkbox"
            checked={value.autoScan}
            onChange={(e) => setValue({ ...value, autoScan: e.target.checked })}
          />
          自动采集
        </label>
        <p>
          只保存计数、归属与时间，不保存对话正文。会话目录默认为
          ~/.factory/sessions。
        </p>
        {error && <p role="alert">{error}</p>}
        <footer>
          <button onClick={onClose}>取消</button>
          <button
            onClick={async () => {
              try {
                await save(value);
              } catch (e) {
                setError(errorText(e));
              }
            }}
          >
            保存设置
          </button>
        </footer>
      </section>
    </div>
  );
}

function PriceDialog({
  onClose,
  providers,
  models,
  labels,
}: {
  onClose: () => void;
  providers: string[];
  models: string[];
  labels: Record<string, string>;
}) {
  const [provider, setProvider] = useState(providers[0] ?? "");
  const [model, setModel] = useState(models[0] ?? "");
  const [currency, setCurrency] = useState("USD");
  const [values, setValues] = useState(["", "", "", "", ""]);
  const [effective, setEffective] = useState(() => {
    const d = new Date();
    return `${dateString(d)}T${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  });
  const [error, setError] = useState("");

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
                  providerId: provider,
                  model,
                  currency,
                  input: values[0] || null,
                  cacheRead: values[1] || null,
                  cacheWrite: values[2] || null,
                  output: values[3] || null,
                  thinking: values[4] || null,
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
            供应商
            <select
              value={provider}
              onChange={(e) => setProvider(e.target.value)}
            >
              <option value="">通用价格</option>
              {providers.map((p) => (
                <option key={p} value={p}>
                  {labels?.[p] ?? p}
                </option>
              ))}
            </select>
          </label>
          <label>
            API 模型 ID
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
          {["普通输入", "缓存读", "缓存写", "输出", "思考"].map(
            (label, index) => (
              <label key={label}>
                {label}
                <input
                  type="number"
                  min="0"
                  step="0.000001"
                  placeholder="未定价"
                  value={values[index]}
                  onChange={(e) =>
                    setValues((v) =>
                      v.map((s, i) => (i === index ? e.target.value : s)),
                    )
                  }
                />
              </label>
            ),
          )}
          <label>
            生效时间
            <input
              type="datetime-local"
              required
              value={effective}
              onChange={(e) => setEffective(e.target.value)}
            />
          </label>
          <p>手动单价用于后续导入事件的估算，不改写已存历史金额。</p>
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
