// ElyMark's page: which clicks on links reach the app (M-05).
import { createRequire } from 'node:module'
import { existsSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = join(dirname(fileURLToPath(import.meta.url)), '../..')
const page = join(root, 'engines/content/ely-mark/index.html')
if (!existsSync(page)) {
  console.log('SKIP: no page; run scripts/build-mark-content.sh')
  process.exit(0)
}
let chromium
try {
  ;({ chromium } = createRequire(join(root, 'site/package.json'))('@playwright/test'))
} catch {
  console.log('SKIP: no Playwright; run pnpm install in site')
  process.exit(0)
}
const fail = (why) => {
  console.error(`FAIL: ${why}`)
  process.exit(1)
}

const browser = await chromium.launch()
const tab = await browser.newPage()
await tab.addInitScript(() => {
  window.asked = []
  window.elyQuery = ({ request, onSuccess }) => {
    window.asked.push(JSON.parse(request))
    onSuccess('')
  }
})
const left = []
await tab.route('https://example.com/**', (route) => {
  left.push(route.request().url())
  route.abort()
})
await tab.goto(pathToFileURL(page).href)
const doc = 'Go [web](https://example.com/) or [ftp](ftp://x.example/) and [one](https://one.example/)[two](https://two.example/) end.\n\n' +
  '[local](file:///outside/file.md) [data](data:text/plain,hello) [empty]()\n'
const links = async () => (await tab.evaluate(() => window.asked)).filter((a) => a.kind === 'link').map((a) => a.href)
const opener = (await tab.evaluate(() => navigator.platform)).startsWith('Mac') ? 'Meta' : 'Control'
const anchor = (text) => tab.locator('a', { hasText: new RegExp(`^${text}$`) })
// Clicks `dx` from the anchor's left edge, mid-height.
const at = async (text, dx, modifiers) => {
  const box = await anchor(text).boundingBox()
  await tab.mouse.move(box.x + dx, box.y + box.height / 2)
  for (const key of modifiers) await tab.keyboard.down(key)
  await tab.mouse.down()
  await tab.mouse.up()
  for (const key of modifiers) await tab.keyboard.up(key)
}

await tab.evaluate((doc) => window.ely.open(doc, 'visual', {}, 1, '', null), doc)
await anchor('web').click()
if ((await links()).length) fail('a plain click in the editor opened a link')
await at('web', 1, [opener])
await at('web', (await anchor('web').boundingBox()).width + 3, [opener])
await at('two', 1, [opener])
await anchor('ftp').click({ modifiers: [opener] })
const asked = await links()
const want = 'https://example.com/ https://two.example/ ftp://x.example/'
if (asked.join(' ') !== want) fail(`the editor asked ${JSON.stringify(asked)}, not ${want}`)

await tab.evaluate((doc) => window.ely.open(doc, 'preview', {}, 2, '', null), doc)
const before = (await links()).length
const here = tab.url()
for (const text of ['web', 'local', 'data', 'empty']) await anchor(text).click()
await tab.waitForTimeout(500)
const opened = (await links()).slice(before)
const sent = 'https://example.com/ file:///outside/file.md data:text/plain,hello '
if (opened.join(' ') !== sent) fail(`preview asked ${JSON.stringify(opened)}, not ${JSON.stringify(sent)}`)
if (left.length || tab.url() !== here) fail(`preview navigated: ${JSON.stringify(left)} ${tab.url()}`)
await browser.close()
console.log('mark-links: an editor click edits, the opener key asks the app, preview asks the app')
