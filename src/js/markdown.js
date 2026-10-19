import { h } from './ui.js';

const ENTITIES = { '&amp;': '&', '&lt;': '<', '&gt;': '>', '&quot;': '"', '&#39;': "'", '&nbsp;': ' ' };
const IMAGE_HOST = 'https://cdn.modrinth.com/';

function fromHtml(text) {
    return text
        .replace(/\r\n?/g, '\n')
        .replace(/<!--[\s\S]*?-->/g, '')
        .replace(/<img\b[^>]*\bsrc=["']([^"']+)["'][^>]*>/gi, '![]($1)')
        .replace(/<br\s*\/?>/gi, '\n')
        .replace(/<\/?(p|div|center|details|summary)\b[^>]*>/gi, '\n\n')
        .replace(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/gi, (_, n, inner) => `\n\n${'#'.repeat(Number(n))} ${inner.trim()}\n\n`)
        .replace(/<a\b[^>]*\bhref=["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi, '[$2]($1)')
        .replace(/<\/?(b|strong)>/gi, '**')
        .replace(/<\/?(i|em)>/gi, '*')
        .replace(/<li\b[^>]*>/gi, '\n- ')
        .replace(/<[^>]+>/g, '')
        .replace(/&(amp|lt|gt|quot|#39|nbsp);/g, (m) => ENTITIES[m]);
}

export function parseInline(text) {
    const pattern = /\[(!\[[^\]]*\]\([^)]+\))\]\(([^)\s]+)[^)]*\)|!\[([^\]]*)\]\(([^)\s]+)[^)]*\)|\[([^\]]+)\]\(([^)\s]+)[^)]*\)|`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*\s][^*]*)\*|(https?:\/\/[^\s<>()]+)/g;
    const out = [];
    let last = 0;
    for (const m of text.matchAll(pattern)) {
        if (m.index > last) out.push({ t: 'text', v: text.slice(last, m.index) });
        if (m[1]) {
            const img = /!\[([^\]]*)\]\(([^)\s]+)/.exec(m[1]);
            if (img[2].startsWith(IMAGE_HOST)) out.push({ t: 'img', alt: img[1], src: img[2], href: m[2] });
        } else if (m[4] !== undefined) {
            if (m[4].startsWith(IMAGE_HOST)) out.push({ t: 'img', alt: m[3], src: m[4] });
        } else if (m[5]) out.push({ t: 'link', v: m[5], href: m[6] });
        else if (m[7]) out.push({ t: 'code', v: m[7] });
        else if (m[8] || m[9]) out.push({ t: 'b', v: m[8] || m[9] });
        else if (m[10]) out.push({ t: 'i', v: m[10] });
        else if (m[11]) out.push({ t: 'link', v: m[11], href: m[11] });
        last = m.index + m[0].length;
    }
    if (last < text.length) out.push({ t: 'text', v: text.slice(last) });
    return out;
}

export function parseMarkdown(source) {
    const blocks = [];
    let para = [];
    const flush = () => {
        const text = para.join(' ').trim();
        if (text) blocks.push({ t: 'p', v: text });
        para = [];
    };
    const lines = String(source || '')
        .replace(/\r\n?/g, '\n')
        .split(/(^[ \t]*```[^\n]*\n[\s\S]*?^[ \t]*```[ \t]*$)/m)
        .map((part, i) => (i % 2 ? part : fromHtml(part)))
        .join('')
        .split('\n');
    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (/^\s*```/.test(line)) {
            flush();
            const code = [];
            while (++i < lines.length && !/^\s*```/.test(lines[i])) code.push(lines[i]);
            blocks.push({ t: 'code', v: code.join('\n') });
            continue;
        }
        const heading = /^\s*(#{1,6})\s+(.*?)\s*#*\s*$/.exec(line);
        const item = /^\s*([-*+]|\d+[.)])\s+(.*)$/.exec(line);
        const quote = /^\s*>\s?(.*)$/.exec(line);
        if (!line.trim()) flush();
        else if (/^\s*([-*_])(\s*\1){2,}\s*$/.test(line)) {
            flush();
            blocks.push({ t: 'hr' });
        } else if (heading) {
            flush();
            if (heading[2]) blocks.push({ t: 'h', level: heading[1].length, v: heading[2] });
        } else if (item) {
            flush();
            const ordered = /\d/.test(item[1]);
            const prev = blocks[blocks.length - 1];
            if (prev?.t === 'list' && prev.ordered === ordered) prev.items.push(item[2]);
            else blocks.push({ t: 'list', ordered, items: [item[2]] });
        } else if (quote) {
            flush();
            const prev = blocks[blocks.length - 1];
            if (prev?.t === 'quote') prev.v += ` ${quote[1]}`;
            else blocks.push({ t: 'quote', v: quote[1] });
        } else if (/^\s*\|?\s*:?-{3,}/.test(line) && line.includes('|')) {
            continue;
        } else if (line.trim().startsWith('|')) {
            flush();
            blocks.push({ t: 'p', v: line.trim().replace(/^\||\|$/g, '').split('|').map((c) => c.trim()).filter(Boolean).join('｜') });
        } else {
            para.push(line.trim());
        }
    }
    flush();
    return blocks;
}

function inline(text, open) {
    return parseInline(text).map((tok) => {
        if (tok.t === 'text') return tok.v;
        if (tok.t === 'code') return h('code', {}, tok.v);
        if (tok.t === 'b') return h('strong', {}, tok.v);
        if (tok.t === 'i') return h('em', {}, tok.v);
        if (tok.t === 'img') {
            const img = h('img', { src: tok.src, alt: tok.alt, loading: 'lazy' });
            return tok.href && /^https?:\/\//.test(tok.href) ? h('a', { href: tok.href, onclick: (e) => { e.preventDefault(); open(tok.href); } }, img) : img;
        }
        if (!/^https?:\/\//.test(tok.href)) return tok.v;
        return h('a', { href: tok.href, title: tok.href, onclick: (e) => { e.preventDefault(); open(tok.href); } }, tok.v);
    });
}

export function renderMarkdown(source, open) {
    return h('div', { class: 'md' }, ...parseMarkdown(source).map((block) => {
        if (block.t === 'h') return h(`h${Math.min(6, block.level + 2)}`, {}, ...inline(block.v, open));
        if (block.t === 'list') return h(block.ordered ? 'ol' : 'ul', {}, ...block.items.map((it) => h('li', {}, ...inline(it, open))));
        if (block.t === 'quote') return h('blockquote', {}, ...inline(block.v, open));
        if (block.t === 'code') return h('pre', {}, block.v);
        if (block.t === 'hr') return h('hr');
        return h('p', {}, ...inline(block.v, open));
    }));
}
