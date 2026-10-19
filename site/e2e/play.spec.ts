import { expect, test, type Page } from "./fixtures.ts";

const TREE = "Open a Markdown file's history in ElyTree.";

/** Opens the board and waits until it has grown. */
async function open(page: Page): Promise<string[]> {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("console", (m) => m.type() === "error" && errors.push(m.text()));
  await page.goto("/");
  await page.locator("#roads").scrollIntoViewIfNeeded();
  await expect(page.locator("#roads .stage")).not.toHaveClass(/grow/, { timeout: 10_000 });
  return errors;
}

/** Screen centre of a road cell. */
async function cell(page: Page, at: string) {
  const box = (await page.locator(`#roads [data-cell="${at}"]:not(.file)`).boundingBox())!;
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
}

const mark = (page: Page) => page.getByRole("button", { name: "ElyMark file" });

/** Screen point of a grid spot on the desk top. */
const desk = (page: Page, x: number, y: number) =>
  page.evaluate(([x, y]) => {
    const svg = document.querySelector<SVGSVGElement>("#roads .stage")!;
    const p = new DOMPoint((x - y) * Math.cos(Math.PI / 6) * 28, ((x + y) / 2 - 0.6) * 28).matrixTransform(svg.getScreenCTM()!);
    return { x: p.x, y: p.y };
  }, [x, y]);

test("dragging Mark's file down its road reaches ElyTree", async ({ page }) => {
  const errors = await open(page);
  const from = (await mark(page).boundingBox())!;
  const to = await cell(page, "14,12");
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x, to.y, { steps: 12 });
  await page.mouse.up();
  await expect(mark(page)).toHaveAttribute("data-cell", "14,12");
  await expect(page.locator("#roads .news")).toHaveText(TREE);
  await expect(page.locator('#roads [data-cell="14,12"]:not(.file)')).toHaveCSS("--tone", "#3d7fe5");
  expect(errors).toEqual([]);
});

test("a file dragged onto empty desk stays on its road", async ({ page }) => {
  const errors = await open(page);
  const from = (await mark(page).boundingBox())!;
  const away = await desk(page, 10.5, 8.5);
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(away.x, away.y);
  await page.mouse.up();
  await page.waitForTimeout(500);
  await expect(mark(page)).toHaveAttribute("data-cell", "5,10");
  await expect(page.locator("#roads .news")).toHaveText("");
  expect(errors).toEqual([]);
});

test("arrow keys walk Mark's file to ElyTree, then reset", async ({ page }) => {
  const errors = await open(page);
  await mark(page).focus();
  for (const key of ["ArrowDown", "ArrowDown", ...Array(9).fill("ArrowRight")])
    await page.keyboard.press(key);
  await expect(mark(page)).toHaveAttribute("data-cell", "14,12");
  await expect(page.locator("#roads .news")).toHaveText(TREE);
  await page.getByRole("button", { name: "Put the files back" }).click();
  await expect(mark(page)).toHaveAttribute("data-cell", "5,10");
  await expect(page.locator("#roads .news")).toHaveText("");
  expect(errors).toEqual([]);
});

test.describe("reduced motion", () => {
  test.use({ reducedMotion: "reduce" });

  test("files stack at ElyPDF and the line says why", async ({ page }) => {
    const errors = await open(page);
    const doc = page.getByRole("button", { name: "ElyDoc file" });
    await doc.focus();
    await page.keyboard.press("ArrowDown");
    for (let i = 0; i < 13; i++) await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowUp");
    await expect(doc).toHaveAttribute("data-cell", "15,5");
    await expect(page.locator("#roads .news")).toHaveText("Export to PDF, open it in ElyPDF.");
    const show = page.getByRole("button", { name: "ElyShow file" });
    await show.focus();
    for (const key of ["ArrowDown", "ArrowDown", "ArrowRight", "ArrowRight", "ArrowRight", "ArrowRight", "ArrowRight", "ArrowUp"])
      await page.keyboard.press(key);
    await expect(show).toHaveAttribute("data-cell", "15,5");
    expect(errors).toEqual([]);
  });
});
