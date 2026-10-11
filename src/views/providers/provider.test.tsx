import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceProvider } from "../../lib/workspace";
import { ProviderView } from "./ProviderView";
import { request } from "../../lib/api";
vi.mock("../../lib/api", () => ({
  request: vi.fn(),
  errorText: (e: unknown) => String(e),
}));
afterEach(cleanup);
test("provider direct credentials require explicit consent; save stays a draft", async () => {
  const mock = vi.mocked(request);
  mock.mockResolvedValue({
    revision: 0,
    targetPath: "/test/settings.json",
    catalog: { providers: [], models: [] },
  });
  const user = userEvent.setup();
  render(
    <WorkspaceProvider>
      <ProviderView />
    </WorkspaceProvider>,
  );
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: "＋ 添加供应商" })
        .hasAttribute("disabled"),
    ).toBe(false),
  );
  await user.click(screen.getByRole("button", { name: "＋ 添加供应商" }));
  await user.type(screen.getByLabelText("连接名称"), "Example");
  await user.type(
    screen.getByLabelText(/API 基础地址/),
    "https://example.com/custom",
  );
  await user.selectOptions(screen.getByLabelText("凭据方式"), "direct");
  await user.type(screen.getByLabelText("API Key"), "test-secret");
  expect(
    screen.getByRole("button", { name: "保存草稿" }).hasAttribute("disabled"),
  ).toBe(true);
  await user.click(screen.getByRole("checkbox"));
  await user.click(screen.getByRole("button", { name: "保存草稿" }));
  await waitFor(() =>
    expect(mock).toHaveBeenCalledWith(
      "save_provider",
      expect.objectContaining({
        allowPlaintext: true,
        revision: 0,
        provider: expect.objectContaining({
          baseUrl: "https://example.com/custom",
          credential: { mode: "direct", value: "test-secret" },
        }),
      }),
    ),
  );
  expect(
    mock.mock.calls.some(([command]) => command === "apply_configuration"),
  ).toBe(false);
});
