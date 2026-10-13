import { useEffect, useRef, useState } from "react";
import { errorText, request } from "../../lib/api";

type Quota = {
  windows: {
    label: string;
    remainingPercent: number | null;
    windowSeconds: number | null;
    resetAt: number | null;
  }[];
  allowed: boolean | null;
  limitReached: boolean | null;
  fetchedAt: number;
};

function duration(seconds: number | null) {
  if (seconds === null) return "周期未知";
  if (seconds % 86400 === 0) return `${seconds / 86400} 天`;
  if (seconds % 3600 === 0) return `${seconds / 3600} 小时`;
  return `${Math.round(seconds / 60)} 分钟`;
}

export function CodexQuota() {
  const [quota, setQuota] = useState<Quota | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const active = useRef(true);
  const locked = useRef(false);
  useEffect(() => {
    active.current = true;
    return () => { active.current = false; };
  }, []);
  async function refresh() {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setError("");
    try {
      const next = await request<Quota>("codex_quota");
      if (active.current) setQuota(next);
    } catch (e) {
      if (active.current) setError(errorText(e));
    } finally {
      locked.current = false;
      if (active.current) setBusy(false);
    }
  }
  return (
    <section aria-label="订阅额度">
      <div className="codex-heading">
        <h3>订阅额度</h3>
        <button disabled={busy} onClick={() => void refresh()}>
          {busy ? "查询额度中…" : "刷新额度"}
        </button>
      </div>
      <p className="muted">按需查询 ChatGPT 返回的额度窗口，不消耗推理请求。剩余比例不是 Token 余额或现金余额。</p>
      {error && <p role="alert">{error}{quota ? "；下方保留上次结果，非最新额度。" : "；剩余额度未知。"}</p>}
      {!quota && !error && <p className="muted">尚未查询额度，点击“刷新额度”获取。</p>}
      {quota && <>
        <p role="status">
          {quota.limitReached === true ? "已达到使用限制" : quota.allowed === false ? "当前不允许使用" : "上次查询结果"}
          {" · "}{new Date(quota.fetchedAt * 1000).toLocaleString()}
        </p>
        <div className="table-scroll">
          <table>
            <thead><tr><th>额度窗口</th><th>周期</th><th>剩余</th><th>重置时间</th></tr></thead>
            <tbody>{quota.windows.map((window) => <tr key={window.label}>
              <td>{window.label}</td>
              <td>{duration(window.windowSeconds)}</td>
              <td>{window.remainingPercent === null ? "未知" : `${Number(window.remainingPercent.toFixed(1))}%`}</td>
              <td>{window.resetAt === null ? "未知" : new Date(window.resetAt * 1000).toLocaleString()}</td>
            </tr>)}</tbody>
          </table>
        </div>
      </>}
    </section>
  );
}
