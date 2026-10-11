// ElyMark's page: a failed editor says so (M-13).
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
  // Setting a role throws; `spare` keeps CodeMirror's.
  const set = Element.prototype.setAttribute
  window.breaks = (spare) => {
    Element.prototype.setAttribute = function (name, value) {
      const kept = spare && this.classList.contains('cm-content')
      if (name === 'role' && !kept) throw new Error('broken')
      return set.call(this, name, value)
    }
  }
  window.mends = () => {
    Element.prototype.setAttribute = set
  }
})
await tab.goto(pathToFileURL(page).href)
// What the app heard since the last look.
let seen = 0
const heard = async () => {
  await tab.waitForTimeout(800)
  const all = await tab.evaluate(() => window.asked.filter((a) => a.kind === 'shown' || a.kind === 'failed'))
  const fresh = all.slice(seen)
  seen = all.length
  return JSON.stringify(fresh)
}
const holders = () => tab.evaluate(() => document.querySelectorAll('#document > div').length)
const shown = () => tab.evaluate(() => document.querySelector('#document .cm-content')?.textContent ?? '')
const open = (mode) => tab.evaluate((mode) => window.ely.open('# Kept\n', mode, {}, 1, '', null), mode)

await tab.evaluate(() => window.breaks(true))
await open('visual')
const fell = JSON.stringify([{ kind: 'shown', mode: 'source', fallback: true, broke: 'Error' }])
if ((await heard()) !== fell) fail('a broken visual editor did not fall back to source')
if ((await shown()) !== '# Kept') fail(`the fallback shows ${JSON.stringify(await shown())}`)

await tab.evaluate(() => window.breaks(false))
const failed = JSON.stringify([{ kind: 'failed', generation: 1, what: 'Error' }])
for (let n = 0; n < 3; n++) {
  await open('visual')
  if ((await heard()) !== failed) fail(`two broken editors, try ${n}: the app was not told`)
}
if ((await holders()) !== 0) fail(`failed builds left ${await holders()} holders`)

await tab.evaluate(() => window.mends())
await tab.evaluate(() => window.ely.mode('source'))
const back = JSON.stringify([{ kind: 'shown', mode: 'source', fallback: false, broke: null }])
if ((await heard()) !== back) fail('a mode after the failure did not rebuild')
if ((await shown()) !== '# Kept' || (await holders()) !== 1) fail('the rebuilt editor lost the text')
await browser.close()
console.log('mark-broken: fallback, told failures, no leftover holders, a mode rebuilds')
