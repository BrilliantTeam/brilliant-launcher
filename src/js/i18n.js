export const LANGUAGES = [['zh-TW', '繁體中文'], ['zh-CN', '简体中文'], ['en', 'English']];

const SKIP = '#console > :not(.log-note), #news-list, .news-list, .news-body, textarea, script, style, code, pre, [data-no-i18n], .inst-title, .pick-title, .inst-page-name h1, #chip-name, .account-card .name, .palette-input';
const ATTRS = ['title', 'placeholder', 'aria-label'];

function greedy(text, map, max) {
    let out = '';
    for (let i = 0; i < text.length;) {
        let hit = null;
        for (let len = Math.min(max, text.length - i); len > 0; len--) {
            const piece = text.substr(i, len);
            if (map[piece] !== undefined) {
                hit = [piece, map[piece]];
                break;
            }
        }
        if (hit) {
            out += hit[1];
            i += hit[0].length;
        } else {
            out += text[i++];
        }
    }
    return out;
}

const longest = (map) => Math.max(1, ...Object.keys(map).map((k) => k.length));

let convert = (text) => text;

export async function prepare(language) {
    if (language === 'zh-CN') {
        const { TW_TO_CN, T_TO_S } = await import('./i18n/hans.js');
        const [a, b] = [longest(TW_TO_CN), longest(T_TO_S)];
        convert = (text) => greedy(greedy(text, TW_TO_CN, a), T_TO_S, b);
    } else if (language === 'en') {
        const { EXACT, PATTERNS } = await import('./i18n/en.js');
        const CJK = /[　-〿一-鿿＀-￯]/;
        const compiled = PATTERNS.map(([source, target]) => {
            const fragments = source.split(/\{\d+\}/).map((f) => f.trim()).filter((f) => CJK.test(f));
            const weak = /^\{\d+\}/.test(source) && (source.replace(/\{\d+\}/g, '').match(/[一-鿿]/g) ?? []).length <= 2;
            return { re: new RegExp(`^${source.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\\\{(\d+)\\\}/g, '(.*?)')}$`, 's'), target, fragments, weak, bar: source.includes('｜') };
        });
        const exact = (text) => {
            if (EXACT[text] !== undefined) return EXACT[text];
            const core = text.trim();
            if (core !== text && EXACT[core] !== undefined) return text.replace(core, EXACT[core]);
            const flat = core.replace(/\s+/g, ' ');
            return flat !== core && EXACT[flat] !== undefined ? text.replace(core, EXACT[flat]) : null;
        };
        const one = (text) => {
            if (!CJK.test(text)) return text;
            const hit = exact(text);
            if (hit !== null) return hit;
            const core = text.trim();
            for (const p of compiled) {
                for (const subject of core === text ? [text] : [text, core]) {
                    const m = p.re.exec(subject);
                    if (!m) continue;
                    const raw = m.slice(1).map((g) => g ?? '');
                    if (raw.some((g) => p.fragments.some((f) => g.includes(f)) || (!p.bar && g.includes('｜')))) continue;
                    const groups = raw.map(one);
                    if (p.weak && groups.some((g) => CJK.test(g))) continue;
                    const out = p.target.replace(/\{(\d+)\}/g, (_, i) => groups[Number(i)] ?? '');
                    return subject === text ? out : text.replace(core, out);
                }
            }
            if (text.includes('｜')) {
                const parts = text.split('｜').map(one);
                if (parts.join('｜') !== text) return parts.join('｜');
            }
            if (text.includes('、')) {
                const parts = text.split('、').map(one);
                if (!parts.some((p) => CJK.test(p))) return parts.join(', ');
            }
            return text;
        };
        convert = one;
    } else {
        return false;
    }
    return true;
}

export const translate = (text) => convert(text);

const seen = new WeakMap();

function textNode(node) {
    const value = node.nodeValue;
    const record = seen.get(node);
    if (record && record.out === value) return;
    const out = convert(value);
    seen.set(node, { out });
    if (out !== value) node.nodeValue = out;
}

function attributes(el) {
    const record = seen.get(el) ?? {};
    for (const attr of ATTRS) {
        const value = el.getAttribute(attr);
        if (!value || record[attr] === value) continue;
        const out = convert(value);
        record[attr] = out;
        if (out !== value) el.setAttribute(attr, out);
    }
    seen.set(el, record);
}

function walk(root) {
    if (root.nodeType === Node.TEXT_NODE) {
        if (!root.parentElement?.closest(SKIP)) textNode(root);
        return;
    }
    if (root.nodeType !== Node.ELEMENT_NODE) return;
    if (root.matches('textarea')) return attributes(root);
    if (root.closest(SKIP)) return;
    attributes(root);
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => {
            if (node.nodeType !== Node.ELEMENT_NODE || !node.matches(SKIP)) return NodeFilter.FILTER_ACCEPT;
            if (node.matches('textarea')) attributes(node);
            return NodeFilter.FILTER_REJECT;
        }
    });
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        if (node.nodeType === Node.TEXT_NODE) textNode(node);
        else attributes(node);
    }
}

export async function startI18n(language) {
    if (!(await prepare(language))) return;
    document.documentElement.lang = language === 'zh-CN' ? 'zh-Hans' : language === 'en' ? 'en' : 'zh-Hant';
    walk(document.documentElement);
    new MutationObserver((records) => {
        for (const r of records) {
            if (r.type === 'childList') r.addedNodes.forEach(walk);
            else if (r.type === 'characterData') { if (!r.target.parentElement?.closest(SKIP)) textNode(r.target); }
            else if (r.type === 'attributes' && (r.target.matches('textarea') || !r.target.closest(SKIP))) attributes(r.target);
        }
    }).observe(document.documentElement, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
}
