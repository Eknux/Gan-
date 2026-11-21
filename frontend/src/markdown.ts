// Markdown → güvenli HTML. marked ayrıştırır, DOMPurify XSS'e karşı temizler.
// Sözdizimi vurgulaması, HTML DOM'a yerleştirildikten sonra `highlightCodeBlocks`
// ile yapılır (marked'in sürümden sürüme değişen renderer API'sine bağlı kalmamak için).
//
// Gan, model çıktısını render ettiği için DOMPurify zorunludur: model
// `<img onerror=...>` gibi zararlı içerik döndürebilir.

import DOMPurify from "dompurify";
import { marked } from "marked";
import hljs from "highlight.js/lib/core";
import rust from "highlight.js/lib/languages/rust";
import typescript from "highlight.js/lib/languages/typescript";
import javascript from "highlight.js/lib/languages/javascript";
import python from "highlight.js/lib/languages/python";
import json from "highlight.js/lib/languages/json";
import bash from "highlight.js/lib/languages/bash";
import xml from "highlight.js/lib/languages/xml";
import css from "highlight.js/lib/languages/css";
import go from "highlight.js/lib/languages/go";
import java from "highlight.js/lib/languages/java";
import sql from "highlight.js/lib/languages/sql";
import yaml from "highlight.js/lib/languages/yaml";
import markdownLang from "highlight.js/lib/languages/markdown";
import shell from "highlight.js/lib/languages/shell";

// Yalnızca yaygın dilleri kaydet; bundle'ı ~900KB'den ~120KB'ye indir.
const LANGS: Record<string, unknown> = {
  rust, typescript, javascript, python, json, bash, xml, css, go, java, sql, yaml,
  markdown: markdownLang, shell,
};
for (const [name, lang] of Object.entries(LANGS)) hljs.registerLanguage(name, lang as never);
// `ts` ve `sh` takma adları.
hljs.registerAliases(["ts"], { languageName: "typescript" });
hljs.registerAliases(["js"], { languageName: "javascript" });
hljs.registerAliases(["py"], { languageName: "python" });
hljs.registerAliases(["sh", "zsh"], { languageName: "shell" });
hljs.registerAliases(["html", "xhtml"], { languageName: "xml" });
hljs.registerAliases(["yml"], { languageName: "yaml" });

marked.setOptions({ gfm: true, breaks: true });

const PURIFY_CONFIG = {
  ALLOWED_TAGS: [
    "p", "br", "hr", "strong", "em", "del", "code", "pre", "blockquote",
    "ul", "ol", "li", "a", "span", "div", "h1", "h2", "h3", "h4", "h5", "h6",
    "table", "thead", "tbody", "tr", "th", "td", "img",
  ],
  ALLOWED_ATTR: ["href", "title", "alt", "src", "class", "target", "rel"],
  ALLOW_DATA_ATTR: false,
};

/** Markdown metnini güvenli HTML dizesine çevir (vurgulama yapılmaz). */
export function renderMarkdown(md: string): string {
  const trimmed = md.trim();
  if (!trimmed) return "";
  const raw = marked.parse(trimmed, { async: false }) as string;
  // Dış bağlantıların yeni sekmede açılması için <a> etiketlerine target/rel ekle.
  const withTargets = raw.replace(/<a\s/gi, '<a target="_blank" rel="noopener noreferrer" ');
  return DOMPurify.sanitize(withTargets, PURIFY_CONFIG) as string;
}

/** Bir kapsayıcı içindeki tüm `pre > code` bloklarını vurgula.
 *  Her bloğun yanına "Kopyala" düğmesi ekler. */
export function highlightCodeBlocks(root: HTMLElement): void {
  const blocks = root.querySelectorAll<HTMLPreElement>("pre");
  for (const pre of blocks) {
    const code = pre.querySelector("code");
    if (!code || pre.parentElement?.classList.contains("code-block")) continue;
    // Vurgula.
    try {
      hljs.highlightElement(code);
    } catch {
      // Bilinmeyen dil: yoksay.
    }
    // Sar: dil etiketi + kopyala düğmesi.
    const wrapper = document.createElement("div");
    wrapper.className = "code-block";
    pre.parentNode?.insertBefore(wrapper, pre);
    wrapper.appendChild(pre);
    const copy = document.createElement("button");
    copy.className = "code-copy";
    copy.type = "button";
    copy.textContent = "Kopyala";
    copy.addEventListener("click", () => {
      void navigator.clipboard.writeText(code.textContent ?? "").then(() => {
        copy.textContent = "Kopyalandı";
        setTimeout(() => (copy.textContent = "Kopyala"), 1500);
      });
    });
    wrapper.appendChild(copy);
  }
}
