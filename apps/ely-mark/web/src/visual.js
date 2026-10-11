// Milkdown: visual editing, or read-only preview.
import {
  Editor,
  defaultValueCtx,
  remarkStringifyOptionsCtx,
  editorViewCtx,
  editorViewOptionsCtx,
  parserCtx,
  remarkCtx,
  rootCtx,
} from '@milkdown/kit/core'
import { commonmark } from '@milkdown/kit/preset/commonmark'
import { gfm } from '@milkdown/kit/preset/gfm'
import { history } from '@milkdown/kit/plugin/history'
import { codeBlockComponent } from '@milkdown/kit/component/code-block'
import { Plugin, TextSelection } from '@milkdown/kit/prose/state'
import { Decoration, DecorationSet } from '@milkdown/kit/prose/view'
import { keymap } from '@milkdown/kit/prose/keymap'
import { liftListItem, sinkListItem } from '@milkdown/kit/prose/schema-list'
import { redo, redoDepth, undoDepth } from '@milkdown/kit/prose/history'
import { $prose, callCommand } from '@milkdown/kit/utils'
import { insertTableCommand, remarkGFMPlugin } from '@milkdown/kit/preset/gfm'
import {
  addColumnAfter,
  addColumnBefore,
  addRowAfter,
  addRowBefore,
  deleteColumn,
  deleteRow,
} from '@milkdown/kit/prose/tables'
import { visit } from 'unist-util-visit'
import { image } from './image.js'
import { math } from './math.js'
import { code } from './code.js'
import { diagrams } from './diagrams.js'
import { faithful, raw } from './preserve.js'
import { docFinder } from './find.js'
import { search } from 'prosemirror-search'
import { style } from './style.js'

