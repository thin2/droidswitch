import { afterEach, expect, test, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorkspaceProvider } from "../../lib/workspace";
import { request } from "../../lib/api";
import { ImportWizard } from "./ImportWizard";
vi.mock("../../lib/api", () => ({ request: vi.fn(), errorText: String }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
for (const hasCredential of [true, false]) {
  test(`import defaults to existing key when available: ${hasCredential}`, async () => {
    const mock = vi.mocked(request);
    const workspace = { revision: 2, catalog: { providers: [], models: [] } };
    mock.mockImplementation(async (command) =>
      command === "preview_ccswitch"
        ? [
            {
              key: "source",
              hash: "hash",
              app: "claude",
              name: "Example",
              baseUrl: "https://example.com",
              protocol: "anthropic",
              models: [],
              hasCredential,
              issue: null,
              duplicate: "same",
            },
          ]
        : workspace,
    );
    const user = userEvent.setup();
    render(
      <WorkspaceProvider>
        <ImportWizard onClose={() => {}} />
      </WorkspaceProvider>,
    );
    await user.click(screen.getByRole("button", { name: "读取供应商" }));
    await user.selectOptions(
      await screen.findByLabelText("处理方式"),
      "update",
    );
    const mode = screen.getByLabelText("凭据保存") as HTMLSelectElement;
    expect(mode.value).toBe(hasCredential ? "direct" : "environment");
    const confirm = screen.getByRole("button", { name: /确认导入/ });
    expect(mock.mock.calls.some(([c]) => c === "import_ccswitch")).toBe(false);
    if (!hasCredential) {
      expect((confirm as HTMLButtonElement).disabled).toBe(true);
      expect(
        (screen.getByLabelText(/^环境变量名/) as HTMLInputElement).value,
      ).toBe("");
      await user.type(screen.getByLabelText(/^环境变量名/), "MY_KEY");
    }
    await user.click(confirm);
    await waitFor(() =>
      expect(mock).toHaveBeenCalledWith(
        "import_ccswitch",
        expect.objectContaining({
          allowPlaintext: hasCredential,
          choices: [
            expect.objectContaining({
              action: "update",
              credentialMode: hasCredential ? "direct" : "environment",
            }),
          ],
        }),
      ),
    );
  });
}
