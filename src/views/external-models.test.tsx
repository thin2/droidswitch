import { afterEach, expect, test, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceProvider } from "../lib/workspace";
import { request } from "../lib/api";
import { ExternalModels } from "./ExternalModels";
import type { Workspace } from "../lib/types";
vi.mock("../lib/api", () => ({
  request: vi.fn(),
  errorText: (e: unknown) => String(e),
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const item = {
  model: "remote",
  displayName: "File model",
  apiKey: "secret-never-render",
  extraHeaders: { Authorization: "secret-header" },
  index: 0,
};
const workspace: Workspace = {
  revision: 5,
  targetPath: "/fixture/settings.json",
  catalog: {
    providers: [
      {
        id: "p",
        name: "QA",
        baseUrl: "https://example.test",
        protocol: "openai",
        credential: { mode: "direct", value: "secret-never-render" },
        modelListUrl: null,
        extraHeaders: {},
      },
    ],
    models: [
      {
        id: "m",
        providerId: "p",
        apiModelId: "remote",
        displayName: "Managed model",
        droidSelectorId: null,
        manufacturer: null,
        protocolOverride: null,
        enabled: true,
        maxOutputTokens: null,
        imageInput: false,
        extraArgs: {},
      },
    ],
  },
  baseline: { document: { customModels: [item] }, fingerprint: "fixture" },
  bindings: [{ modelId: "m", original: { ...item, apiKey: "old-secret" } }],
};
const conflict = {
  revision: 5,
  bindings: [
    {
      modelId: "m",
      status: "changed_or_missing",
      matches: [],
      candidates: [
        { index: 0, fields: ["apiKey", "noImageSupport", "extraHeaders"] },
      ],
    },
  ],
  external: [{ index: 0, existingModelIds: ["m"], selectable: true }],
};
function mockApi(report: unknown, w = workspace) {
  vi.mocked(request).mockImplementation(async (command) =>
    command === "inspect_configuration"
      ? report
      : command === "load_workspace"
        ? w
        : { ...w, revision: w.revision + 1 },
  );
}
function mount() {
  return render(
    <WorkspaceProvider>
      <ExternalModels />
    </WorkspaceProvider>,
  );
}
test("uses backend metadata matching rather than JSON text equality", async () => {
  mockApi({
    revision: 5,
    bindings: [
      { modelId: "m", status: "matched", matches: [0], candidates: [] },
    ],
    external: [],
  });
  mount();
  await screen.findByText("没有未接管的模型。");
  expect(screen.queryByRole("button", { name: "选择恢复方式" })).toBeNull();
  expect(screen.queryByRole("button", { name: "接管" })).toBeNull();
  expect(request).toHaveBeenCalledWith("inspect_configuration", {
    revision: 5,
  });
});
test("shows only changed field labels and requires explicit target before app recovery", async () => {
  mockApi(conflict);
  const user = userEvent.setup();
  const { container } = mount();
  await screen.findByText(/变化字段：API Key/);
  expect(container.textContent).not.toContain("secret");
  await user.click(screen.getByRole("button", { name: "选择恢复方式" }));
  const dialog = screen.getByRole("dialog", { name: "恢复接管关系" });
  expect(
    (
      within(dialog).getByRole("button", {
        name: "确认恢复",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  await user.selectOptions(
    within(dialog).getByLabelText("对应的磁盘条目"),
    "0",
  );
  await user.selectOptions(within(dialog).getByLabelText("恢复方式"), "app");
  await user.click(within(dialog).getByRole("checkbox"));
  await user.click(within(dialog).getByRole("button", { name: "确认恢复" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith("resolve_binding", {
      id: "m",
      index: 0,
      resolution: "app",
      allowPlaintext: true,
      revision: 5,
    }),
  );
  expect(
    vi.mocked(request).mock.calls.some(([c]) => c === "apply_configuration"),
  ).toBe(false);
});
test("file recovery is explicit and cancel does not change workspace", async () => {
  mockApi(conflict);
  const user = userEvent.setup();
  mount();
  await user.click(await screen.findByRole("button", { name: "选择恢复方式" }));
  await user.click(screen.getByRole("button", { name: "取消" }));
  expect(
    vi.mocked(request).mock.calls.some(([c]) => c === "resolve_binding"),
  ).toBe(false);
  await user.click(screen.getByRole("button", { name: "选择恢复方式" }));
  await user.selectOptions(screen.getByLabelText("对应的磁盘条目"), "0");
  await user.click(screen.getByRole("button", { name: "确认恢复" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      "resolve_binding",
      expect.objectContaining({ resolution: "file", allowPlaintext: false }),
    ),
  );
});
test("cleared bindings require explicit association and keep existing model ID", async () => {
  mockApi({ ...conflict, bindings: [] }, { ...workspace, bindings: [] });
  const user = userEvent.setup();
  mount();
  await user.click(await screen.findByRole("button", { name: "接管" }));
  expect(
    (screen.getByRole("button", { name: "确认接管" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  await user.selectOptions(screen.getByLabelText("关联已有模型"), "m");
  await user.click(screen.getByRole("checkbox"));
  await user.click(screen.getByRole("button", { name: "确认接管" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith("takeover_model", {
      index: 0,
      existingModelId: "m",
      createSeparate: false,
      allowPlaintext: true,
      revision: 5,
    }),
  );
});
test("duplicate disk entries cannot be adopted or selected for recovery", async () => {
  mockApi({
    ...conflict,
    external: [{ index: 0, existingModelIds: [], selectable: false }],
  });
  mount();
  await screen.findByText(/条目无效或重复/);
  expect(
    (
      screen.getByRole("button", {
        name: "接管",
      }) as HTMLButtonElement
    ).disabled,
  ).toBe(true);
  expect(
    (screen.getByRole("button", { name: "选择恢复方式" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
});
test("failed recovery keeps the choice visible and allows reloading", async () => {
  mockApi(conflict);
  const implementation = vi.mocked(request).getMockImplementation()!;
  vi.mocked(request).mockImplementation(async (command, args) => {
    if (command === "resolve_binding")
      throw new Error("外部配置已变化，请重新载入");
    return implementation(command, args);
  });
  const user = userEvent.setup();
  mount();
  await user.click(await screen.findByRole("button", { name: "选择恢复方式" }));
  await user.selectOptions(screen.getByLabelText("对应的磁盘条目"), "0");
  await user.click(screen.getByRole("button", { name: "确认恢复" }));
  expect(screen.getByRole("dialog", { name: "恢复接管关系" })).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "取消" }));
  await user.click(screen.getByRole("button", { name: "重新载入当前配置" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith("reload_configuration", {
      revision: 5,
    }),
  );
});
