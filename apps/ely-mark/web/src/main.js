// The page the app drives: one document, three modes.
import { visual } from './visual.js'
import { source } from './source.js'
import { ask } from './bridge.js'
import { sameQuery } from './find.js'
import { apply, follow } from './look.js'
import { html, text } from './render.js'

let texts = {}
let latest = 0
let version = 0
// The app's count of page loads; reports carry it.
let generation = 0
let timer
let pending = null
/** The editor shown. */
let current = null
// A mode asked for while an editor builds.
let asked = null
// The text no editor could show; a mode retries it.
let broken = null
// A place to show once the editor is built.
let revealing = null
// The search in force, kept across modes.
let seeking = null
/** The file's texts where a mode with edits ended (§14.3). */
let times = []
let at = 0

/** YAML front matter, kept out of the visual editor. */
function split(text) {
  const front = /^---\n[\s\S]*?\n(?:---|\.\.\.)[ \t]*(?:\n|$)/.exec(text)
  return front ? [front[0], text.slice(front[0].length)] : ['', text]
}

const tell = (said) => ask(said).catch((error) => console.error(`ely: ${error.message}`))

/** Sends the waiting text now. */
function flush() {
  clearTimeout(timer)
  if (pending) {
    tell(pending())
    outline()
  }
  pending = null
}

/** A link the user opens; the app decides. */
const linked = (href) => tell({ kind: 'link', generation, href })

/** The shown editor's headings, for the app's outline. */
function outline() {
  if (current) tell({ kind: 'outline', generation, items: current.outline() })
}

/** A new text: told now, its words once typing rests. */
function revised(text) {
  version += 1
  const now = version
  tell({ kind: 'changed', version: now, generation })
  clearTimeout(timer)
  pending = () => ({ kind: 'text', version: now, generation, text: text() })
  timer = setTimeout(flush, 300)
}

/** An edit: later texts in the timeline are gone. */
function changed(text) {
  times.length = at + 1
  revised(text)
}

/** An editor for `mode`; a failed one leaves no holder. */
async function build(text, mode) {
  const holder = document.createElement('div')
  holder.hidden = true
  document.getElementById('document').append(holder)
  try {
    return await fill(holder, text, mode)
  } catch (error) {
    holder.remove()
    throw error
  }
}

/** Builds in `holder`; visual falls back to source. */
async function fill(holder, text, mode) {
  const entry = { mode, holder, fallback: false }
  if (mode !== 'source') {
    const [front, body] = split(text)
    if (front) {
      const raw = document.createElement('pre')
      raw.className = 'ely-front'
      raw.textContent = front
      holder.append(raw)
    }
    const place = document.createElement('div')
    holder.append(place)
    const edited = (write) => changed(() => front + write())
    let made
    try {
      made = await visual(place, body, mode === 'visual', texts, edited, edge, linked)
    } catch (error) {
      // A broken editor falls back to source and says so.
      entry.broke = error.name
    }
    if (made) {
      // Lines count from the file's top, front matter included.
      const above = front ? front.split('\n').length - 1 : 0
      const line = (n, query) => {
        if (n > above) return made.line(n - above, query)
        holder.firstChild.scrollIntoView({ block: 'center' })
        return true
      }
      const redone = () => {
        const ahead = made.redone()
        return ahead === null ? null : front + ahead
      }
      const where = () => {
        const { line, column } = made.where()
        return { line: above + line, column }
      }
      const at = (n, column) => (n > above ? made.at(n - above, column) : line(n, ''))
      return { ...entry, ...made, text: () => front + made.text(), redone, line, where, at }
    }
    // Blocks the editor cannot map open as source.
    holder.replaceChildren()
    Object.assign(entry, { mode: 'source', fallback: true })
  }
  return { ...entry, ...source(holder, text, changed, edge) }
}

function drop(entry) {
  entry.close()
  entry.holder.remove()
}

/** Shows `text` as `want`; the newest request alone lands. */
async function show(text, want) {
  const turn = ++latest
  asked = null
  broken = null
  if (current) drop(current)
  current = null
  let entry
  try {
    entry = await build(text, want)
  } catch (error) {
    if (turn !== latest) return
    // Source failed too: tell the app, keep the text.
    broken = text
    tell({ kind: 'failed', generation, what: error.name })
    const next = asked
    asked = null
    return next && mode(next)
  }
  if (turn !== latest) return drop(entry)
  current = entry
  entry.holder.hidden = false
  document.getElementById('document').dataset.mode = entry.mode
  entry.focus()
  await ask({ kind: 'shown', mode: entry.mode, fallback: entry.fallback, broke: entry.broke ?? null })
  // A newer request owns what is queued now.
  if (turn !== latest) return
  outline()
  if (seeking) found(entry.find.seek(seeking))
  const place = revealing
  revealing = null
  if (place) reveal(place)
  const next = asked
  asked = null
  if (next && next !== entry.mode) return mode(next)
}