/** An editor over `markdown`; null when blocks cannot map. */
export async function visual(root, markdown, editable, texts, changed, edge, linked) {
  const owner = diagrams()
  let write = () => ({ text: markdown, spans: [] })
  // Setting up raw blocks is no edit.
  let ready = false
  const watch = $prose(
    () =>
      new Plugin({
        view: () => ({
          update: (view, before) => {
            if (ready && !view.state.doc.eq(before.doc)) changed(() => write(view.state.doc).text)
          },
        }),
      }),
  )
  // Focus mode dims all blocks but the caret's.
  const current = $prose(
    () =>
      new Plugin({
        props: {
          decorations: (state) => {
            const { $from } = state.selection
            if ($from.depth < 1) return null
            const start = $from.before(1)
            const block = Decoration.node(start, start + $from.node(1).nodeSize, {
              class: 'ely-current',
            })
            return DecorationSet.create(state.doc, [block])
          },
        },
      }),
  )
  // Preview: a click opens. Editor: Cmd (Ctrl elsewhere).
  const mac = navigator.platform.startsWith('Mac')
  const links = $prose(
    () =>
      new Plugin({
        props: {
          handleDOMEvents: {
            click: (view, event) => {
              const anchor = event.target.closest?.('a')
              if (!anchor || !view.dom.contains(anchor)) return false
              event.preventDefault()
              if (editable && !(mac ? event.metaKey : event.ctrlKey)) return true
              // The DOM href is sanitized; the mark is not.
              const node = view.state.doc.nodeAt(view.posAtDOM(anchor, 0))
              const mark = node?.marks.find((mark) => mark.type.name === 'link')
              if (!mark) throw new Error('ely: a link without its mark')
              linked(mark.attrs.href)
              return true
            },
          },
        },
      }),
  )
  // Past their history, undo and redo reach the page.
  const ends = $prose(() =>
    keymap({
      'Mod-z': (state) => undoDepth(state) === 0 && edge('undo'),
      'Mod-Shift-z': (state) => redoDepth(state) === 0 && edge('redo'),
      'Mod-y': (state) => redoDepth(state) === 0 && edge('redo'),
      // Tab moves list items; it never leaves the editor.
      Tab: (state, dispatch) => sinkListItem(state.schema.nodes.list_item)(state, dispatch) || true,
      'Shift-Tab': (state, dispatch) =>
        liftListItem(state.schema.nodes.list_item)(state, dispatch) || true,
    }),
  )
  const made = Editor.make()
    .config((ctx) => {
      ctx.set(rootCtx, root)
      ctx.set(defaultValueCtx, markdown)
      const { tableCellPadding, tablePipeAlign, ...marks } = style(markdown)
      ctx.update(remarkStringifyOptionsCtx, (prev) => ({ ...prev, ...marks }))
      ctx.set(remarkGFMPlugin.options.key, { tableCellPadding, tablePipeAlign })
      ctx.update(editorViewOptionsCtx, (prev) => ({ ...prev, editable: () => editable }))
    })
    .use(commonmark)
    .use(gfm)
    .use(history)
    .use(image)
    .use(math)
    .use(raw)
    .config(code(texts, editable, owner))
    .use(codeBlockComponent)
    .use(watch)
    .use(current)
    .use(ends)
    .use(links)
    .use($prose(() => search()))
    .create()
  const editor = await made.catch((error) => {
    owner.release()
    throw error
  })
  const view = editor.action((ctx) => ctx.get(editorViewCtx))
  const close = () => {
    editor.destroy()
    owner.release()
  }
  const keep = editor.action((ctx) => faithful(ctx, markdown))
  if (!keep) {
    close()
    return null
  }
  write = keep
  ready = true
  // The text once every undone step is redone.
  const redone = () => {
    let state = view.state
    if (!redoDepth(state)) return null
    while (redoDepth(state)) redo(state, (tr) => (state = state.apply(tr)))
    return write(state.doc).text
  }
  // Headings, raw ones too, by the title their source gives.
  const [remark, parse] = editor.action((ctx) => [ctx.get(remarkCtx), ctx.get(parserCtx)])
  /** An mdast node's words. */
  const words = (node) => node.value ?? (node.children ?? []).map(words).join('')
  // Headings; inside raw blocks, as remark reads their source.
  const headings = () => {
    const found = []
    view.state.doc.descendants((node, pos) => {
      if (node.type.name === 'heading')
        found.push({ level: node.attrs.level, title: node.textContent, pos })
      if (node.type.name !== 'ely_raw') return true
      visit(remark.parse(node.textContent), 'heading', (heading) => {
        const start = (heading.children[0] ?? heading).position.start.offset
        found.push({ level: heading.depth, title: words(heading), pos, at: pos + 1 + start })
      })
      return false
    })
    return found
  }
  // `exact`: the caret at `pos`; else in that block.
  const reveal = (pos, exact) => {
    if (editable) {
      const doc = view.state.doc
      const place = exact
        ? TextSelection.create(doc, pos)
        : TextSelection.near(doc.resolve(pos + 1))
      view.dispatch(view.state.tr.setSelection(place).scrollIntoView())
      return view.focus()
    }
    const { node } = view.domAtPos(pos + 1)
    ;(node.nodeType === 1 ? node : node.parentElement).scrollIntoView({ block: 'center' })
  }
  /** `node`'s text, and where a character of it sits. */
  const reading = (node, start) => {
    const chunks = []
    // Text and line breaks, as `textContent` reads them.
    node.descendants((child, at) => {
      const words = child.isText
        ? child.text
        : child.isLeaf
          ? child.type.spec.leafText?.(child)
          : null
      if (words) chunks.push({ text: words.toLowerCase(), at: start + 1 + at })
    })
    const at = (offset) => {
      for (const [index, chunk] of chunks.entries()) {
        const last = index === chunks.length - 1
        if (offset < chunk.text.length || (last && offset === chunk.text.length))
          return chunk.at + offset
        offset -= chunk.text.length
      }
      return null
    }
    return { joined: chunks.map((chunk) => chunk.text).join(''), at }
  }
  // Non-overlapping hits, the same rule as `split`.
  const hits = (text, needle) => text.split(needle).length - 1
  /** The block holding file line `n`: its index and start. */
  const blockOf = (n) => {
    const { text, spans } = write(view.state.doc)
    let offset = 0
    for (let k = 1; k < n && offset >= 0; k++) offset = text.indexOf('\n', offset) + 1
    if (n < 1 || offset < 0) return null
    const index = spans.findIndex((span) => span && span[1] >= offset)
    if (index < 0) return null
    let pos = 0
    for (let k = 0; k < index; k++) pos += view.state.doc.child(k).nodeSize
    return { text, spans, offset, index, pos }
  }
  /** Line `n`: the query's place in its block. */
  const line = (n, query) => {
    const block = blockOf(n)
    if (!block) return false
    const { text, spans, offset, index, pos } = block
    const needle = query.toLowerCase()
    const node = view.state.doc.child(index)
    // Text before and through the line, as shown.
    const shown = (end) => {
      const source = text.slice(spans[index][0], Math.max(spans[index][0], end))
      return (node.type.name === 'ely_raw' ? source : parse(source).textContent).toLowerCase()
    }
    const ends = text.indexOf('\n', offset)
    const before = shown(offset)
    const through = shown(ends < 0 ? text.length : ends)
    const { joined, at } = reading(node, pos)
    const k = needle ? hits(before, needle) : 0
    let found = -1
    if (needle && hits(through, needle) > k) {
      let from = 0
      for (let seen = 0; seen <= k; seen++) {
        found = joined.indexOf(needle, from)
        if (found < 0) break
        from = found + needle.length
      }
    }
    // A hit hidden, as in a link: the line's start.
    const place = found >= 0 ? at(found) : at(before.length)
    reveal(place ?? pos, place !== null)
    return true
  }
  /** The caret: its block's file line, and how far in. */
  const where = () => {
    const { text, spans } = write(view.state.doc)
    const { from } = view.state.selection
    let [start, pos] = [0, 0]
    view.state.doc.forEach((node, offset, index) => {
      if (offset <= from && spans[index]) [start, pos] = [spans[index][0], offset]
    })
    return { line: text.slice(0, start).split('\n').length, column: from - pos }
  }
  /** The caret back where `where` read it. */
  const at = (n, column) => {
    const block = blockOf(n)
    if (!block) return false
    const size = view.state.doc.child(block.index).nodeSize
    reveal(block.pos + Math.min(Math.max(column, 1), size - 1) - 1, false)
    return true
  }
  // Rows and columns; false when the caret is outside tables.
  const tables = {
    'row-above': addRowBefore,
    'row-below': addRowAfter,
    'column-left': addColumnBefore,
    'column-right': addColumnAfter,
    'delete-row': deleteRow,
    'delete-column': deleteColumn,
  }
  const tablesIn = (doc) => {
    const found = []
    doc.descendants((node, pos) => {
      if (node.type.name === 'table') found.push({ node, pos })
    })
    return found
  }
  // Milkdown may leave the caret before the new table.
  const newTable = () => {
    const old = new Set(tablesIn(view.state.doc).map(({ node }) => node))
    editor.action(callCommand(insertTableCommand.key, { row: 3, col: 3 }))
    const made = tablesIn(view.state.doc).find(({ node }) => !old.has(node))
    if (!made) throw new Error('the inserted table is missing')
    const cell = TextSelection.near(view.state.doc.resolve(made.pos + 1))
    view.dispatch(view.state.tr.setSelection(cell).scrollIntoView())
    return true
  }
  const table = (op) => {
    if (!editable) return false
    const done = op === 'insert' ? newTable() : tables[op](view.state, view.dispatch)
    view.focus()
    return done
  }
  /** Images at the caret: `{ src, alt }` each. */
  const insert = (images) => {
    if (!editable) return false
    const image = view.state.schema.nodes.image
    let tr = view.state.tr
    for (const { src, alt } of images)
      tr = tr.replaceSelectionWith(image.create({ src, alt, title: '' }), false)
    view.dispatch(tr.scrollIntoView())
    view.focus()
    return true
  }
  return {
    insert,
    table,
    find: docFinder(view),
    close,
    text: () => write(view.state.doc).text,
    redone,
    focus: () => editable && view.focus(),
    outline: () => headings().map(({ level, title }) => ({ level, title })),
    heading: (key) => {
      const found = headings()[key]
      if (found) reveal(found.at ?? found.pos, found.at !== undefined)
      return Boolean(found)
    },
    line,
    where,
    at,
    // The caret's line to the middle of the view.
    center: () => {
      const { top } = view.coordsAtPos(view.state.selection.head)
      window.scrollBy(0, top - window.innerHeight / 2)
    },
  }
}
