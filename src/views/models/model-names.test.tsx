import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { request } from "../../lib/api";
import { WorkspaceProvider } from "../../lib/workspace";
import { ModelNameSuggestions } from "./ModelNameSuggestions";
vi.mock("../../lib/api", () => ({
  request: vi.fn(),
  errorText: (e: unknown) => String(e),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const w = { revision: 3, catalog: { providers: [], models: [] } };
const suggestions = [
  { modelId: "a", providerName: "QA", before: "shared", after: "shared (QA)" },
  {
    modelId: "b",
    providerName: "zzshu",
    before: "shared",
    after: "shared (zzshu)",
  },
];
test("suggests both supplier names and saves only after confirmation", async () => {
  vi.mocked(request).mockImplementation(async (command) =>
    command === "suggest_model_names" ? suggestions : w,
  );
  const user = userEvent.setup();
  render(
    <WorkspaceProvider>
      <ModelNameSuggestions />
    </WorkspaceProvider>,
  );
  await user.click(await screen.findByRole("button", { name: "查看命名建议" }));
  expect(screen.getByText("shared (QA)")).toBeTruthy();
  expect(screen.getByText("shared (zzshu)")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "取消" }));
  expect(
    vi.mocked(request).mock.calls.some(([c]) => c === "apply_model_names"),
  ).toBe(false);
  await user.click(screen.getByRole("button", { name: "查看命名建议" }));
  await user.click(screen.getByRole("button", { name: "采用建议并保存草稿" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith("apply_model_names", { revision: 3 }),
  );
  expect(
    vi.mocked(request).mock.calls.some(([c]) => c === "apply_configuration"),
  ).toBe(false);
});
test("does not show a naming prompt for unique names", async () => {
  vi.mocked(request).mockImplementation(async (command) =>
    command === "suggest_model_names" ? [] : w,
  );
  render(
    <WorkspaceProvider>
      <ModelNameSuggestions />
    </WorkspaceProvider>,
  );
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith("suggest_model_names", {
      revision: 3,
    }),
  );
  expect(screen.queryByRole("button", { name: "查看命名建议" })).toBeNull();
});
