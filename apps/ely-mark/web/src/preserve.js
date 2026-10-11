// Blocks Milkdown cannot write back stay raw text (§14.3).
import { editorViewCtx, remarkCtx, schemaCtx, serializerCtx } from '@milkdown/kit/core'
import { canon } from './tables.js'
import { Fragment } from '@milkdown/kit/prose/model'
import { Selection } from '@milkdown/kit/prose/state'
import { $nodeSchema } from '@milkdown/kit/utils'
import { common } from './diff.js'

/** Source shown and written as it is. */
export const raw = $nodeSchema('ely_raw', () => ({
  group: 'block',
  content: 'text*',
  marks: '',
  code: true,
  defining: true,
  isolating: true,
  parseDOM: [{ tag: 'pre.ely-raw', preserveWhitespace: 'full' }],
  toDOM: () => ['pre', { class: 'ely-raw' }, 0],
  parseMarkdown: { match: () => false, runner: () => {} },
  toMarkdown: {
    match: (node) => node.type.name === 'ely_raw',
    runner: (state, node) => state.addNode('html', undefined, node.textContent),
  },
}))

/** Each block's span, as Milkdown reads it; null if unplaced. */
function spans(ctx, text) {
  const remark = ctx.get(remarkCtx)
  const tree = remark.runSync(remark.parse(text), text)
  const at = tree.children.map(
    (node) => node.position && [node.position.start.offset, node.position.end.offset],
  )
  return at.every(Boolean) ? at : null
}

/** The text in `from`..`to` past its outer whitespace. */
function inner(text, from, to) {
  const piece = text.slice(from, to)
  const start = piece.search(/\S/)
  if (start < 0) return null
  return [from + start, from + piece.trimEnd().length]
}

/** Raw blocks in place; edits then rewrite only their blocks. */
export function faithful(ctx, original) {
  const view = ctx.get(editorViewCtx)
  const parsed = view.state.doc
  const at = spans(ctx, original)
  // No blocks: Milkdown still holds one empty paragraph.
  const blank = at?.length === 0 && parsed.childCount === 1 && parsed.textContent === ''
  if (!at || (at.length !== parsed.childCount && !blank)) return null
  const schema = ctx.get(schemaCtx)
  const serializer = ctx.get(serializerCtx)
  const written = (nodes) => serializer(schema.topNodeType.create(null, nodes)).replace(/\n+$/, '')
  // Table spacing is free; every cell's source must match.
  const kept = (node, text) => {
    const own = written([node])
    return own === text || canon(own) === canon(text)
  }
  const rawOf = ([from, to]) => {
    const text = original.slice(from, to)
    return schema.nodes.ely_raw.create(null, text ? schema.text(text) : null)
  }
  const nodes = []
  const places = []
  // Text Milkdown drops between blocks, as raw blocks.
  const gap = (from, to) => {
    const place = inner(original, from, to)
    if (!place) return
    nodes.push(rawOf(place))
    places.push(place)
  }
  let last = 0
  if (!blank)
    parsed.forEach((node, _, index) => {
      gap(last, at[index][0])
      nodes.push(kept(node, original.slice(...at[index])) ? node : rawOf(at[index]))
      places.push(at[index])
      last = at[index][1]
    })
  gap(last, original.length)
  if (nodes.length) {
    const swap = view.state.tr.replaceWith(0, parsed.content.size, Fragment.from(nodes))
    swap.setSelection(Selection.atStart(swap.doc))
    view.dispatch(swap.setMeta('addToHistory', false))
  }
  const first = view.state.doc
  const before = []
  first.forEach((node) => before.push(node))
  /** Where each fresh block sits in `text` from `at`. */
  const found = (nodes, text, at, spans) => {
    let from = 0
    for (const node of nodes) {
      const own = written([node])
      const place = text.indexOf(own, from)
      spans.push(place < 0 ? null : [at + place, at + place + own.length])
      if (place >= 0) from = place + own.length
    }
  }
  // The text and each block's span in it.
  return (doc) => {
    if (doc.eq(first)) return { text: original, spans: places }
    const after = []
    doc.forEach((node) => after.push(node))
    if (!places.length) {
      const fresh = written(after)
      const spans = []
      found(after, fresh, original.length, spans)
      return { text: `${original}${fresh}\n`, spans }
    }
    const pairs = common(before, after, (x, y) => x === y || x.eq(y))
    pairs.push([before.length, after.length])
    let out = ''
    let cursor = 0
    let [i, j] = [0, 0]
    const spans = []
    for (const [x, y] of pairs) {
      if (x > i || y > j) {
        const fresh = y > j ? written(after.slice(j, y)) : ''
        let start, end, text
        if (x > i && fresh) {
          ;[start, end, text] = [places[i][0], places[x - 1][1], fresh]
        } else if (x > i && i > 0) {
          // Removed blocks take the gap before them.
          ;[start, end, text] = [places[i - 1][1], places[x - 1][1], '']
        } else if (x > i && x < places.length) {
          ;[start, end, text] = [places[0][0], places[x][0], '']
        } else if (x > i) {
          ;[start, end, text] = [places[0][0], places[x - 1][1], '']
        } else if (i > 0) {
          ;[start, end, text] = [places[i - 1][1], places[i - 1][1], `\n\n${fresh}`]
        } else {
          ;[start, end, text] = [places[0][0], places[0][0], `${fresh}\n\n`]
        }
        out += original.slice(cursor, start)
        found(after.slice(j, y), text, out.length, spans)
        out += text
        cursor = end
      }
      // Unchanged blocks keep their bytes, shifted.
      if (x < places.length) {
        const shift = out.length - cursor
        spans.push([places[x][0] + shift, places[x][1] + shift])
      }
      ;[i, j] = [x + 1, y + 1]
    }
    return { text: out + original.slice(cursor), spans }
  }
}
