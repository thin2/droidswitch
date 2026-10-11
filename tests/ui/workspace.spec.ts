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
        invoke: async (
          command: string,
          args: { providerId?: string; ids?: string[] },
        ) => {
          if (command === "import_models") {
            localStorage.setItem("test-import", JSON.stringify(args));
            return { ...initial, revision: initial.revision + 1 };
          }
          if (command === "load_workspace") return initial;
          if (command === "latest_tests") return {};
          if (command === "fetch_models")
            return {
              ids: [
                "test/model-v1",
                "example/fast-model",
                "example/reasoning-model",
              ],
              elapsedMs: 1092,
              incomplete: false,
            };
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

test("remote model toolbar spacing and filtered selection", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1024, height: 800 });
  await page.goto("/");
  await page.getByRole("button", { name: "获取模型", exact: true }).click();
  await page.getByRole("button", { name: "获取列表", exact: true }).click();
  const dialog = page.getByRole("dialog");
  const search = dialog.getByRole("textbox", { name: "筛选远程模型" });
  const select = dialog.getByRole("button", { name: "全选结果" });
  await expect(select).toBeVisible();
  const a = (await search.boundingBox())!;
  const b = (await select.boundingBox())!;
  expect(b.x - (a.x + a.width)).toBeGreaterThanOrEqual(7);
  expect(a.height).toBe(b.height);
  await search.fill("fast");
  await select.click();
  await expect(dialog.getByRole("status")).toContainText("已选 1 项");
  await expect(select).toBeDisabled();
  await search.fill("");
  await page.screenshot({ path: "/tmp/droid-switch-fetch-toolbar.png" });
});

test("supplier actions scope fetching, batch deduplication and model filter", async ({
  page,
}) => {
  await page.goto("/#/providers");
  await page.getByRole("button", { name: "获取模型", exact: true }).click();
  let dialog = page.getByRole("dialog");
  await expect(dialog.getByRole("combobox", { name: "供应商", exact: true })).toHaveValue("p");
  await expect(dialog.getByRole("combobox", { name: "供应商", exact: true })).toBeDisabled();
  await dialog.getByRole("button", { name: "手动添加模型" }).click();
  dialog = page.getByRole("dialog");
  await expect(
    dialog.getByRole("button", { name: "添加 0 个模型" }),
  ).toBeDisabled();
  await dialog
    .getByLabel("模型 ID（每行一个）")
    .fill("test/model-v1\n new/model \nnew/model\n\nother/model");
  await expect(dialog.getByRole("status")).toHaveText(
    "待添加 2 项 · 已存在 1 项 · 重复输入 1 项",
  );
  await page.screenshot({ path: "/tmp/droid-switch-batch-models.png" });
  await dialog.getByRole("button", { name: "添加 2 个模型" }).click();
  await expect(dialog).not.toBeVisible();
  const args = await page.evaluate(() =>
    JSON.parse(localStorage.getItem("test-import")!),
  );
  expect(args.providerId).toBe("p");
  expect(args.ids).toEqual(["new/model", "other/model"]);
  await page.getByRole("link", { name: "查看模型（1）" }).click();
  await expect(page.getByLabel("筛选供应商")).toHaveValue("p");
  await expect(page.locator(".supplier-badge")).toHaveText("测试供应商");
  await page.reload();
  await expect(page.getByLabel("筛选供应商")).toHaveValue("p");
  await page.getByLabel("筛选供应商").selectOption("");
  await expect(page).toHaveURL(/#\/models$/);
});
