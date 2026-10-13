import { useState } from "react";
import { useWorkspace } from "../../lib/workspace";
import { request, errorText } from "../../lib/api";
import type { Scheme, ObservedModel } from "../../lib/types";
import { SwitchConfirmDialog } from "../../components/SwitchConfirmDialog";
const empty: Scheme = {
  source: null,
  official: { model: null, reasoning: null },
  api: { modelId: null, reasoning: null },
};
export function SourcePlans({
  browse,
  onBrowse,
}: {
  browse: "official" | "api";
  onBrowse: (source: "official" | "api") => void;
}) {
  const { workspace, busy, mutate, setNotice } = useWorkspace();
  const [catalog, setCatalog] = useState<ObservedModel[]>([]),
    [loading, setLoading] = useState(false),
    [error, setError] = useState("");
  const [switchConfirm, setSwitchConfirm] = useState<"official" | "api" | null>(null);
  const scheme = workspace?.scheme ?? empty;
  const observed =
    browse === "official"
      ? catalog.filter((m) => m.source === "official")
      : catalog.filter((m) => m.source === "api");
  const selected =
    browse === "official"
      ? observed.find((m) => m.id === scheme.official.model)
      : observed.find(
          (m) =>
            m.name ===
            workspace?.catalog.models.find((m) => m.id === scheme.api.modelId)
              ?.displayName,
        );
  async function refresh() {
    setLoading(true);
    setError("");
    try {
      setCatalog(await request<ObservedModel[]>("discover_models"));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setLoading(false);
    }
  }
  const sourceLabel = (source: string | null | undefined) =>
    source === "official"
      ? "Factory 订阅"
      : source === "api"
        ? "自定义 API"
        : "未设置";

  const handleSetAsActive = () => {
    // Check if switching source from applied scheme
    const currentAppliedSource = workspace?.appliedScheme?.source;
    const isDifferentSource = currentAppliedSource && currentAppliedSource !== browse;

    if (isDifferentSource) {
      // Show confirmation dialog
      setSwitchConfirm(browse);
    } else {
      // No confirmation needed, apply directly
      void mutate("save_scheme", {
        scheme: { ...scheme, source: browse },
      });
    }
  };

  const handleConfirmSwitch = () => {
    void mutate("save_scheme", {
      scheme: { ...scheme, source: browse },
    });
    setSwitchConfirm(null);
  };
  return (
    <section className="source-panel">
      <div className="source-segments" role="group" aria-label="浏览模型来源">
        <button
          aria-pressed={browse === "official"}
          onClick={() => onBrowse("official")}
        >
          Factory 订阅
        </button>
        <button aria-pressed={browse === "api"} onClick={() => onBrowse("api")}>
          自定义 API
        </button>
      </div>
      <p>
        当前浏览：{sourceLabel(browse)} · 待应用：{sourceLabel(scheme.source)} ·
        上次应用：{sourceLabel(workspace?.appliedScheme?.source)}
      </p>
      <div className="source-controls">
        <label>
          {browse === "official" ? "官方默认模型" : "API 默认模型"}
          <select
            disabled={busy}
            value={
              browse === "official"
                ? (scheme.official.model ?? "")
                : (scheme.api.modelId ?? "")
            }
            onChange={(e) => {
              const value = e.target.value || null;
              void mutate("save_scheme", {
                scheme:
                  browse === "official"
                    ? {
                        ...scheme,
                        official: { ...scheme.official, model: value },
                      }
                    : { ...scheme, api: { ...scheme.api, modelId: value } },
              });
            }}
          >
            <option value="">未选择</option>
            {browse === "official" ? (
              <>
                {scheme.official.model &&
                  !observed.some((m) => m.id === scheme.official.model) && (
                    <option value={scheme.official.model}>
                      {scheme.official.model}（待刷新确认）
                    </option>
                  )}
                {observed.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
              </>
            ) : (
              workspace?.catalog.models
                .filter((m) => m.enabled)
                .map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.displayName} ·{" "}
                    {
                      workspace.catalog.providers.find(
                        (p) => p.id === m.providerId,
                      )?.name
                    }
                  </option>
                ))
            )}
          </select>
        </label>
        <label>
          思考强度
          <select
            disabled={busy || !selected}
            value={
              (browse === "official"
                ? scheme.official.reasoning
                : scheme.api.reasoning) ?? ""
            }
            onChange={(e) => {
              const reasoning = e.target.value || null;
              void mutate("save_scheme", {
                scheme:
                  browse === "official"
                    ? { ...scheme, official: { ...scheme.official, reasoning } }
                    : { ...scheme, api: { ...scheme.api, reasoning } },
              });
            }}
          >
            <option value="">模型默认</option>
            {selected?.reasoning.map((r) => (
              <option key={r} value={r}>
                {r}
              </option>
            ))}
          </select>
        </label>
        <button
          disabled={loading || busy || !workspace}
          onClick={() => void refresh()}
        >
          {loading ? "正在检测…" : "刷新 Droid 模型目录"}
        </button>
        <button
          disabled={
            busy ||
            (browse === "official"
              ? !scheme.official.model
              : !scheme.api.modelId)
          }
          onClick={handleSetAsActive}
        >
          将此方案设为待应用
        </button>
      </div>
      {error && <p role="alert">{error}</p>}
      {browse === "official" && (
        <>
          <p>登录状态：状态待确认。当前无法确认套餐或剩余额度。</p>
          <button
            onClick={async () => {
              try {
                await request("open_factory_login");
                setNotice(
                  "已打开 Factory 页面。请在 Droid 中完成登录，并在新会话确认模型。",
                );
              } catch (e) {
                setError(errorText(e));
              }
            }}
          >
            前往 Droid 登录
          </button>
        </>
      )}
      <p className="muted">
        切换浏览不会写入配置。只有底部”确认应用”生效；已有会话、CLI
        显式参数、项目设置及任务级模型可能优先。不会自动回退到另一付费来源。
      </p>
      {switchConfirm && (
        <SwitchConfirmDialog
          targetSource={switchConfirm}
          customModelCount={workspace?.catalog.models.length ?? 0}
          onConfirm={handleConfirmSwitch}
          onCancel={() => setSwitchConfirm(null)}
        />
      )}
    </section>
  );
}
