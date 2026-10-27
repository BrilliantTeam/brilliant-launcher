import { api, fileSrc, openUrl } from '../api.js';
import { emit, idle, on, store } from '../store.js';
import { $, clear, dialog, h, hexToHsl, hslToHex } from '../ui.js';

const BOARDS = [
    { key: 'general', label: '一般公告' },
    { key: 'event', label: '活動公告' },
    { key: 'update', label: '更新公告' }
];
const SITE = 'https://www.brilliantw.net/';
const pages = new Map();
let board = BOARDS[0].key;

const runText = (runs, skipMentions) =>
    runs.filter((run) => !run.e && !(skipMentions && run.c && run.t.startsWith('@'))).map((run) => run.t).join('');

export function titleOf(message) {
    for (const line of message.lines || []) {
        if (line.k === 'gap' || line.k === 'small') continue;
        const text = runText(line.r || [], true).replace(/\s+/g, ' ').trim();
        if (text) return text;
    }
    return '（附圖公告）';
}

const fmtDay = (ms) => {
    const date = new Date(ms);
    return `${date.getMonth() + 1}/${date.getDate()}`;
};

const SITE_ORIGIN = 'https://www.brilliantw.net';

export function mediaUrl(src) {
    const absolute = src.startsWith('/') ? SITE_ORIGIN + src : src;
    return absolute.startsWith(`${SITE_ORIGIN}/api/announcements/media?`) ? `${absolute}&still=1` : absolute;
}

const external = (url, ...children) =>
    h('a', { href: '#', title: url, onclick: (e) => { e.preventDefault(); openUrl(url); } }, ...children);

function runNode(run) {
    if (run.e) return h('img', { class: 'emoji', src: mediaUrl(run.e), alt: run.t, title: run.t });
    const styles = [
        run.b ? 'font-weight:700' : '',
        run.i ? 'font-style:italic' : '',
        run.u || run.s ? `text-decoration:${[run.u ? 'underline' : '', run.s ? 'line-through' : ''].filter(Boolean).join(' ')}` : '',
        run.c ? `color:${run.c}` : ''
    ].filter(Boolean).join(';');
    let node = h(run.code ? 'code' : 'span', styles ? { style: styles } : {}, run.t);
    if (run.sp) {
        node.classList.add('spoiler');
        node.addEventListener('click', () => node.classList.add('shown'));
    }
    return run.h ? external(run.h, node) : node;
}

function imageNode(line) {
    const width = line.sm ? Math.min(line.w || 160, 160) : line.w || 0;
    const img = h('img', { src: mediaUrl(line.src), alt: line.l || '', loading: 'lazy', style: width ? `max-width:min(100%, ${width}px)` : '' });
    const box = h('figure', { class: `news-img${line.sp ? ' spoiler' : ''}` }, img);
    if (line.v) box.append(h('span', { class: 'play' }, line.l || '影片'));
    else if (line.l) box.append(h('figcaption', {}, line.l));
    box.addEventListener('click', () => {
        if (box.classList.contains('spoiler') && !box.classList.contains('shown')) return box.classList.add('shown');
        if (line.href) openUrl(line.href);
    });
    if (line.href) box.classList.add('link');
    return box;
}

export function lineNode(line) {
    const runs = (line.r || []).map(runNode);
    let node;
    if (line.k === 'gap') node = h('div', { class: 'gap' });
    else if (line.k === 'img') node = imageNode(line);
    else if (line.k === 'code') node = h('pre', { class: 'code' }, runText(line.r || [], false));
    else if (line.k === 'poll') {
        node = h('div', { class: 'poll' },
            h('i', { style: `width:${Math.max(0, Math.min(100, line.p || 0))}%;background:${line.bar || 'var(--accent)'}` }),
            h('span', {}, ...runs));
    } else if (line.k === 'li') {
        node = h('div', { class: 'li', style: `padding-left:${18 + (line.d || 0) * 16}px` }, h('span', { class: 'bullet' }, line.n || '•'), ...runs);
    } else node = h('div', { class: /^h[1-3]$/.test(line.k) ? line.k : line.k === 'small' ? 'small' : 'p' }, ...runs);
    if (line.bar && line.k !== 'poll') node.style.borderLeft = `3px solid ${line.bar}`;
    if (line.q) node.classList.add('quote');
    return node;
}

