import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { CodexQuota } from "./CodexQuota";
import { request } from "../../lib/api";

vi.mock("../../lib/api", () => ({ request: vi.fn(), errorText: String }));
afterEach(() => { cleanup(); vi.resetAllMocks(); });
const quota = {
  windows: [
    { label: "使用额度 · 主要窗口", remainingPercent: 0, windowSeconds: 18000, resetAt: 2000000000 },
    { label: "使用额度 · 次要窗口", remainingPercent: null, windowSeconds: 604800, resetAt: null },
  ], allowed: false, limitReached: true, fetchedAt: 1900000000,
};
test("quota is explicitly refreshed and distinguishes zero from unknown", async () => {
  vi.mocked(request).mockResolvedValue(quota);
  render(<CodexQuota />);
  expect(request).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole("button", { name: "刷新额度" }));
  expect(await screen.findByText("0%")).toBeTruthy();
  expect(screen.getByText("5 小时")).toBeTruthy();
  expect(screen.getByText("7 天")).toBeTruthy();
  expect(screen.getAllByText("未知")).toHaveLength(2);
  expect(screen.getByRole("status").textContent).toContain("已达到使用限制");
  expect(request).toHaveBeenCalledWith("codex_quota");
});
test("failed refresh preserves but labels the previous result as stale", async () => {
  vi.mocked(request).mockResolvedValueOnce(quota).mockRejectedValueOnce("额度接口请求过于频繁");
  render(<CodexQuota />);
  await userEvent.click(screen.getByRole("button", { name: "刷新额度" }));
  await screen.findByText("0%");
  await userEvent.click(screen.getByRole("button", { name: "刷新额度" }));
  expect((await screen.findByRole("alert")).textContent).toContain("非最新额度");
  expect(screen.getByText("0%")).toBeTruthy();
});
test("first query failure does not fabricate a balance", async () => {
  vi.mocked(request).mockRejectedValue("额度查询授权失效");
  render(<CodexQuota />);
  await userEvent.click(screen.getByRole("button", { name: "刷新额度" }));
  expect((await screen.findByRole("alert")).textContent).toContain("剩余额度未知");
  expect(screen.queryByText("100%")).toBeNull();
});
