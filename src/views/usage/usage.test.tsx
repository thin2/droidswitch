import { expect, test } from "vitest";
import { csv, costs, total, type UsageEvent } from "./data";
const event: UsageEvent = {
  id: "event",
  session: "session",
  occurredAt: 1,
  source: "api",
  provider: '=HYPERLINK("bad")',
  model: "model",
  input: null,
  cacheRead: 0,
  cacheWrite: null,
  output: 12,
  credits: null,
  cost: "0.000000000001",
  currency: "USD",
  kind: "回合汇总",
};
test("CSV escapes formulas and preserves unknown metadata", () => {
  const output = csv([event]);
  expect(output).toContain("'=HYPERLINK");
  expect(output).toContain("未知");
  expect(output).not.toContain("apiKey");
});
test("usage null is never zero, and currencies remain separate with exact sums", () => {
  expect(total([event], "input")).toBe("未知");
  expect(total([event], "cacheRead")).toBe("0");
  expect(costs([event, event, { ...event, currency: "CNY", cost: "1" }])).toBe(
    "USD 0.000000000002 · CNY 1",
  );
});

import { afterEach, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceProvider } from "../../lib/workspace";
import { UsageView } from "./UsageView";
import { request } from "../../lib/api";
vi.mock("../../lib/api", () => ({
  request: vi.fn(),
  errorText: (e: unknown) => String(e),
}));
afterEach(cleanup);
test("empty usage gives collection settings rather than fabricated zero totals", async () => {
  vi.mocked(request).mockImplementation(async (command) =>
    command === "load_usage"
      ? { events: [], report: null, truncated: false }
      : {
          revision: 0,
          targetPath: "/test",
          catalog: { providers: [], models: [] },
        },
  );
  render(
    <WorkspaceProvider>
      <UsageView />
    </WorkspaceProvider>,
  );
  await screen.findByText("当前范围暂无可用用量");
  await userEvent.click(screen.getByRole("button", { name: "配置采集目录" }));
  expect(screen.getByRole("dialog", { name: "采集设置" })).toBeTruthy();
  expect(
    vi.mocked(request).mock.calls.some(([command]) => command === "scan_usage"),
  ).toBe(false);
});
