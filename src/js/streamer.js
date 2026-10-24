import { on, store } from './store.js';

const ATTRS = ['title', 'aria-label', 'placeholder', 'alt'];
const SKIP = 'script, style, textarea, input';
let active = false;
let rules = [];
const touched = new Map();

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

function buildRules() {
    const names = [...new Set((store.accounts ?? []).map((a) => a.name).filter((n) => n && n.length >= 3))].sort((a, b) => b.length - a.length);
    const uuids = (store.accounts ?? []).map((a) => a.uuid).filter(Boolean);
    rules = [
        [/[A-Za-z]:[\\/]Users[\\/][^\\/\s"'<>|]+/gi, 'C:\\Users\\•••'],
        [/\/(home|Users)\/[^/\s"'<>]+/g, '/$1/•••'],
        [/[\w.+-]+@[\w-]+\.[\w.-]+/g, '•••@•••'],
        [/\b(?:\d{1,3}\.){3}\d{1,3}(?::\d+)?\b/g, '•.•.•.•'],
        ...uuids.flatMap((u) => [[new RegExp(escape(u), 'gi'), '••••••••'], [new RegExp(escape(u.replace(/-/g, '')), 'gi'), '••••••••']]),
        ...names.map((n) => [new RegExp(escape(n), 'g'), '•••'])
    ];
}

export const maskText = (text) => (active && text ? rules.reduce((out, [re, to]) => out.replace(re, to), text) : text);

function maskNode(node) {
    const value = node.nodeValue;
    const out = maskText(value);
    if (out === value) return;
    if (!touched.has(node)) touched.set(node, value);
    node.nodeValue = out;
}

function maskAttrs(el) {
    for (const attr of ATTRS) {
        const value = el.getAttribute(attr);
        if (!value) continue;
        const out = maskText(value);
        if (out === value) continue;
        const key = `${attr}\u0000`;
        const saved = touched.get(el) ?? {};
        if (!(key in saved)) saved[key] = value;
        touched.set(el, saved);
        el.setAttribute(attr, out);
    }
}

function walk(root) {
    if (root.nodeType === Node.TEXT_NODE) {
        if (!root.parentElement?.closest(SKIP)) maskNode(root);
        return;
    }
    if (root.nodeType !== Node.ELEMENT_NODE) return;
    maskAttrs(root);
    if (root.matches(SKIP)) return;
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT, {
        acceptNode: (node) => (node.nodeType === Node.ELEMENT_NODE && node.matches('script, style') ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT)
    });
    for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        if (node.nodeType === Node.TEXT_NODE) {
            if (!node.parentElement?.closest(SKIP)) maskNode(node);
        } else maskAttrs(node);
    }
}

function restore() {
    for (const [node, saved] of touched) {
        if (!node.isConnected) continue;
        if (typeof saved === 'string') node.nodeValue = saved;
        else for (const [key, value] of Object.entries(saved)) node.setAttribute(key.slice(0, -1), value);
    }
    touched.clear();
}

const observer = new MutationObserver((records) => {
    if (!active) return;
    for (const r of records) {
        if (r.type === 'childList') r.addedNodes.forEach(walk);
        else if (r.type === 'characterData') { if (!r.target.parentElement?.closest(SKIP)) maskNode(r.target); }
        else if (r.type === 'attributes') maskAttrs(r.target);
    }
});

function apply() {
    const want = !!store.settings?.streamerMode;
    buildRules();
    if (want && !active) {
        active = true;
        walk(document.body);
        observer.observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ATTRS });
    } else if (!want && active) {
        active = false;
        observer.disconnect();
        restore();
    } else if (active) {
        walk(document.body);
    }
}

export function initStreamer() {
    on('settings', apply);
    on('accounts', apply);
    apply();
}
