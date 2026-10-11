import { useState } from "react";
import { request, errorText } from "../lib/api";
import { useWorkspace } from "../lib/workspace";
export function MaintenanceSettings() {
  const { workspace, busy, mutate, setNotice } = useWorkspace();
  const [retention, setRetention] = useState<number | null>(null),
    [cleanup, setCleanup] = useState(false),
    [error, setError] = useState("");
  return (
    <section className="panel">
      <h2>Droid 检测</h2>
      <button
        onClick={async () => {
          try {
            setNotice(
              `已检测到支持的 Droid ${await request<string>("detect_droid")}`,
            );
          } catch (e) {
            setError(errorText(e));
          }
        }}
      >
        检测版本与可执行文件
      </button>
      <h2>外观</h2>
      <p>当前使用暖白主题，详情面板随窗口宽度自动切换为抽屉。</p>
      <h2>备份保留策略</h2>
      <label>
        保留天数
        <input
          type="number"
          min="1"
          max="3650"
          value={retention ?? workspace?.backupRetentionDays ?? 30}
          onChange={(e) => setRetention(Number(e.target.value))}
        />
      </label>
      <button
        disabled={busy}
        onClick={() =>
          void mutate("set_backup_retention", {
            days: retention ?? workspace?.backupRetentionDays ?? 30,
          })
        }
      >
        保存保留期
      </button>
      <button onClick={() => setCleanup(true)}>清理过期备份</button>
      <p>手动清理超过保留期的备份，至少保留最近一份；中断恢复期间禁止清理。</p>
      {error && <p role="alert">{error}</p>}
      {cleanup && (
        <div className="modal-backdrop">
          <section
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-label="清理备份"
          >
            <h2>清理过期备份？</h2>
            <p>仅清理超过已保存保留期的应用备份，当前配置不受影响。</p>
            <footer>
              <button onClick={() => setCleanup(false)}>取消</button>
              <button
                onClick={async () => {
                  try {
                    const count = await request<number>("prune_backups", {
                      confirmed: true,
                    });
                    setCleanup(false);
                    setNotice(`已清理 ${count} 份过期备份`);
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
    </section>
  );
}
