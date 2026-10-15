import { MaintenanceSettings } from "./MaintenanceSettings";
import { request, errorText } from "../lib/api";
import { useState } from "react";
import { ExternalModels } from "./ExternalModels";
import { useWorkspace } from "../lib/workspace";
import { UpdateChecker } from "../components/UpdateChecker";

export function Settings() {
  const { workspace, busy, mutate, setNotice } = useWorkspace();
  const [path, setPath] = useState<string | null>(null);
  return (
    <>
      <UpdateChecker />
      <section className="panel">
        <h2>Droid 配置位置</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void mutate("set_target_path", {
              path: path ?? workspace?.targetPath ?? "",
            });
          }}
        >
          <label>
            目标配置文件
            <input
              required
              value={path ?? workspace?.targetPath ?? ""}
              onChange={(e) => setPath(e.target.value)}
              placeholder="~/.factory/settings.json"
            />
          </label>
          <p>
            支持绝对路径与 ~ 主目录。选择路径会校验现有
            JSON；保存路径不会创建或覆盖文件。
          </p>
          <button className="primary" disabled={busy || !workspace}>
            保存路径
          </button>
        </form>
        <h2>草稿导出</h2>
        <button
          onClick={async () => {
            try {
              const text = await request<string>("export_draft");
              const url = URL.createObjectURL(
                new Blob([text], { type: "application/json" }),
              );
              const a = document.createElement("a");
              a.href = url;
              a.download = "droid-switch-draft-redacted.json";
              a.click();
              setTimeout(() => URL.revokeObjectURL(url), 1000);
            } catch (e) {
              setNotice(errorText(e));
            }
          }}
        >
          导出脱敏草稿
        </button>
        <p>导出用于排查或记录，密钥已移除，不能直接作为 Droid 配置使用。</p>
        <h2>配置校验</h2>
        <button
          onClick={async () => {
            try {
              setNotice(await request<string>("validate_workspace"));
            } catch (e) {
              setNotice(errorText(e));
            }
          }}
        >
          检查字段、JSON 与引用
        </button>
        <h2>凭据与生效方式</h2>
        <p>
          推荐使用环境变量。请在启动 Droid
          的终端设置变量；桌面应用继承的是自身启动时的环境。
        </p>
        <p>配置应用后，请重启 Droid 或开启新会话，并用 /model 确认实际模型。</p>
        <p>当前适配开发契约：Droid 0.238.0。Windows 实机兼容性待验收。</p>
      </section>
      <MaintenanceSettings />
      <ExternalModels />
    </>
  );
}
