import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

interface UpdateInfo {
  available: boolean;
  current_version: string;
  latest_version?: string;
  download_url?: string;
  release_notes?: string;
  release_date?: string;
}

export function UpdateChecker() {
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const checkForUpdates = async () => {
    setChecking(true);
    setError(null);
    try {
      const info = await invoke<UpdateInfo>("check_for_updates");
      setUpdateInfo(info);
    } catch (err) {
      setError(err as string);
    } finally {
      setChecking(false);
    }
  };

  const installUpdate = async () => {
    setInstalling(true);
    setError(null);
    try {
      await invoke("install_update");
      // 更新成功，应用将重启
    } catch (err) {
      setError(err as string);
      setInstalling(false);
    }
  };

  return (
    <div className="update-checker">
      <div className="section-header">
        <h3>软件更新</h3>
      </div>

      <div className="update-info">
        {updateInfo && (
          <div className="current-version">
            当前版本: <strong>{updateInfo.current_version}</strong>
          </div>
        )}

        {!updateInfo && !checking && (
          <button onClick={checkForUpdates} disabled={checking || installing}>
            检查更新
          </button>
        )}

        {checking && <div className="status-text">正在检查更新...</div>}

        {updateInfo && !updateInfo.available && (
          <div className="status-text success">已是最新版本</div>
        )}

        {updateInfo && updateInfo.available && (
          <div className="update-available">
            <div className="update-header">
              <span className="badge new">有新版本</span>
              <strong>{updateInfo.latest_version}</strong>
            </div>

            {updateInfo.release_notes && (
              <div className="release-notes">
                <h4>更新说明</h4>
                <div className="notes-content">{updateInfo.release_notes}</div>
              </div>
            )}

            {updateInfo.release_date && (
              <div className="release-date">
                发布时间: {updateInfo.release_date}
              </div>
            )}

            <div className="update-actions">
              <button
                onClick={installUpdate}
                disabled={installing}
                className="primary"
              >
                {installing ? "正在安装..." : "立即更新"}
              </button>
              <button onClick={checkForUpdates} disabled={checking || installing}>
                重新检查
              </button>
            </div>

            {installing && (
              <div className="status-text">
                正在下载并安装更新，完成后应用将自动重启...
              </div>
            )}
          </div>
        )}

        {error && (
          <div className="status-text error">
            {error}
            {updateInfo?.download_url && (
              <div className="manual-download">
                <a href={updateInfo.download_url} target="_blank" rel="noopener noreferrer">
                  手动下载最新版本
                </a>
              </div>
            )}
          </div>
        )}
      </div>

      <style>{`
        .update-checker {
          padding: 20px;
          max-width: 600px;
        }

        .section-header {
          margin-bottom: 20px;
        }

        .section-header h3 {
          margin: 0;
          font-size: 18px;
          font-weight: 600;
        }

        .update-info {
          display: flex;
          flex-direction: column;
          gap: 16px;
        }

        .current-version {
          font-size: 14px;
          color: #666;
        }

        .status-text {
          padding: 12px;
          border-radius: 6px;
          font-size: 14px;
        }

        .status-text.success {
          background: #f0f9ff;
          color: #0369a1;
        }

        .status-text.error {
          background: #fef2f2;
          color: #dc2626;
        }

        .update-available {
          border: 1px solid #e5e7eb;
          border-radius: 8px;
          padding: 16px;
          display: flex;
          flex-direction: column;
          gap: 12px;
        }

        .update-header {
          display: flex;
          align-items: center;
          gap: 8px;
          font-size: 16px;
        }

        .badge {
          padding: 2px 8px;
          border-radius: 4px;
          font-size: 12px;
          font-weight: 600;
        }

        .badge.new {
          background: #dbeafe;
          color: #1e40af;
        }

        .release-notes {
          margin-top: 8px;
        }

        .release-notes h4 {
          margin: 0 0 8px 0;
          font-size: 14px;
          font-weight: 600;
        }

        .notes-content {
          padding: 12px;
          background: #f9fafb;
          border-radius: 6px;
          font-size: 13px;
          line-height: 1.6;
          white-space: pre-wrap;
        }

        .release-date {
          font-size: 13px;
          color: #6b7280;
        }

        .update-actions {
          display: flex;
          gap: 8px;
          margin-top: 8px;
        }

        button {
          padding: 8px 16px;
          border: 1px solid #d1d5db;
          border-radius: 6px;
          background: white;
          cursor: pointer;
          font-size: 14px;
          transition: all 0.2s;
        }

        button:hover:not(:disabled) {
          background: #f9fafb;
        }

        button.primary {
          background: #2563eb;
          color: white;
          border-color: #2563eb;
        }

        button.primary:hover:not(:disabled) {
          background: #1d4ed8;
        }

        button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .manual-download {
          margin-top: 8px;
        }

        .manual-download a {
          color: #2563eb;
          text-decoration: none;
        }

        .manual-download a:hover {
          text-decoration: underline;
        }
      `}</style>
    </div>
  );
}