function openMessage(message) {
    const body = h('div', { class: 'news-body' }, ...(message.lines || []).map(lineNode));
    const meta = h('div', { class: 'news-meta' }, `${message.author}　${new Date(message.time).toLocaleString('zh-TW')}${message.edited ? '　（已編輯）' : ''}`);
    dialog({
        title: titleOf(message),
        wide: true,
        content: h('div', {}, meta, body),
        buttons: [
            { label: '在官網查看', value: 'site' },
            { label: '關閉', value: null }
        ]
    }).then((value) => {
        if (value === 'site') openUrl(SITE);
    });
}

function renderList() {
    const list = clear($('#news-list'));
    const page = pages.get(board);
    if (page === undefined) {
        list.append(h('li', { class: 'note' }, '正在讀取公告⋯'));
        return;
    }
    if (!page || !page.messages?.length) {
        list.append(h('li', { class: 'note' }, page ? '目前沒有公告' : '暫時無法連線到官網'));
        return;
    }
    for (const message of page.messages.slice(0, 6)) {
        list.append(
            h(
                'li',
                {},
                h(
                    'button',
                    { type: 'button', onclick: () => openMessage(message) },
                    h('span', { class: 't' }, titleOf(message)),
                    h('span', { class: 'd' }, fmtDay(message.time))
                )
            )
        );
    }
}

function renderTabs() {
    clear($('#news-tabs')).append(
        ...BOARDS.map((item) =>
            h(
                'button',
                {
                    type: 'button',
                    role: 'tab',
                    'aria-selected': item.key === board ? 'true' : 'false',
                    onclick: () => select(item.key)
                },
                item.label
            )
        )
    );
}

async function select(key) {
    board = key;
    renderTabs();
    renderList();
    if (!pages.has(key) || pages.get(key) === null) {
        const page = await api.site_json({ path: `/api/announcements/plain?ch=${key}` }).catch(() => null);
        pages.set(key, page);
        if (board === key) renderList();
    }
}

function preload(url) {
    return new Promise((resolve, reject) => {
        const image = new Image();
        image.onload = () => resolve(url);
        image.onerror = reject;
        image.src = url;
    });
}

let slides = null;
let slideTimer = 0;
let slideIndex = 0;
let front = 0;
let appliedBackground = null;
let generation = 0;
let loadingSlide = false;
let retryTimer = 0;

function setBgPalette(palette) {
    if (store.bgAccent === palette.accent && JSON.stringify(store.bgPalette) === JSON.stringify(palette)) return;
    store.bgAccent = palette.accent;
    store.bgPalette = palette;
    emit('bg-color');
}

const wantsBgColors = (settings) => !!(settings?.accentFromBg || settings?.autoPalette || settings?.theme === 'background');

export function paletteFromPixels(data) {
    const buckets = Array.from({ length: 12 }, () => ({ w: 0, hue: 0, sat: 0 }));
    let x = 0;
    let y = 0;
    let satSum = 0;
    let lightSum = 0;
    let count = 0;
    for (let i = 0; i < data.length; i += 4) {
        const hex = `#${[data[i], data[i + 1], data[i + 2]].map((v) => v.toString(16).padStart(2, '0')).join('')}`;
        const [hue, sat, light] = hexToHsl(hex);
        count++;
        lightSum += light;
        satSum += sat;
        x += Math.cos((hue * Math.PI) / 180) * sat;
        y += Math.sin((hue * Math.PI) / 180) * sat;
        const weight = (sat / 100) ** 2 * (1 - Math.abs(light - 50) / 50);
        if (weight < 0.02) continue;
        const bucket = buckets[Math.floor(hue / 30) % 12];
        bucket.w += weight;
        bucket.hue += hue * weight;
        bucket.sat += sat * weight;
    }
    if (!count) return null;
    const ranked = buckets.filter((b) => b.w > 0).map((b) => ({ hue: b.hue / b.w, sat: b.sat / b.w, w: b.w })).sort((a, b) => b.w - a.w);
    const base = Math.round(((Math.atan2(y, x) * 180) / Math.PI + 360) % 360);
    const first = ranked[0] ?? { hue: base, sat: 60 };
    const far = (a, b) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b)) >= 45;
    const second = ranked.find((b) => far(b.hue, first.hue)) ?? { hue: (first.hue + 150) % 360, sat: first.sat };
    const vivid = (b) => Math.max(55, Math.min(85, Math.round(b.sat)));
    return {
        accent: hslToHex(Math.round(first.hue), vivid(first), 62),
        link: hslToHex(Math.round(second.hue), vivid(second), 64),
        hue: base,
        sat: Math.round(satSum / count),
        light: Math.round(lightSum / count)
    };
}

