import { Lexer, Marked, type Tokens } from "marked";

export interface TocEntry {
  id: string;
  text: string;
  depth: 2 | 3;
}

export interface Rendered {
  html: string;
  toc: TocEntry[];
}

export interface FrontMatter {
  title: string;
  description: string;
  body: string;
}

/** `---\ntitle: …\ndescription: …\n---` at the top of a page, both keys optional. */
export function splitFrontMatter(src: string): FrontMatter {
  const m = /^---\n([\s\S]*?)\n---\n?/.exec(src);
  const meta: Record<string, string> = {};
  if (m) {
    for (const line of (m[1] ?? "").split("\n")) {
      const i = line.indexOf(":");
      if (i > 0) meta[line.slice(0, i).trim()] = line.slice(i + 1).trim();
    }
  }
  return {
    title: meta.title ?? "",
    description: meta.description ?? "",
    body: m ? src.slice(m[0].length) : src,
  };
}

export function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/<[^>]+>/g, "")
    .replace(/&[a-z]+;/g, "")
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-");
}

function stripHtml(html: string): string {
  return html.replace(/<[^>]+>/g, "");
}

const CALLOUT = /^\[!(NOTE|TIP|WARNING|IMPORTANT)\]\s*/;

function escapeAttr(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/"/g, "&quot;").replace(/</g, "&lt;");
}

/**
 * One parser per render so heading ids stay unique within a page and the
 * table of contents is collected while rendering.
 */
export function renderMarkdown(body: string): Rendered {
  const toc: TocEntry[] = [];
  const seen = new Map<string, number>();
  const marked = new Marked({
    gfm: true,
    renderer: {
      heading({ tokens, depth }: Tokens.Heading) {
        const text = this.parser.parseInline(tokens);
        let id = slugify(stripHtml(text));
        const n = seen.get(id) ?? 0;
        seen.set(id, n + 1);
        if (n > 0) id = `${id}-${n}`;
        if (depth === 2 || depth === 3) toc.push({ id, text: stripHtml(text), depth });
        return `<h${depth} id="${id}"><a class="anchor" href="#${id}" aria-hidden="true">#</a>${text}</h${depth}>\n`;
      },
      blockquote({ tokens }: Tokens.Blockquote) {
        const head = tokens.at(0);
        if (head?.type === "paragraph") {
          const first = head as Tokens.Paragraph;
          const m = CALLOUT.exec(first.raw);
          if (m) {
            const label = m[1] ?? "NOTE";
            const kind = label.toLowerCase();
            const text = first.text.replace(CALLOUT, "");
            const rest = this.parser.parse([
              {
                ...first,
                raw: first.raw.replace(CALLOUT, ""),
                text,
                tokens: Lexer.lexInline(text),
              },
              ...tokens.slice(1),
            ]);
            return `<div class="callout callout-${kind}"><div class="callout-title">${label.charAt(0) + label.slice(1).toLowerCase()}</div>${rest}</div>\n`;
          }
        }
        return `<blockquote>${this.parser.parse(tokens)}</blockquote>\n`;
      },
      image({ href, title, text }: Tokens.Image) {
        const cap = title ?? text;
        const cls = /\/android-[^/]*$/.test(href) ? ' class="portrait"' : "";
        return `<figure><img src="${escapeAttr(href)}" alt="${escapeAttr(text)}" loading="lazy"${cls} />${
          cap ? `<figcaption>${escapeAttr(cap)}</figcaption>` : ""
        }</figure>`;
      },
      link({ href, title, tokens }: Tokens.Link) {
        const text = this.parser.parseInline(tokens);
        const external = /^https?:\/\//.test(href);
        const t = title ? ` title="${escapeAttr(title)}"` : "";
        const rel = external ? ` target="_blank" rel="noreferrer"` : "";
        return `<a href="${escapeAttr(href)}"${t}${rel}>${text}</a>`;
      },
    },
  });
  const html = marked.parse(body, { async: false });
  return { html, toc };
}

/** Plain text of a page for the search index. */
export function plainText(body: string): string {
  return body
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/[#>*`_|-]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
