# Droid 实测基线

- 采集日期：2026-10-11（Asia/Shanghai）
- 方法：本机会话运行 `droid --version`，退出码 0。
- 实际输出：`0.238.0`
- 可执行文件：`/Users/hanning/.local/bin/droid`
- 平台：macOS / Apple Silicon（Darwin arm64）
- 最小输入：`fixtures/settings.minimal.json`，未覆盖用户配置。
- BYOK key：规格记录暂缺；本项没有发起模型调用。
- 后续 /model、默认字段、重载、覆盖和日志口径均未实测，不能据版本号推断。
