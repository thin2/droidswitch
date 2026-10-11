import type { Page } from '../App';

const content: Record<Page, { title: string; description: string }> = {
  models: { title: '模型库尚未连接', description: '模型读取与草稿存储接入后，可在这里管理 API 模型。' },
  providers: { title: '供应商管理待接入', description: '连接、协议和凭据将分别保存；同一服务可配置多个连接。' },
  usage: { title: '尚未采集用量', description: '本地会话采集器尚未接入，当前无法提供 Token 或费用统计。' },
  backups: { title: '备份读取待接入', description: '配置应用流程接入后，可在这里查看备份并预览恢复影响。' },
  settings: { title: '本地配置', description: '目标路径设置和 Droid 检测将在配置内核接入后开放。' },
};

export function WorkspacePage({ page }: { page: Page }) {
  const item = content[page];
  return <section className="panel" aria-label={item.title}>
    <span className="status">功能待接入</span>
    <h2>{item.title}</h2><p>{item.description}</p>
    {page === 'models' && <a className="button" href="#/providers">查看供应商页面</a>}
    {page === 'settings' && <dl><dt>计划默认目标</dt><dd><code>~/.factory/settings.json</code></dd><dt>凭据默认方式</dt><dd>环境变量引用；需要在启动 Droid 的终端配置。</dd></dl>}
  </section>;
}
