import { expect, test, type Page } from "./fixtures.ts";

const APPS = ["ElyDoc", "ElyTable", "ElyShow", "ElyPDF", "ElyMail", "ElyTree", "ElyMark"];

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto("/");
  await page.locator(".tour .model").scrollIntoViewIfNeeded();
  await button(page).hover();
  await page.evaluate(() =>
    Promise.all(document.querySelector(".tour .model")!.getAnimations({ subtree: true }).map((a) => a.finished)),
  );
});

test.afterEach(() => expect(errors).toEqual([]));

const button = (page: Page) => page.locator(".speaker-button");
const paused = (page: Page) => page.evaluate(() => document.querySelector<HTMLAudioElement>(".tour audio")!.paused);

/** Screen top of each building's first top face. */
const tops = (page: Page) =>
  page.evaluate(
    (apps) =>
      apps.map((n) => document.querySelector(`.tour .block[data-part="${n}"] .face.top`)!.getBoundingClientRect().top),
    APPS,
  );

/** Whether anything dances: hops, road light or a moving woofer. */
const dancing = (page: Page) =>
  page.evaluate(() => {
    const hops = [...document.querySelectorAll<SVGGElement>(".tour .block")].filter((g) =>
      g.style.getPropertyValue("--hop"),
    );
    const moved = [...document.querySelectorAll<SVGGElement>(".tour .woofer")].filter((g) => g.style.translate);
    return hops.length + document.querySelectorAll(".tour .pulse").length + moved.length;
  });

test("the speaker plays and pauses the music", async ({ page }) => {
  await expect(button(page)).toHaveAccessibleName("Play music");
  await button(page).click();
  await expect.poll(() => paused(page)).toBe(false);
  await expect(button(page)).toHaveAccessibleName("Pause music");
  await button(page).click();
  await expect.poll(() => paused(page)).toBe(true);
  await expect(button(page)).toHaveAccessibleName("Play music");
});

test("buildings hop on the beat; all seven on the bar", async ({ page }) => {
  const rest = await tops(page);
  await button(page).click();
  await page.waitForFunction(
    ([apps, rest]) =>
      apps.every(
        (n, i) =>
          document.querySelector(`.tour .block[data-part="${n}"] .face.top`)!.getBoundingClientRect().top < rest[i] - 10,
      ),
    [APPS, rest] as const,
    { polling: "raf", timeout: 6000 },
  );
  await page.waitForFunction(
    ([apps, rest]) =>
      apps.filter(
        (n, i) =>
          document.querySelector(`.tour .block[data-part="${n}"] .face.top`)!.getBoundingClientRect().top < rest[i] - 10,
      ).length === 1,
    [APPS, rest] as const,
    { polling: "raf", timeout: 6000 },
  );
});

test("paused, nothing moves", async ({ page }) => {
  const rest = await tops(page);
  await button(page).click();
  await expect.poll(() => dancing(page)).toBeGreaterThan(0);
  await button(page).click();
  await expect.poll(() => paused(page)).toBe(true);
  await expect.poll(() => dancing(page)).toBe(0);
  for (let i = 0; i < 4; i++) {
    expect(await dancing(page)).toBe(0);
    expect(await tops(page)).toEqual(rest);
    await page.waitForTimeout(150);
  }
});

test.describe("reduced motion", () => {
  test.use({ reducedMotion: "reduce" });

  test("music plays, buildings stay, the woofer stays out", async ({ page }) => {
    const rest = await tops(page);
    await button(page).click();
    await expect.poll(() => paused(page)).toBe(false);
    await page.waitForTimeout(1200);
    expect(await dancing(page)).toBe(0);
    expect(await tops(page)).toEqual(rest);
    const push = await page.locator(".tour .woofer").first().evaluate((g) => getComputedStyle(g).translate);
    expect(push).not.toBe("none");
  });
});

test("the desk cell under the pointer rises", async ({ page }) => {
  const at = await page.evaluate(() => {
    const svg = document.querySelector<SVGSVGElement>(".tour .stage")!;
    const [cos, sin, unit, top] = [Math.cos(Math.PI / 6), 0.5, 28, 0.6];
    const p = new DOMPoint((10.5 - 10.5) * cos * unit, ((10.5 + 10.5) * sin - top) * unit).matrixTransform(
      svg.getScreenCTM()!,
    );
    return { x: p.x, y: p.y };
  });
  await page.mouse.move(at.x, at.y);
  await expect(page.locator(".tour .tile")).toHaveCount(1);
  await page.mouse.move(at.x, at.y - 600);
  await expect(page.locator(".tour .tile")).toHaveCount(0);
});
