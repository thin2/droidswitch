import { test, expect } from "@playwright/test";
test("supplier naming suggestions are reviewable before changing the draft", async ({ page }) => {
  await page.addInitScript(() => {
    let adopted = false;
    const suggestions = [{ modelId: "a", providerName: "QA", before: "claude-opus-5-5", after: "claude-opus-5-5 (QA)" }, { modelId: "b", providerName: "zzshu", before: "claude-opus-5-5", after: "claude-opus-5-5 (zzshu)" }];
    const workspace = { revision: 1, targetPath: "/test/settings.json", catalog: { providers: [], models: [] }, bindings: [] };
    Object.defineProperty(window, "isTauri", { value: true });
    Object.defineProperty(window, "__TAURI_INTERNALS__", { value: { invoke: async (command: string, args: unknown) => {
      if (command === "load_workspace") return workspace;
      if (command === "latest_tests") return {};
      if (command === "suggest_model_names") return adopted ? [] : suggestions;
      if (command === "apply_model_names") { adopted = true; localStorage.setItem("naming", JSON.stringify(args)); return { ...workspace, revision: 2 }; }
      throw new Error("未允许的测试命令");
    } } });
  });
  await page.setViewportSize({ width: 1024, height: 800 });
  await page.goto("/#/providers");
  await page.getByRole("button", { name: "查看命名建议" }).click();
  const dialog = page.getByRole("dialog", { name: "确认模型显示名称" });
  await expect(dialog.getByText("claude-opus-5-5 (QA)", { exact: true })).toBeVisible();
  await expect(dialog.getByText("claude-opus-5-5 (zzshu)", { exact: true })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(1024);
  await page.screenshot({ path: "/tmp/droidswitch-naming.png" });
  await dialog.getByRole("button", { name: "采用建议并保存草稿" }).click();
  await expect(page.getByRole("button", { name: "查看命名建议" })).toHaveCount(0);
  expect(await page.evaluate(() => JSON.parse(localStorage.getItem("naming") || "null"))).toEqual({ revision: 1 });
});
