import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
const payload=JSON.parse(readFileSync(new URL("../fixtures/usage.json",import.meta.url),"utf8"));
const scheme={source:null,official:{model:null,reasoning:null},api:{modelId:null,reasoning:null}};
const workspace={revision:7,targetPath:"/test/settings.json",catalog:{providers:[],models:[]},appliedCatalog:{providers:[],models:[]},baseline:{document:{},fingerprint:null},bindings:[],scheme,appliedScheme:scheme};
test.beforeEach(async({page})=>{
  await page.addInitScript(({payload,workspace})=>{
    Object.defineProperty(window,"isTauri",{value:true});Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{invoke:async(command:string,args:any)=>{
      if(command==="load_workspace")return workspace;
      if(command==="load_usage")return payload;
      if(command==="latest_tests")return {};
      if(command==="save_price"){if(!args.price.model || typeof args.price.effectiveAt!=="number" || args.price.output!==null)throw Error("bad price contract");localStorage.setItem("saved-price",JSON.stringify(args.price));return null;}
      if(command==="save_usage_settings"){if(args.revision!==7)throw Error("missing revision");return {...workspace,revision:8};}
      if(command==="export_usage")return {csv:"\ufefftime,model\r\nnow,claude-test",rows:1};
      if(command==="scan_usage")return {};
      throw Error(`unmocked ${command}`);
    }}});
  },{payload,workspace});
});
for(const width of [1440,1024])test(`usage readable compact layout ${width}`,async({page})=>{
  await page.setViewportSize({width,height:900});await page.goto("/#/usage");
  await expect(page.getByText("每日趋势")).toBeVisible();await expect(page.getByRole("option",{name:"QA",exact:true})).toHaveCount(1);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth)).toBeLessThanOrEqual(width);
  await expect(page.getByText("Invalid Date")).toHaveCount(0);await expect(page.getByRole("dialog")).toHaveCount(0);
  for (const label of ["开始日期", "结束日期"]) { const box = await page.getByLabel(label).boundingBox(); expect(box?.width).toBeGreaterThanOrEqual(140); }
  await expect(page.getByLabel("来源", {exact:true})).not.toBeVisible();
  expect((await page.locator('.usage-filters').boundingBox())?.height).toBeLessThan(130);
  await page.screenshot({path:`/tmp/droid-usage-fixed-${width}.png`,fullPage:true});
  const row=page.locator('tbody tr[tabindex="0"]').first();await row.focus();await page.keyboard.press("Enter");await expect(page.getByRole("dialog",{name:"记录详情"})).toBeVisible();await page.keyboard.press("Escape");await expect(page.getByRole("dialog")).toHaveCount(0);await expect(row).toBeFocused();
});
test("usage saves actual price contract and downloads CSV",async({page})=>{
  await page.goto("/#/usage");await page.getByText("更多操作").click();await page.getByRole("button",{name:"定价管理"}).click();await page.getByLabel("普通输入",{exact:true}).fill("1.5");await page.getByRole("button",{name:"保存单价"}).click();await expect(page.getByRole("dialog")).toHaveCount(0);expect(JSON.parse((await page.evaluate(()=>localStorage.getItem("saved-price")))!)).toMatchObject({model:"claude-test",input:"1.5",output:null});
  const pending=page.waitForEvent("download");await page.getByRole("button",{name:"导出 CSV"}).click();const download=await pending;const stream=await download.createReadStream();const chunks=[];for await (const chunk of stream!)chunks.push(chunk);expect(Buffer.concat(chunks).toString()).toContain("now,claude-test");
});
