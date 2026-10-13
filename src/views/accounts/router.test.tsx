import { afterEach, expect, test, vi } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodexView } from "./CodexView";
import { request } from "../../lib/api";
const reload = vi.fn();
vi.mock("../../lib/api", () => ({
  request: vi.fn(),
  errorText: (e: unknown) => String(e),
}));
vi.mock("../../lib/workspace", () => ({
  useWorkspace: () => ({ workspace: { revision: 7 }, reload }),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});
const stopped = {
  running: false,
  loginPending: false,
  savedLogin: false,
  account: null,
  models: [],
  endpoint: "http://127.0.0.1:18317/v1",
  version: "CLIProxyAPI 8.0.27",
};
test("quiet polling keeps buttons enabled and does not toggle busy styles", async () => {
  vi.useFakeTimers();
  let finishPoll!: (value: typeof stopped) => void;
  vi.mocked(request).mockResolvedValueOnce(stopped).mockImplementationOnce(
    () => new Promise((resolve) => { finishPoll = resolve; }),
  );
  render(<CodexView />);
  await act(async () => {});
  const button = screen.getByRole("button", { name: "使用 ChatGPT 登录" });
  expect(button.hasAttribute("disabled")).toBe(false);
  await act(async () => { vi.advanceTimersByTime(2500); });
  expect(button.hasAttribute("disabled")).toBe(false);
  await act(async () => { finishPoll(stopped); });
  expect(button.hasAttribute("disabled")).toBe(false);
});
test("saved login remains removable while service is stopped", async () => {
  vi.mocked(request).mockResolvedValue({ ...stopped, savedLogin: true });
  render(<CodexView />);
  await userEvent.click(
    await screen.findByRole("button", { name: "退出账号" }),
  );
  expect(
    vi.mocked(request).mock.calls.some(([, args]) => args?.action === "logout"),
  ).toBe(false);
  await userEvent.click(screen.getByRole("button", { name: "确认退出" }));
  expect(request).toHaveBeenCalledWith(
    "codex_action",
    expect.objectContaining({ action: "logout" }),
  );
});
test("login enters pending state and can be cancelled", async () => {
  vi.mocked(request).mockImplementation(async (_, args) =>
    args?.action === "login_manual"
      ? { ...stopped, running: true, loginPending: true }
      : stopped,
  );
  render(<CodexView />);
  const login = await screen.findByRole("button", {
    name: "使用 ChatGPT 登录",
  });
  await vi.waitFor(() => expect(login.hasAttribute("disabled")).toBe(false));
  await userEvent.click(login);
  await userEvent.click(
    await screen.findByRole("button", { name: "取消授权" }),
  );
  expect(request).toHaveBeenCalledWith(
    "codex_action",
    expect.objectContaining({ action: "cancel" }),
  );
});
test("manual callback is submitted once and cleared while awaiting credentials", async () => {
  const pending = {
    ...stopped,
    running: true,
    loginPending: true,
    manualCallback: true,
  };
  vi.mocked(request).mockImplementation(async (_, args) =>
    args?.action === "submit_callback"
      ? { ...pending, callbackSubmitted: true }
      : pending,
  );
  render(<CodexView />);
  const field = await screen.findByLabelText("完整回调地址");
  const callback =
    "http://localhost:1455/auth/callback?state=test&code=synthetic-code";
  await userEvent.type(field, callback);
  await userEvent.click(screen.getByRole("button", { name: "提交回调" }));
  expect(request).toHaveBeenCalledWith(
    "codex_action",
    expect.objectContaining({
      action: "submit_callback",
      callbackUrl: callback,
    }),
  );
  expect(await screen.findByText(/回调已提交，等待代理/)).toBeTruthy();
  expect(screen.queryByLabelText("完整回调地址")).toBeNull();
  expect(screen.queryByText("已登录")).toBeNull();
});
test("background authorization failure remains visible after refresh", async () => {
  vi.mocked(request).mockResolvedValue({
    ...stopped,
    connectionError: "浏览器授权已返回，但代理换取登录令牌失败。",
  });
  render(<CodexView />);
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    expect.stringContaining("换取登录令牌失败"),
  );
  await userEvent.click(screen.getByRole("button", { name: "刷新状态" }));
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    expect.stringContaining("换取登录令牌失败"),
  );
});
test("connects selected model directly and refreshes workspace", async () => {
  vi.mocked(request).mockResolvedValue({
    ...stopped,
    running: true,
    account: "test@example.com",
    models: ["gpt-test"],
  });
  render(<CodexView />);
  await userEvent.click(
    await screen.findByRole("button", { name: "设为默认并接入 Droid" }),
  );
  expect(request).toHaveBeenCalledWith("codex_action", {
    action: "add_model",
    model: "gpt-test",
    revision: 7,
  });
  expect(reload).toHaveBeenCalled();
  expect(await screen.findByText(/已备份并写入 Droid 配置/)).toBeTruthy();
});
test("operation failures are visible and retry remains available", async () => {
  vi.mocked(request).mockImplementation(async (_, args) => {
    if (args?.action === "start") throw new Error("端口已被占用");
    return stopped;
  });
  render(<CodexView />);
  const start = await screen.findByRole("button", { name: "启动服务" });
  await vi.waitFor(() => expect(start.hasAttribute("disabled")).toBe(false));
  await userEvent.click(start);
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    expect.stringContaining("端口已被占用"),
  );
  expect(start.hasAttribute("disabled")).toBe(false);
});
