import { test, expect } from "@playwright/test";
for (const width of [1024, 1440]) {
  test(`codex authorization layout ${width}`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.addInitScript(() => {
      Object.defineProperty(window, "isTauri", { value: true });
      const scheme = { source: null, official: { model: null, reasoning: null }, api: { modelId: null, reasoning: null } };
      Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke: async (command: string) => {
        if (command === "load_workspace") return { revision: 1, catalog: { providers: [], models: [] }, appliedCatalog: { providers: [], models: [] }, scheme, appliedScheme: scheme, bindings: [], targetPath: "/test/settings.json" };
        if (command === "codex_action") return { running: true, savedLogin: true, loginPending: false, account: "example@chatgpt.test", models: ["gpt-example", "gpt-example-mini"], endpoint: "http://127.0.0.1:18317/v1", version: "CLIProxyAPI 8.0.27" };
        throw new Error("Unsupported mock command");
      } } });
    });
    await page.goto("/#/codex");
    await expect(page.getByText("example@chatgpt.test", { exact: true })).toBeVisible();
    await expect(page.getByRole("button", { name: "设为默认并接入 Droid" })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
    await page.screenshot({ path: `/tmp/droidswitch-codex-${width}.png`, fullPage: true });
  });
}
