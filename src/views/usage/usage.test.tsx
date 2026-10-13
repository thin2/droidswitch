import { expect, test } from "vitest";
import { csv, formatCost, formatTokens, type RecordRow } from "./data";

const record: RecordRow = {
  id: "event",
  session: "session",
  occurredAt: 1,
  granularity: "request",
  timeConfidence: "exact",
  source: "api",
  provider: "test",
  providerLabel: '=HYPERLINK("bad")',
  selector: "selector",
  apiModel: "model",
  input: null,
  cacheRead: 0,
  cacheWrite: null,
  output: 12,
  thinking: 0,
  requestId: "req",
  attempt: 1,
  cost: "0.000000000001",
  currency: "USD",
  complete: true,
};

test("CSV escapes formulas and preserves unknown metadata", () => {
  const output = csv([record]);
  expect(output).toContain("'=HYPERLINK");
  expect(output).toContain("未知");
  expect(output).not.toContain("apiKey");
});

test("usage null is never zero, and currencies remain separate and tiny costs never display as zero", () => {
  expect(formatTokens(0, 1)).toBe("未知");
  expect(formatTokens(100, 0)).toBe("100");
  expect(formatTokens(100, 1)).toBe("100（部分缺失）");
  expect(
    formatCost([
      { currency: "USD", cost: "0.000000000001" },
      { currency: "USD", cost: "0.000000000001" },
      { currency: "CNY", cost: "1" },
    ]),
  ).toBe("USD <0.0001 · USD <0.0001 · CNY 1.00");
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
      ? {
          settings: {
            path: "~/.factory/sessions",
            logsPath: "~/.factory/logs",
            exportsPath: "",
            retentionDays: 90,
            autoScan: false,
          },
          notice: null,
          report: null,
          page: {
            summary: {
              tokens: {
                input: 0,
                cacheRead: 0,
                cacheWrite: 0,
                output: 0,
                thinking: 0,
                unknownInput: 0,
                unknownCacheRead: 0,
                unknownCacheWrite: 0,
                unknownOutput: 0,
                unknownThinking: 0,
              },
              records: 0,
              sessions: 0,
              estimates: [],
              pricedRecords: 0,
              billableRecords: 0,
              partialRecords: 0,
              unknownRecords: 0,
              credits: 0,
              creditsRecords: 0,
              unknownSourceRecords: 0,
            },
            trends: [],
            unassigned: [],
            unassignedTokens: {
              input: 0,
              cacheRead: 0,
              cacheWrite: 0,
              output: 0,
              thinking: 0,
              unknownInput: 0,
              unknownCacheRead: 0,
              unknownCacheWrite: 0,
              unknownOutput: 0,
              unknownThinking: 0,
            },
            providers: [],
            models: [],
            records: [],
            totalRecords: 0,
            coverage: {
              providers: 0,
              models: 0,
              earliest: null,
              latest: null,
              windowFrom: null,
              windowTo: null,
              endpointResolvedRecords: 0,
              endpointUnresolvedRecords: 0,
            },
            prices: [],
            providersFilter: [],
            modelsFilter: [],
          },
          scanning: false,
        }
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

import * as usageData from "./data";
import { waitFor } from "@testing-library/react";

function connectedPayload() {
  const tokens = {
    input: 100,
    cacheRead: 0,
    cacheWrite: 0,
    output: 10,
    thinking: 0,
    unknownInput: 0,
    unknownCacheRead: 0,
    unknownCacheWrite: 0,
    unknownOutput: 0,
    unknownThinking: 0,
  };
  return {
    settings: {
      path: "/sessions",
      logsPath: "/logs",
      exportsPath: "",
      retentionDays: 90,
      autoScan: true,
    },
    notice: null,
    report: null,
    scanning: false,
    page: {
      summary: {
        tokens,
        records: 1,
        sessions: 1,
        estimates: [],
        pricedRecords: 0,
        billableRecords: 1,
        partialRecords: 0,
        unknownRecords: 0,
        credits: 0,
        creditsRecords: 0,
        unknownSourceRecords: 0,
      },
      trends: [
        {
          start: Math.floor(Date.now() / 1000),
          tokens,
          records: 1,
          estimates: [],
        },
      ],
      unassigned: [],
      unassignedTokens: { ...tokens, input: 0, output: 0 },
      providers: [],
      models: [],
      records: [
        {
          ...record,
          providerLabel: "QA",
          input: 100,
          output: 10,
          complete: false,
          cost: null,
        },
      ],
      totalRecords: 1,
      coverage: {},
      prices: [],
      providersFilter: ["qa-id"],
      providerLabels: { "qa-id": "QA" },
      modelsFilter: ["model"],
    },
  };
}
function connectedMock() {
  vi.mocked(request).mockReset();
  vi.mocked(request).mockImplementation(async (command, args) => {
    if (command === "load_usage") return connectedPayload();
    if (command === "load_workspace")
      return {
        revision: 7,
        targetPath: "/test",
        catalog: { providers: [], models: [] },
      };
    if (command === "export_usage")
      return { csv: "\ufeff时间,模型\r\n今天,model", rows: 1 };
    if (command === "save_usage_settings") {
      expect(args?.revision).toBe(7);
      return {
        revision: 8,
        targetPath: "/test",
        catalog: { providers: [], models: [] },
      };
    }
    if (command === "save_price") {
      const price = args?.price as Record<string, unknown>;
      expect(price).toMatchObject({
        providerId: "qa-id",
        model: "model",
        input: "1.5",
        cacheRead: null,
        cacheWrite: null,
        output: null,
        thinking: null,
        currency: "USD",
      });
      expect(typeof price.effectiveAt).toBe("number");
      expect(price).not.toHaveProperty("effectiveFrom");
      return null;
    }
    throw Error(`unexpected command ${command}`);
  });
}
test("real IPC-shaped response renders trends and exports CSV contents", async () => {
  connectedMock();
  const download = vi.spyOn(usageData, "download").mockImplementation(() => {});
  render(
    <WorkspaceProvider>
      <UsageView />
    </WorkspaceProvider>,
  );
  await screen.findByText("每日趋势");
  expect(screen.queryByText(/Invalid Date/)).toBeNull();
  await userEvent.click(screen.getByText("更多操作"));
  await userEvent.click(screen.getByRole("button", { name: "导出 CSV" }));
  expect(download).toHaveBeenCalledWith(
    "\ufeff时间,模型\r\n今天,model",
    "droid-usage.csv",
    "text/csv;charset=utf-8",
  );
  download.mockRestore();
});
test("settings send workspace revision and prices preserve missing rates", async () => {
  connectedMock();
  render(
    <WorkspaceProvider>
      <UsageView />
    </WorkspaceProvider>,
  );
  await screen.findByText("每日趋势");
  await userEvent.click(screen.getByText("更多操作"));
  await userEvent.click(screen.getByRole("button", { name: "采集设置" }));
  await userEvent.click(screen.getByRole("button", { name: "保存设置" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "采集设置" })).toBeNull(),
  );
  await userEvent.click(screen.getByRole("button", { name: "定价管理" }));
  await userEvent.type(screen.getByLabelText("普通输入"), "1.5");
  await userEvent.click(screen.getByRole("button", { name: "保存单价" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog", { name: "单价设置" })).toBeNull(),
  );
  expect(
    vi.mocked(request).mock.calls.some(([command]) => command === "save_price"),
  ).toBe(true);
});
test("record details close with Escape and return focus", async () => {
  connectedMock();
  render(
    <WorkspaceProvider>
      <UsageView />
    </WorkspaceProvider>,
  );
  await screen.findByText("每日趋势");
  const row = screen.getByText("QA", { selector: "td" }).closest("tr")!;
  row.focus();
  await userEvent.keyboard("{Enter}");
  expect(screen.getByRole("dialog", { name: "记录详情" })).toBeTruthy();
  await userEvent.keyboard("{Escape}");
  expect(screen.queryByRole("dialog", { name: "记录详情" })).toBeNull();
  expect(document.activeElement).toBe(row);
});
