export interface DiffRow {
  old: number | null
  cur: number | null
  type: "ctx" | "add" | "del"
  text: string
}

export interface DiffToken {
  t: "txt" | "cm" | "str" | "num" | "kw" | "fn"
  v: string
}

export interface CiteRef {
  n: number
  label: string
  host: string
  url: string
}

export type CitationSegment =
  | { kind: "text"; text: string }
  | { kind: "cite"; index: number }

const KEYWORDS = new Set([
  "export", "function", "return", "const", "let", "var", "if", "else", "throw", "new",
  "import", "from", "async", "await", "class", "extends", "typeof", "void", "true",
  "false", "null", "undefined", "for", "while", "switch", "case", "break", "continue",
  "try", "catch", "finally", "this", "super", "static", "type", "interface", "enum", "as", "of", "in",
  "pub", "fn", "struct", "impl", "match", "use", "mod", "crate", "mut", "self", "Self", "where",
])

function isWs(ch: string): boolean {
  return ch === " " || ch === "\t" || ch === "\r" || ch === "\n"
}

function isDigitChar(ch: string): boolean {
  const code = ch.charCodeAt(0)
  return code >= 48 && code <= 57
}

function isIdentStart(ch: string): boolean {
  const code = ch.charCodeAt(0)
  return (
    (code >= 65 && code <= 90) ||
    (code >= 97 && code <= 122) ||
    ch === "_" ||
    ch === "$"
  )
}

function isIdentCont(ch: string): boolean {
  return isIdentStart(ch) || isDigitChar(ch)
}

interface RawTok {
  kind: "txt" | "cm" | "str" | "num" | "id"
  v: string
}

export function tokenizeDiffLine(line: string): DiffToken[] {
  const raw: RawTok[] = []
  let i = 0
  const len = line.length

  while (i < len) {
    const ch = line[i]

    if (isWs(ch)) {
      let j = i + 1
      while (j < len && isWs(line[j])) j++
      raw.push({ kind: "txt", v: line.slice(i, j) })
      i = j
      continue
    }

    if (ch === "/" && line[i + 1] === "/") {
      raw.push({ kind: "cm", v: line.slice(i) })
      break
    }

    if (ch === "/" && line[i + 1] === "*") {
      const end = line.indexOf("*/", i + 2)
      const stop = end === -1 ? len : end + 2
      raw.push({ kind: "cm", v: line.slice(i, stop) })
      i = stop
      continue
    }

    if (ch === "#" && (i === 0 || isWs(line[i - 1]))) {
      raw.push({ kind: "cm", v: line.slice(i) })
      break
    }

    if (ch === '"' || ch === "'" || ch === "`") {
      const quote = ch
      let j = i + 1
      while (j < len) {
        if (line[j] === "\\") {
          j += 2
          continue
        }
        if (line[j] === quote) {
          j++
          break
        }
        j++
      }
      raw.push({ kind: "str", v: line.slice(i, j) })
      i = j
      continue
    }

    if (isDigitChar(ch)) {
      let j = i + 1
      while (j < len && (isDigitChar(line[j]) || line[j] === ".")) j++
      raw.push({ kind: "num", v: line.slice(i, j) })
      i = j
      continue
    }

    if (isIdentStart(ch)) {
      let j = i + 1
      while (j < len && isIdentCont(line[j])) j++
      raw.push({ kind: "id", v: line.slice(i, j) })
      i = j
      continue
    }

    raw.push({ kind: "txt", v: ch })
    i++
  }

  const out: DiffToken[] = []
  for (let idx = 0; idx < raw.length; idx++) {
    const cur = raw[idx]
    if (cur.kind !== "id") {
      out.push({ t: cur.kind, v: cur.v })
      continue
    }
    if (KEYWORDS.has(cur.v)) {
      out.push({ t: "kw", v: cur.v })
      continue
    }
    let look = idx + 1
    while (look < raw.length && raw[look].kind === "txt" && raw[look].v.trim().length === 0) {
      look++
    }
    const next = raw[look]
    out.push({ t: next && next.v.startsWith("(") ? "fn" : "txt", v: cur.v })
  }
  return out
}

function parseHunkHeaderNumbers(header: string): { oldStart: number; newStart: number } | null {
  if (!header.startsWith("@@")) return null
  const endAt = header.indexOf("@@", 2)
  if (endAt === -1) return null
  const inner = header.slice(2, endAt).trim()
  const parts = inner.split(" ")
  let oldStart = 1
  let newStart = 1
  for (const part of parts) {
    if (part.startsWith("-")) {
      const numPart = part.slice(1).split(",")[0]
      const parsed = Number.parseInt(numPart, 10)
      if (!Number.isNaN(parsed)) oldStart = parsed
    } else if (part.startsWith("+")) {
      const numPart = part.slice(1).split(",")[0]
      const parsed = Number.parseInt(numPart, 10)
      if (!Number.isNaN(parsed)) newStart = parsed
    }
  }
  return { oldStart, newStart }
}

