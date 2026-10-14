import { test, expect } from "@playwright/test";
test("configuration recovery presents field-only diagnostics and explicit choices", async ({ page }) => {
  await page.addInitScript(() => {
    const item = { model: "shared-model", displayName: "测试模型 · QA", apiKey: "never-display-this-key", noImageSupport: false };
    const workspace = { revision: 8, targetPath: "/test/settings.json", catalog: {
      providers: [{ id: "p", name: "QA", baseUrl: "https://example.test", protocol: "openai", credential: { mode: "direct", value: "never-display-this-key" }, modelListUrl: null, extraHeaders: {} }],
      models: [{ id: "m", providerId: "p", apiModelId: "shared-model", displayName: "测试模型 · QA", enabled: true, imageInput: false, maxOutputTokens: null, protocolOverride: null, droidSelectorId: null, manufacturer: null, extraArgs: {} }],
    }, baseline: { document: { customModels: [item] }, fingerprint: null }, bindings: [{ modelId: "m", original: { ...item, noImageSupport: true } }] };
    Object.defineProperty(window, "isTauri", { value: true });
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke: async (command: string, args: unknown) => {
      if (command === "load_workspace") return workspace;
      if (command === "latest_tests") return {};
      if (command === "inspect_configuration") return { revision: 8, bindings: [{ modelId: "m", status: "changed_or_missing", matches: [], candidates: [{ index: 0, fields: ["apiKey", "noImageSupport"] }] }], external: [{ index: 0, existingModelIds: ["m"], selectable: true }] };
      if (command === "resolve_binding") { localStorage.setItem("resolved", JSON.stringify(args)); return { ...workspace, revision: 9 }; }
      throw new Error("测试环境未提供此命令");
    } } });
  });
  await page.setViewportSize({ width: 1024, height: 900 });
  await page.goto("/#/settings");
  await expect(page.getByText(/变化字段：API Key/)).toBeVisible();
  await expect(page.locator("body")).not.toContainText("never-display-this-key");
  await page.getByRole("button", { name: "选择恢复方式" }).click();
  const dialog = page.getByRole("dialog", { name: "恢复接管关系" });
  await expect(dialog.getByRole("button", { name: "确认恢复" })).toBeDisabled();
  await dialog.getByLabel("对应的磁盘条目").selectOption("0");
  await dialog.getByLabel("恢复方式").selectOption("app");
  await dialog.getByRole("checkbox").check();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(1024);
  await page.screenshot({ path: "/tmp/droidswitch-recovery.png", fullPage: true });
  await dialog.getByRole("button", { name: "确认恢复" }).click();
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem("resolved") || "null"))).toEqual({ id: "m", index: 0, resolution: "app", allowPlaintext: true, revision: 8 });
});