/** Undo or redo past an editor's own history. */
function edge(direction) {
  const to = direction === 'undo' ? at - 1 : at + 1
  if (!current || to < 0 || to >= times.length) return false
  flush()
  at = to
  const text = times[at]
  revised(() => text)
  show(text, current.mode)
  return true
}

/** Relative links and images resolve under `href`. */
function base(href) {
  const link =
    document.querySelector('base') ?? document.head.appendChild(document.createElement('base'))
  link.href = href
}

/** Puts the caret on `place.line`, the page at `place.scroll`. */
function settle(place) {
  if (!current || !place) return
  current.at(place.line, place.column)
  document.scrollingElement.scrollTop = place.scroll
}

/** A document, after each page load; `place` restores a session. */
async function open(text, want, words, load, folder, place) {
  texts = words
  generation = load
  times = [text]
  at = 0
  base(folder)
  await show(text, want)
  settle(place)
}

/** Changed on disk: the caret and scroll stay near. */
async function refresh(text, load) {
  const place = current && { ...current.where(), scroll: document.scrollingElement.scrollTop }
  clearTimeout(timer)
  pending = null
  generation = load
  version = 0
  times = [text]
  at = 0
  await show(text, current?.mode ?? 'visual')
  settle(place)
}

/** `text` in place of the editor's, as one edit. */
function adopt(text) {
  if (current) flush()
  times.length = at + 1
  times.push(text)
  at += 1
  revised(() => text)
  return show(text, current?.mode ?? 'visual')
}

let placing
/** The caret's line and the scroll, once they rest. */
function placed() {
  clearTimeout(placing)
  placing = setTimeout(() => {
    if (!current) return
    const scroll = document.scrollingElement.scrollTop
    tell({ kind: 'place', generation, ...current.where(), scroll })
  }, 500)
}
document.addEventListener('selectionchange', () => {
  placed()
  follow(current)
})
window.addEventListener('scroll', placed, { passive: true })

/** Another mode for the same text; edits are sent first. */
function mode(next) {
  if (!current) {
    if (broken !== null) return show(broken, next)
    asked = next
    return
  }
  const text = current.text()
  const ahead = current.redone?.() ?? null
  flush()
  // Leaving a mode with edits marks the timeline.
  if (text !== times[at]) {
    times.length = at + 1
    times.push(text)
    at += 1
  }
  // Undone steps stay ahead, for redo.
  if (ahead !== null && ahead !== text) {
    times.length = at + 1
    times.push(ahead)
  }
  return show(text, next)
}

/** Shows `{ heading }` or `{ line, query }`. */
function reveal(place) {
  if (!current) {
    revealing = place
    return
  }
  const shown =
    'heading' in place ? current.heading(place.heading) : current.line(place.line, place.query)
  // The app logs a line number, never the query.
  if (shown === false) tell({ kind: 'missed', generation, line: place.line ?? null })
}

/** The app's find box hears where the search stands. */
const found = (status) => tell({ kind: 'found', generation, ...status })

/** Searches for `spec`: search, caseSensitive, regexp, wholeWord, replace. */
function find(spec) {
  seeking = spec
  if (current) found(current.find.seek(spec))
}

/** The box's spec rules; a new query starts afresh. */
function step({ spec, forward }) {
  const fresh = !seeking || !sameQuery(seeking, spec)
  find(spec)
  if (current && !fresh) found(current.find.step(forward))
}

function replace({ spec, all }) {
  find(spec)
  if (current && current.mode !== 'preview') found(current.find.replace(all))
}

function unfind() {
  seeking = null
  current?.find.clear()
}

/** The writing view's settings (MARK-09). */
function look(settings) {
  apply(settings)
  follow(current)
}

/** The whole text as `kind`: html or text, tagged `ask`. */
async function render({ kind, ask: asked }) {
  try {
    if (!current) throw new Error('no document is shown')
    const [, body] = split(current.text())
    const result = kind === 'text' ? { text: await text(body) } : await html(body)
    tell({ kind: 'rendered', generation, ask: asked, ...result })
  } catch (error) {
    tell({ kind: 'rendered', generation, ask: asked, error: error.message })
  }
}

/** A table step; the app hears whether it ran. */
function table(op) {
  const done = current?.table?.(op) ?? false
  tell({ kind: 'table', generation, op, done })
}

/** Image links at the caret; preview takes none. */
function insert(images) {
  const done = current?.insert?.(images) ?? false
  tell({ kind: 'inserted', generation, done })
}

window.ely = { open, refresh, adopt, look, render, mode, base, reveal, find, step, replace, unfind, table, insert }
