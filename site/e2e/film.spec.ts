import { expect, test, type Page } from "./fixtures.ts";

const PAGE = "/";

/** Scrolls to the film and mutes it for autoplay. */
async function reach(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(m.text());
  });
  await page.goto(PAGE);
  await page.locator("#film").scrollIntoViewIfNeeded();
  await expect(page.locator("#film")).toHaveClass(/seen/);
  await page.evaluate(() => {
    const video = document.querySelector<HTMLVideoElement>("#film video")!;
    video.muted = true;
    Object.assign(window, { filmVideo: video });
  });
  return errors;
}

const box = (page: Page) => page.locator("#film video").evaluate((v) => v.getBoundingClientRect().toJSON());

test("the film flattens to play and returns to its face", async ({ page }) => {
  const errors = await reach(page);
  await expect(page.locator("#film video")).toHaveAttribute("preload", "metadata");
  const face = await box(page);
  const play = page.getByRole("button", { name: "Play the film" });
  await play.click();
  await expect.poll(() => page.locator("#film video").evaluate((v: HTMLVideoElement) => v.paused)).toBe(false);
  await expect.poll(() => page.locator("#film video").evaluate((v) => v.getAnimations().length)).toBe(0);
  const flat = await box(page);
  expect(flat.width / flat.height).toBeCloseTo(16 / 9, 1);
  expect(flat.width).toBeGreaterThan(page.viewportSize()!.width * 0.7);
  expect(flat.width).toBeGreaterThan(face.width * 1.5);
  await expect(page.locator("#film video")).toHaveJSProperty("controls", true);

  await page.keyboard.press("Escape");
  await expect(play).toBeFocused();
  await expect.poll(() => page.locator("#film video").evaluate((v) => v.getAnimations().length)).toBe(0);
  const back = await box(page);
  expect(back.width).toBeCloseTo(face.width, 0);
  expect(back.height).toBeCloseTo(face.height, 0);
  const state = await page.evaluate(() => {
    const video = document.querySelector<HTMLVideoElement>("#film video")!;
    return { same: video === (window as unknown as { filmVideo: HTMLVideoElement }).filmVideo, paused: video.paused, at: video.currentTime };
  });
  expect(state.same).toBe(true);
  expect(state.paused).toBe(true);
  expect(state.at).toBeGreaterThan(0);
  expect(errors).toEqual([]);
});

test("with reduced motion the film plays flat at once", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  const errors = await reach(page);
  await page.getByRole("button", { name: "Play the film" }).click();
  const video = page.locator("#film video");
  expect(await video.evaluate((v) => [v.getAnimations().length, getComputedStyle(v).transform])).toEqual([0, "none"]);
  await expect.poll(() => video.evaluate((v: HTMLVideoElement) => v.paused)).toBe(false);
  await page.keyboard.press("Escape");
  await expect(page.getByRole("button", { name: "Play the film" })).toBeFocused();
  expect(errors).toEqual([]);
});
