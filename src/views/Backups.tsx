import { useEffect, useState } from "react";
import { request, errorText } from "../lib/api";
import { useWorkspace } from "../lib/workspace";
interface Backup {
  id: string;
  targetPath: string;
  createdAt: number;
  fileExisted: boolean;
}
interface Restore {
  token: string;
  before: unknown;
  after: unknown;
  removesFile: boolean;
}
export function Backups() {
  const { workspace, busy, mutate, setNotice } = useWorkspace();
  const [items, setItems] = useState<Backup[]>([]),
    [error, setError] = useState(""),
    [preview, setPreview] = useState<(Restore & { id: string }) | null>(null),
    [loading, setLoading] = useState(false);
  async function load() {
    try {
      setItems(await request<Backup[]>("list_backups"));
      setError("");
    } catch (e) {
      setError(errorText(e));
    }
  }
  useEffect(() => {
    void load();
  }, [workspace?.revision]);
  return (
    <section className="panel">
      <h2>配置备份</h2>
      <button
        onClick={async () => {
          try {
            await request("open_backup_directory");
          } catch (e) {
            setError(errorText(e));
          }
        }}
      >
        打开备份目录
      </button>
      <p>每次应用或恢复前自动备份。备份可能含明文凭据，保存在应用私有目录。</p>
      {error && <p role="alert">{error}</p>}
      {!items.length && <p>尚无备份。</p>}
      {items.map((b) => (
        <article className="backup-row" key={b.id}>
          <div>
            <strong>{new Date(b.createdAt * 1000).toLocaleString()}</strong>
            <p>
              <code>{b.targetPath}</code>
            </p>
            <small>{b.fileExisted ? "原文件快照" : "原文件不存在"}</small>
          </div>
          <button
            disabled={loading || busy || b.targetPath !== workspace?.targetPath}
            onClick={async () => {
              setLoading(true);
              try {
                setPreview({
                  ...(await request<Restore>("preview_restore", { id: b.id })),
                  id: b.id,
                });
              } catch (e) {
                setError(errorText(e));
              } finally {
                setLoading(false);
              }
            }}
          >
            预览恢复
          </button>
        </article>
      ))}
      {preview && (
        <div className="modal-backdrop">
          <section
            className="modal wide-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="restore-title"
          >
            <h2 id="restore-title">恢复配置备份</h2>
            <p>
              将先备份当前文件，再恢复此快照。未应用的模型草稿将恢复为快照对应的管理状态。
            </p>
            {preview.removesFile && (
              <p>此快照记录的是文件不存在，恢复会移除当前配置文件。</p>
            )}
            <div className="diff">
              <section>
                <h3>当前（脱敏）</h3>
                <pre>{JSON.stringify(preview.before, null, 2)}</pre>
              </section>
              <section>
                <h3>恢复后（脱敏）</h3>
                <pre>{JSON.stringify(preview.after, null, 2)}</pre>
              </section>
            </div>
            <footer>
              <button disabled={busy} onClick={() => setPreview(null)}>
                取消
              </button>
              <button
                disabled={busy}
                className="primary"
                onClick={async () => {
                  if (
                    await mutate("restore_backup", {
                      id: preview.id,
                      token: preview.token,
                    })
                  ) {
                    setPreview(null);
                    setNotice("备份已恢复，请新建 Droid 会话确认。");
                  }
                }}
              >
                确认恢复
              </button>
            </footer>
          </section>
        </div>
      )}
    </section>
  );
}
