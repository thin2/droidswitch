import { invoke, isTauri } from "@tauri-apps/api/core";
export async function request<T>(
  command: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  if (!isTauri())
    throw new Error(
      "请使用桌面版连接本地数据。浏览器预览不能读取或写入 Droid 配置。",
    );
  return invoke<T>(command, args);
}
export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
