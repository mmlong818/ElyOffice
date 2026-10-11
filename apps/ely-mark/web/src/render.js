// The whole document as one page or as text (MARK-11).
import { unified } from 'unified'
import remarkParse from 'remark-parse'
import remarkGfm from 'remark-gfm'
import remarkMath from 'remark-math'
import remarkRehype from 'remark-rehype'
import rehypeKatex from 'rehype-katex'
import { fromDom } from 'hast-util-from-dom'
import rehypeStringify from 'rehype-stringify'
import { toText } from 'hast-util-to-text'
import { visit } from 'unist-util-visit'
import { VFile } from 'vfile'
import { bundle } from './diagrams.js'

const markdown = () => unified().use(remarkParse).use(remarkGfm).use(remarkMath)

/** Mermaid drawn for paper; a broken one stays code. */
async function draw(tree) {
  const blocks = []
  visit(tree, 'code', (node) => {
    if (node.lang === 'mermaid') blocks.push(node)
  })
  const failed = []
  if (!blocks.length) return failed
  const paint = await bundle()
  for (const node of blocks) {
    try {
      const svg = await paint(node.value, 'paper')
      Object.assign(node, { type: 'html', value: `<figure class="diagram">${svg}</figure>` })
    } catch {
      // Lines only: Mermaid's message quotes the document.
      failed.push(node.position.start.line)
    }
  }
  return failed
}

/** A `srcset`'s first URL, split as the spec does. */
function first(srcset) {
  const url = srcset.replace(/^[\s,]+/, '').split(/\s/)[0]
  return url.replace(/,+$/, '')
}

/** One `src` per image, swapped for the app's mark. */
function marked(html) {
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const token = `ely-image-${crypto.randomUUID()}-`
  // Responsive sources go: the page carries one file per image.
  for (const source of doc.querySelectorAll('picture > source')) source.remove()
  const images = []
  for (const image of doc.querySelectorAll('img')) {
    const srcset = image.getAttribute('srcset')
    image.removeAttribute('srcset')
    image.removeAttribute('sizes')
    const src = image.getAttribute('src') || (srcset && first(srcset))
    if (!src) continue
    images.push(src)
    image.setAttribute('src', token + (images.length - 1))
  }
  return { html: doc.body.innerHTML, images, token }
}

/** KaTeX's failures: line and its message, never logged. */
function formulas(file) {
  return file.messages
    .filter((message) => message.source === 'rehype-katex')
    .map((message) => ({
      line: message.line,
      error: message.cause?.rawMessage ?? message.cause?.name ?? message.reason
    }))
}

/** `body` as HTML, images marked; the failed diagrams and formulas. */
export async function html(body) {
  const tree = markdown().parse(body)
  displayed(tree, body)
  const failed = await draw(tree)
  const page = unified()
    .use(remarkRehype, { allowDangerousHtml: true })
    .use(rehypeKatex)
    .use(rehypeStringify, { allowDangerousHtml: true })
  const file = new VFile()
  const done = page.stringify(await page.run(tree, file), file)
  return { ...marked(done), failed, formulas: formulas(file) }
}

/** `$$…$$` inline displays, as in the editor. */
function displayed(tree, body) {
  visit(tree, 'inlineMath', (node) => {
    const at = node.position?.start.offset ?? 0
    if (body.startsWith('$$', at)) {
      node.data.hProperties = { className: ['language-math', 'math-display'] }
    }
  })
}

/** `body` as plain text: blocks on lines, math as TeX. */
export async function text(body) {
  const page = unified().use(remarkRehype, { allowDangerousHtml: true })
  const hast = await page.run(markdown().parse(body))
  // Raw HTML alone goes through the DOM; blocks keep lines.
  visit(hast, 'raw', (node, index, parent) => {
    const parsed = new DOMParser().parseFromString(node.value, 'text/html').body
    const shown = fromDom(parsed).children
    parent.children.splice(index, 1, ...shown)
    return index + shown.length
  })
  return toText(hast)
}
