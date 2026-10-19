import { test as base } from "@playwright/test";

export { expect, type Page } from "@playwright/test";

export const VERSION = "9.9.9";
export const DOWNLOADS = 12345;

// Every page reads this version.
export const test = base.extend<{ release: void }>({
  release: [
    async ({ context }, use) => {
      await context.route("**/release.json", (r) => r.fulfill({ json: { version: VERSION, downloads: DOWNLOADS } }));
      await use();
    },
    { auto: true },
  ],
});
