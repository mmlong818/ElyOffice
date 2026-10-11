// Tables compare by alignment and cell source, not spacing.
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'
import { SKIP, visit } from 'unist-util-visit'

const reader = unified().use(remarkParse).use(remarkGfm)

/** A cell's own source, without its padding. */
function cell(text, node) {
  const kids = node.children
  return kids.length ? text.slice(kids[0].position.start.offset, kids.at(-1).position.end.offset) : ''
}

/** `text` with each table's spacing made canonical. */
export function canon(text) {
  const tables = []
  visit(reader.parse(text), 'table', (node) => {
    tables.push(node)
    return SKIP
  })
  let out = text
  // Later tables first, so earlier offsets hold.
  for (const table of tables.reverse()) {
    const width = table.children[0].children.length
    // GFM fills a short row with empty cells.
    const rows = table.children.map((row) => {
      const cells = row.children.map((node) => cell(text, node))
      return [...cells, ...Array(Math.max(0, width - cells.length)).fill('')].join('|')
    })
    const shape = [table.align.join(','), ...rows].join('\n')
    out = `${out.slice(0, table.position.start.offset)}\u0000${shape}\u0000${out.slice(table.position.end.offset)}`
  }
  return out
}
