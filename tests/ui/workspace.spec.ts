import { test, expect } from "@playwright/test";
const workspace = {
  revision: 1,
  targetPath: "/test/settings.json",
  catalog: {
    providers: [
      {
        id: "p",
        name: "测试供应商",
        baseUrl: "https://example.com/v1",
        protocol: "openai",
        credential: { mode: "environment", value: "TEST_KEY" },
        modelListUrl: null,
        extraHeaders: {},
      },
    ],
    models: [
      {
        id: "m",
        providerId: "p",
        apiModelId: "test/model-v1",
        droidSelectorId: null,
        displayName: "测试模型",
        manufacturer: "Test",
        protocolOverride: null,
        enabled: true,
        maxOutputTokens: null,
        imageInput: true,
        extraArgs: {},
      },
    ],
  },
  appliedCatalog: { providers: [], models: [] },
  baseline: { document: {}, fingerprint: null },
  bindings: [],
  scheme: {
    source: null,
    official: { model: null, reasoning: null },
    api: { modelId: null, reasoning: null },
  },
  appliedScheme: {
    source: null,
    official: { model: null, reasoning: null },
    api: { modelId: null, reasoning: null },
  },
};
test.beforeEach(async ({ page }) => {
  await page.addInitScript((initial) => {
    Object.defineProperty(window, "isTauri", { value: true });
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {
        invoke: async (command: string) => {
          if (command === "load_workspace") return initial;
          if (command === "latest_tests") return {};
          if (command === "load_usage")
            return { events: [], report: null, truncated: false };
          if (command === "list_backups") return [];
          throw new Error("测试环境中未允许此命令");
        },
      },
    });
  }, workspace);
});
for (const width of [1440, 1024])
  test(`workspace layout ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: width === 1024 ? 720 : 960 });
    await page.goto("/");
    await page
      .getByRole("button", { name: /测试模型.*test\/model-v1/ })
      .click();
    await expect(page.getByLabel("API 模型 ID")).toBeVisible();
    await expect(page.getByRole("button", { name: "保存草稿" })).toBeVisible();
    expect(
      await page.evaluate(() => document.documentElement.scrollWidth),
    ).toBeLessThanOrEqual(width);
    await page.screenshot({
      path: `/tmp/droid-switch-ui-${width}.png`,
      fullPage: true,
    });
  });
test("modal keyboard containment and official unknown login", async ({
  page,
}) => {
  await page.goto("/");
  await page.getByRole("button", { name: "添加模型", exact: true }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  const first = dialog.locator("select").first();
  await expect(first).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(dialog.getByRole("button", { name: "保存草稿" })).toBeFocused();
  await dialog.getByRole("button", { name: "取消" }).click();
  await page.getByRole("button", { name: "官方订阅", exact: true }).click();
  await expect(page.getByText(/登录状态：状态待确认/)).toBeVisible();
});
