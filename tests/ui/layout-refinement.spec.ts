import { test, expect } from "@playwright/test";
for (const width of [1024, 1440]) test(`model columns and compact non-overlay controls ${width}`, async ({ page }) => {
  await page.addInitScript(() => {
    const names = ["claude-haiku-4-5", "claude-haiku-4-5-20251001", "claude-opus-4-6", "claude-opus-5-5 (qa)", "claude-opus-5-5 (zzshu)"];
    const providers = ["qa", "zzshu"].map(id => ({ id, name: id, baseUrl: "https://example.test", protocol: "anthropic", credential: { mode: "environment", value: "TEST_KEY" }, modelListUrl: null, extraHeaders: {} }));
    const models = names.map((name, i) => ({ id: String(i), providerId: i === 4 ? "zzshu" : "qa", apiModelId: name.replace(/ \(.+\)$/, ""), displayName: name, droidSelectorId: null, manufacturer: null, protocolOverride: null, enabled: i % 2 === 1, maxOutputTokens: null, imageInput: true, extraArgs: {} }));
    const catalog = { providers, models };
    const workspace = { revision: 1, targetPath: "/test/settings.json", catalog, appliedCatalog: catalog, baseline: { document: {}, fingerprint: null }, bindings: [] };
    Object.defineProperty(window, "isTauri", { value: true });
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke: async (command: string, args: { model?: typeof models[number] }) => {
      if (command === "load_workspace") return workspace;
      if (command === "latest_tests") return {};
      if (command === "save_model") { localStorage.setItem("saved-model", JSON.stringify(args.model)); return { ...workspace, revision: 2 }; }
      if (command === "suggest_model_names") return [{ modelId: "a", providerName: "qa", before: "shared", after: "shared (qa)" }, { modelId: "b", providerName: "zzshu", before: "shared", after: "shared (zzshu)" }];
      throw new Error("未允许的测试命令");
    } } });
  });
  await page.setViewportSize({ width, height: 960 });
  await page.goto("/#/models");
  await expect(page.locator(".model-row")).toHaveCount(5);
  await expect(page.getByRole("button", { name: "查看命名建议" })).toBeVisible();
  for (const column of [".model-supplier", ".model-status"]) {
    const positions = await page.locator(column).evaluateAll(nodes => nodes.map(n => n.getBoundingClientRect().left));
    expect(Math.max(...positions) - Math.min(...positions)).toBeLessThan(1);
  }
  const notice = await page.locator(".name-suggestion-notice").boundingBox();
  expect(notice!.height).toBeLessThan(100);
  expect(await page.locator(".change-bar").evaluate(el => getComputedStyle(el).position)).toBe("static");
  await page.locator(".change-bar").scrollIntoViewIfNeeded();
  const bar = await page.locator(".change-bar").boundingBox();
  const list = await page.locator(".model-layout").boundingBox();
  expect(bar!.y).toBeGreaterThanOrEqual(list!.y + list!.height);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
  await page.screenshot({ path: `/tmp/droidswitch-layout-${width}.png`, fullPage: true });
  const fullList = await page.locator(".model-list").boundingBox();
  const layout = await page.locator(".model-layout").boundingBox();
  expect(fullList!.width).toBeCloseTo(layout!.width, 0);
  await expect(page.locator(".detail-panel")).toHaveCount(0);
  await page.locator(".model-row").first().click();
  const drawer = page.getByRole("dialog", { name: "模型详情", exact: true });
  await expect(drawer).toBeVisible();
  await expect(drawer.getByRole("button", { name: "保存草稿" })).toBeVisible();
  await expect(drawer.getByRole("button", { name: "停用", exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => document.body.style.overflow)).toBe("hidden");
  await page.screenshot({ path: `/tmp/droidswitch-drawer-${width}.png` });
  await page.keyboard.press("Escape");
  await expect(drawer).toHaveCount(0);
  expect(await page.evaluate(() => document.body.style.overflow)).not.toBe("hidden");
  await page.locator(".model-row").first().click();
  await drawer.getByLabel("显示名称", { exact: true }).fill("Edited display name");
  await drawer.getByRole("button", { name: "保存草稿" }).click();
  await expect(drawer).toHaveCount(0);
  const saved = await page.evaluate(() => JSON.parse(localStorage.getItem("saved-model") || "null"));
  expect(saved.displayName).toBe("Edited display name");
  expect(saved.apiModelId).toBe("claude-haiku-4-5");

});