export function parseUnifiedDiff(patch: string): { file?: string; rows: DiffRow[] } {
  const lines = patch.split("\n")
  const rows: DiffRow[] = []
  let detectedFile: string | undefined
  let oldLn = 1
  let newLn = 1

  for (const rawLine of lines) {
    const line = rawLine.endsWith("\r") ? rawLine.slice(0, -1) : rawLine
    if (line.startsWith("diff --git ")) {
      const bIdx = line.indexOf(" b/")
      if (bIdx !== -1) detectedFile = line.slice(bIdx + 3).trim()
      continue
    }
    if (line.startsWith("--- ")) {
      if (!detectedFile && !line.startsWith("--- /dev/null")) {
        const cleaned = line.slice(4).trim()
        detectedFile = cleaned.startsWith("a/") ? cleaned.slice(2) : cleaned
      }
      continue
    }
    if (line.startsWith("+++ ")) {
      if (!line.startsWith("+++ /dev/null")) {
        const cleaned = line.slice(4).trim()
        detectedFile = cleaned.startsWith("b/") ? cleaned.slice(2) : cleaned
      }
      continue
    }
    if (
      line.startsWith("index ") ||
      line.startsWith("new file mode") ||
      line.startsWith("deleted file mode")
    ) {
      continue
    }
    const hunk = parseHunkHeaderNumbers(line)
    if (hunk) {
      oldLn = hunk.oldStart
      newLn = hunk.newStart
      continue
    }
    if (line.startsWith("+")) {
      rows.push({ old: null, cur: newLn++, type: "add", text: line.slice(1) })
    } else if (line.startsWith("-")) {
      rows.push({ old: oldLn++, cur: null, type: "del", text: line.slice(1) })
    } else if (line.startsWith(" ")) {
      rows.push({ old: oldLn++, cur: newLn++, type: "ctx", text: line.slice(1) })
    } else if (line.length > 0 && !line.startsWith("\\ No newline")) {
      rows.push({ old: oldLn++, cur: newLn++, type: "ctx", text: line })
    }
  }

  return { file: detectedFile, rows }
}

function isAsciiDigits(value: string): boolean {
  if (value.length === 0) return false
  for (let i = 0; i < value.length; i++) {
    const code = value.charCodeAt(i)
    if (code < 48 || code > 57) return false
  }
  return true
}

export function parseCitationSegments(input: string): CitationSegment[] {
  const segments: CitationSegment[] = []
  let cursor = 0
  let textStart = 0

  while (cursor < input.length) {
    if (input[cursor] === "[") {
      const closeIdx = input.indexOf("]", cursor + 1)
      if (closeIdx !== -1 && closeIdx - cursor <= 5) {
        const inner = input.slice(cursor + 1, closeIdx)
        const after = input[closeIdx + 1] ?? ""
        if (isAsciiDigits(inner) && after !== "(" && after !== ":") {
          if (cursor > textStart) {
            segments.push({ kind: "text", text: input.slice(textStart, cursor) })
          }
          segments.push({ kind: "cite", index: Number.parseInt(inner, 10) })
          cursor = closeIdx + 1
          textStart = cursor
          continue
        }
      }
    }
    cursor++
  }

  if (textStart < input.length) {
    segments.push({ kind: "text", text: input.slice(textStart) })
  }
  return segments
}

function safeHostname(rawUrl: string): string {
  try {
    const h = new URL(rawUrl).hostname
    return h.startsWith("www.") ? h.slice(4) : h
  } catch {
    return "source"
  }
}

function parseReferenceLine(line: string): CiteRef | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith("[")) return null
  const closeBracket = trimmed.indexOf("]")
  if (closeBracket <= 1) return null
  const numStr = trimmed.slice(1, closeBracket)
  if (!isAsciiDigits(numStr)) return null
  const n = Number.parseInt(numStr, 10)
  let rest = trimmed.slice(closeBracket + 1).trim()
  if (rest.startsWith(":")) rest = rest.slice(1).trim()
  if (rest.startsWith("-")) rest = rest.slice(1).trim()

  if (rest.startsWith("[")) {
    const labelEnd = rest.indexOf("](")
    const urlEnd = labelEnd !== -1 ? rest.indexOf(")", labelEnd + 2) : -1
    if (labelEnd !== -1 && urlEnd !== -1) {
      const label = rest.slice(1, labelEnd).trim()
      const url = rest.slice(labelEnd + 2, urlEnd).trim()
      if (url.startsWith("http://") || url.startsWith("https://")) {
        return { n, label: label || safeHostname(url), host: safeHostname(url), url }
      }
    }
  }

  const httpIdx =
    rest.indexOf("https://") !== -1 ? rest.indexOf("https://") : rest.indexOf("http://")
  if (httpIdx === -1) return null

  const beforeUrl = rest.slice(0, httpIdx).trim().replaceAll("—", "").replaceAll("-", "").trim()
  const fromUrl = rest.slice(httpIdx).trim()
  const spaceIdx = fromUrl.indexOf(" ")
  const url = (spaceIdx === -1 ? fromUrl : fromUrl.slice(0, spaceIdx)).trim()
  const afterUrl = (spaceIdx === -1 ? "" : fromUrl.slice(spaceIdx + 1))
    .trim()
    .replaceAll('"', "")
    .replaceAll("'", "")
  const host = safeHostname(url)
  const label = beforeUrl || afterUrl || host
  return { n, label, host, url }
}

export function extractMarkdownCitations(markdown: string): {
  bodyText: string
  refs: CiteRef[]
  hasInlineCitations: boolean
} {
  const lines = markdown.split("\n")
  const refs: CiteRef[] = []
  const keptLines: string[] = []

  for (const line of lines) {
    const parsedRef = parseReferenceLine(line)
    if (parsedRef) {
      if (!refs.some((existing) => existing.n === parsedRef.n)) {
        refs.push(parsedRef)
      }
    } else {
      keptLines.push(line)
    }
  }

  const bodyText = keptLines.join("\n").trim()
  const segments = parseCitationSegments(bodyText)
  const hasInlineCitations =
    refs.length > 0 && segments.some((seg) => seg.kind === "cite")

  return { bodyText, refs, hasInlineCitations }
}
