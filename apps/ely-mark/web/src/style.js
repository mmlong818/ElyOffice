// The document's own markers, so blocks write back unchanged.
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'
import { visit } from 'unist-util-visit'

/** Writing options most of `text` already uses; tables' for remark-gfm. */
export function style(text) {
  const votes = {}
  const vote = (key, value) => {
    votes[key] ??= new Map()
    votes[key].set(value, (votes[key].get(value) ?? 0) + 1)
  }
  visit(unified().use(remarkParse).use(remarkGfm).parse(text), (node) => {
    const at = node.position?.start.offset
    if (at === undefined) return
    const head = text.slice(at, at + 12)
    switch (node.type) {
      case 'listItem': {
        const ordered = /^\d+([.)])/.exec(head)
        if (ordered) vote('bulletOrdered', ordered[1])
        else if (/^[*+-]/.test(head)) vote('bullet', head[0])
        break
      }
      case 'emphasis':
      case 'strong':
        vote(node.type, head[0])
        break
      case 'code':
        if (/^[`~]/.test(head)) vote('fence', head[0])
        break
      case 'thematicBreak':
        vote('rule', head[0])
        break
      case 'heading':
        if (node.depth <= 2) vote('setext', head[0] !== '#')
        break
      case 'table': {
        // Row nodes leave out quote and list prefixes.
        const rows = node.children.map((row) => text.slice(row.position.start.offset, row.position.end.offset).trim())
        const padded = /\| /.test(rows[0])
        const cells = rows.flatMap((row) => row.replace(/^\||\|$/g, '').split('|'))
        const inner = (cell) => (padded ? cell.replace(/^ | $/g, '') : cell)
        vote('tableCellPadding', padded)
        // Aligned cells carry spaces beyond the padding.
        vote('tablePipeAlign', cells.some((cell) => /^\s|\s$/.test(inner(cell))))
        break
      }
    }
  })
  const most = (key) => [...votes[key].entries()].sort((a, b) => b[1] - a[1])[0][0]
  return Object.fromEntries(Object.keys(votes).map((key) => [key, most(key)]))
}
