import { useEffect, useRef, useState } from "react";
import { request, errorText } from "../../lib/api";
import { useWorkspace } from "../../lib/workspace";
import { CodexQuota } from "./CodexQuota";
type Status = {
  connected: boolean;
  connectionError: string | null;
  running: boolean;
  savedLogin: boolean;
  loginPending: boolean;
  manualCallback: boolean;
  callbackSubmitted: boolean;
  account: string | null;
  models: string[];
  endpoint: string;
  version: string;
};
export function CodexView() {
  const { workspace, reload } = useWorkspace();
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState(""),
    [notice, setNotice] = useState("");
  const [model, setModel] = useState(""),
    [confirmLogout, setConfirmLogout] = useState(false);
  const [manual, setManual] = useState(true);
  const [callbackUrl, setCallbackUrl] = useState("");
  const active = useRef(true),
    locked = useRef(false);
  const wasConnected = useRef(false);
  async function act(action: string, quiet = false) {
    if (locked.current) return;
    locked.current = true;
    if (!quiet) setBusy(true);
    if (!quiet) {
      setError("");
      setNotice("");
    }
    try {
      const pastedCallback =
        action === "submit_callback" ? callbackUrl.trim() : undefined;
      if (action === "submit_callback") setCallbackUrl("");
      const next = await request<Status>("codex_action", {
        action,
        model: action === "add_model" ? model : null,
        revision: workspace?.revision ?? null,
        ...(pastedCallback === undefined
          ? {}
          : { callbackUrl: pastedCallback }),
      });
      if (!active.current) return;
      setStatus((previous) =>
        JSON.stringify(previous) === JSON.stringify(next) ? previous : next,
      );
      if (!next.loginPending) setCallbackUrl("");
      if (next.connected && !wasConnected.current) await reload();
      wasConnected.current = next.connected;
      if (!quiet) setError("");
      setModel((previous) =>
        next.models.includes(previous) ? previous : (next.models[0] ?? ""),
      );
      if (action === "add_model") {
        await reload();
        setNotice(
          "已备份并写入 Droid 配置。请新建 Droid 会话使用，或通过 /model 切换模型。",
        );
      }
      if (action === "logout") {
        setConfirmLogout(false);
        setNotice(
          "已移除本应用的登录凭据。已有模型配置仍保留，重新登录并启动后可继续使用。",
        );
      }
    } catch (e) {
      if (active.current) setError(errorText(e));
    } finally {
      locked.current = false;
      if (active.current && !quiet) setBusy(false);
    }
  }
  useEffect(() => {
    active.current = true;
    void act("status");
    const timer = window.setInterval(() => void act("status", true), 2500);
    return () => {
      active.current = false;
      window.clearInterval(timer);
    };
  }, []);
  return (
    <section className="codex-view" aria-label="Codex 授权">
      <div className="card codex-card">
        <div className="codex-heading">
          <div>
            <h2>ChatGPT 账号</h2>
            <p className="muted">登录成功后自动配置 Droid，并设置默认模型。</p>
          </div>
          <span className="badge">
            {status?.loginPending
              ? "等待浏览器授权"
              : status?.account
                ? "已登录"
                : status?.savedLogin
                  ? "已保存登录"
                  : "未登录"}
          </span>
        </div>
        {(error || status?.connectionError) && (
          <p role="alert" className="error-banner">
            {error || status?.connectionError}
          </p>
        )}
        {notice && (
          <p role="status" className="notice">
            {notice}
          </p>
        )}
        <div className="codex-account">
          <strong>
            {status?.account ??
              (status?.loginPending
                ? "请在系统浏览器中完成登录"
                : status?.savedLogin
                  ? "登录已保存，启动服务即可使用"
                  : "连接你的 ChatGPT 账号")}
          </strong>
          <p className="muted">
            登录和令牌刷新由内置 CLIProxyAPI 管理，独立于 Codex、CC Switch
            的登录状态。
          </p>
        </div>
        {status?.running && status.account && !status.loginPending && (
          <CodexQuota key={status.account} />
        )}
        <div className="actions">
          {!status?.account && !status?.savedLogin && !status?.loginPending && (
            <>
              <label>
                <input
                  type="checkbox"
                  checked={manual}
                  disabled={busy}
                  onChange={(e) => setManual(e.target.checked)}
                />
                手动粘贴回调地址
              </label>
              <button
                className="primary"
                disabled={busy || !status}
                onClick={() => void act(manual ? "login_manual" : "login")}
              >
                使用 ChatGPT 登录
              </button>
            </>
          )}
          {status?.loginPending && (
            <button disabled={busy} onClick={() => void act("cancel")}>
              取消授权
            </button>
          )}
          {(status?.account || status?.savedLogin) && (
            <button disabled={busy} onClick={() => setConfirmLogout(true)}>
              退出账号
            </button>
          )}
          <button disabled={busy} onClick={() => void act("status")}>
            刷新状态
          </button>
        </div>
        {status?.loginPending && (
          <div className="codex-callback">
            {status.manualCallback ? (
              <>
                <p className="muted">
                  浏览器授权后，复制地址栏中的完整回调地址并粘贴到这里。
                  手动模式下浏览器可能显示“无法连接本机”，仍可复制地址。
                  地址只用于本次登录，不会保存到配置或日志。
                </p>
                {status.callbackSubmitted ? (
                  <p role="status">回调已提交，等待代理换取并保存登录令牌…</p>
                ) : (
                  <>
                    <label htmlFor="codex-callback-url">完整回调地址</label>
                    <input
                      id="codex-callback-url"
                      type="password"
                      autoComplete="off"
                      spellCheck={false}
                      value={callbackUrl}
                      disabled={busy}
                      onChange={(e) => setCallbackUrl(e.target.value)}
                      placeholder="http://localhost:1455/auth/callback?…"
                    />
                    <button
                      disabled={busy || !callbackUrl.trim()}
                      onClick={() => void act("submit_callback")}
                    >
                      提交回调
                    </button>
                  </>
                )}
              </>
            ) : (
              <p className="muted">
                等待自动回调及令牌保存。浏览器显示成功不代表登录已完成，请以本页状态为准。
              </p>
            )}
          </div>
        )}
        {confirmLogout && (
          <div className="codex-confirm">
            <p>
              退出将停止本地服务并删除本应用保存的登录凭据，正在使用此服务的
              Droid 请求会中断。
            </p>
            <div className="actions">
              <button disabled={busy} onClick={() => void act("logout")}>
                确认退出
              </button>
              <button onClick={() => setConfirmLogout(false)}>保留账号</button>
            </div>
          </div>
        )}
      </div>
      <div className="card codex-card">
        <div className="codex-heading">
          <div>
            <h2>本地服务</h2>
            <p className="muted">
              {status?.endpoint ?? "http://127.0.0.1:18317/v1"}
            </p>
          </div>
          <span className="badge">{status?.running ? "运行中" : "已停止"}</span>
        </div>
        <div className="actions">
          <button
            disabled={busy || !status}
            onClick={() => void act(status?.running ? "stop" : "start")}
          >
            {status?.running ? "停止服务" : "启动服务"}
          </button>
          <span className="muted">
            使用这些模型时，请保持 Droid Switch 和本地服务运行。启动时读取 macOS
            系统代理；修改代理后请重启本地服务。
          </span>
        </div>
      </div>
      {status?.account && (
        <div className="card codex-card">
          <h2>{status.connected ? "已接入 Droid" : "接入 Droid"}</h2>
          <p className="muted">
            模型已自动接入后，新建 Droid 会话即可使用，或在 /model 中切换带有
            “(ChatGPT)” 的模型。目录可见不代表账号一定有调用权限。
          </p>
          <div className="codex-model-row">
            <label htmlFor="codex-model">默认模型</label>
            <select
              id="codex-model"
              value={model}
              onChange={(e) => setModel(e.target.value)}
              disabled={busy || !status.models.length}
            >
              {!status.models.length && <option value="">暂无可用模型</option>}
              {status.models.map((m) => (
                <option key={m} value={m}>
                  {m}
                </option>
              ))}
            </select>
            <button
              className="primary"
              disabled={busy || !model}
              onClick={() => void act("add_model")}
            >
              设为默认并接入 Droid
            </button>
          </div>
        </div>
      )}
      <details className="codex-details">
        <summary>授权方式与存储说明</summary>
        <p>
          当前为 CLIProxyAPI 兼容接入，并非 Droid Switch 自有的官方 Sign in with
          ChatGPT 应用授权。首版支持单账号，不会自动切换其他账号或付费 API。
        </p>
        <p>
          登录凭据保存于本应用私有目录，仅当前用户可访问；尚未使用系统钥匙串。Droid
          配置只写入本地服务密钥。退出账号会清除本地凭据，不等同于撤销服务端所有会话。
        </p>
        <p>
          {status?.version ?? "CLIProxyAPI 8.0.27"} · macOS Apple Silicon ·
          模型及工具兼容性需实际调用确认。
        </p>
      </details>
    </section>
  );
}