function sampleColor(source) {
    try {
        const canvas = document.createElement('canvas');
        canvas.width = canvas.height = 32;
        const ctx = canvas.getContext('2d', { willReadFrequently: true });
        ctx.drawImage(source, 0, 0, 32, 32);
        const palette = paletteFromPixels(ctx.getImageData(0, 0, 32, 32).data);
        if (palette) setBgPalette(palette);
    } catch {}
}

function sampleUrl(url) {
    const image = new Image();
    image.crossOrigin = 'anonymous';
    image.onload = () => sampleColor(image);
    image.src = url;
}

function solidPalette(hex) {
    const [hue, sat, light] = hexToHsl(hex);
    return { accent: hslToHex(hue, Math.max(55, Math.min(85, sat)), 62), link: hslToHex((hue + 150) % 360, Math.max(55, Math.min(85, sat)), 64), hue, sat, light };
}

async function showImage(url, credit, gen) {
    try {
        await preload(url);
    } catch {
        return;
    }
    if (gen !== generation) return;
    if (wantsBgColors(store.settings)) sampleUrl(url);
    const layers = [$('#bg-a'), $('#bg-b')];
    front = 1 - front;
    layers[front].style.backgroundImage = `url("${url}")`;
    layers[front].classList.add('show', 'top');
    const back = layers[1 - front];
    back.classList.remove('top');
    setTimeout(() => back.classList.remove('show'), 1300);
    $('#bg-credit').textContent = credit;
}

async function nextSlide(gen) {
    if (!slides?.length || idle() || loadingSlide) return;
    const slide = slides[slideIndex++ % slides.length];
    loadingSlide = true;
    try {
        await showImage(slide.image, [slide.description, slide.author && `攝影：${slide.author}`].filter(Boolean).join('｜'), gen);
    } finally {
        loadingSlide = false;
    }
}

async function applyBackground() {
    const settings = store.settings ?? {};
    const solid = settings.bgSource === 'color';
    const custom = solid ? '' : settings.background || '';
    const pin = solid || custom ? '' : settings.bgPin || '';
    const seconds = settings.slideSeconds ?? 15;
    const key = `${solid ? `color:${settings.bgColor}` : custom}|${pin}|${seconds}|${wantsBgColors(settings) ? 1 : 0}`;
    if (key === appliedBackground) return;
    const sourceChanged = !appliedBackground || appliedBackground.split('|').slice(0, 2).join('|') !== key.split('|').slice(0, 2).join('|') || (wantsBgColors(settings) && !appliedBackground.endsWith('|1'));
    appliedBackground = key;
    const gen = ++generation;
    loadingSlide = false;
    clearInterval(slideTimer);
    clearTimeout(retryTimer);
    const video = $('#bg-video');
    const isVideo = !solid && /\.(mp4|webm)$/i.test(custom);
    video.classList.toggle('show', isVideo);
    video.classList.toggle('top', isVideo);
    if (isVideo) {
        video.src = fileSrc(custom);
        video.crossOrigin = 'anonymous';
        if (wantsBgColors(settings)) video.addEventListener('loadeddata', () => sampleColor(video), { once: true });
        video.play().catch(() => {});
        $('#bg-credit').textContent = '';
        return;
    }
    video.pause();
    video.removeAttribute('src');
    video.load();
    if (solid) {
        $('#bg-credit').textContent = '';
        if (/^#[0-9a-f]{6}$/i.test(settings.bgColor ?? '')) setBgPalette(solidPalette(settings.bgColor));
        return;
    }
    if (custom) return showImage(fileSrc(custom), '', gen);
    if (!slides) {
        const data = await api.site_json({ path: '/api/launcher/hero' }).catch(() => null);
        if (gen !== generation) return;
        if (!Array.isArray(data?.slides)) {
            retryTimer = setTimeout(() => {
                appliedBackground = null;
                applyBackground();
            }, 30_000);
            return;
        }
        slides = data.slides.sort(() => Math.random() - 0.5);
    }
    const pinned = pin && slides.find((slide) => slide.image === pin);
    if (pinned) return showImage(pinned.image, [pinned.description, pinned.author && `攝影：${pinned.author}`].filter(Boolean).join('｜'), gen);
    if (sourceChanged) await nextSlide(gen);
    if (gen !== generation) return;
    if (seconds > 0 && slides.length > 1) slideTimer = setInterval(() => nextSlide(gen), seconds * 1000);
}

function syncVideo() {
    const video = $('#bg-video');
    if (!video.getAttribute('src')) return;
    if (idle()) video.pause();
    else video.play().catch(() => {});
}

export function initNews() {
    $('.news').classList.add('glass');
    select(board);
    applyBackground();
    on('settings', applyBackground);
    on('instances', syncVideo);
    document.addEventListener('visibilitychange', syncVideo);
}
