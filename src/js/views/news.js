import { api, fileSrc, openUrl } from '../api.js';
import { idle, on, store } from '../store.js';
import { $, clear, dialog, h } from '../ui.js';

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

async function showImage(url, credit) {
    try {
        await preload(url);
    } catch {
        return;
    }
    const layers = [$('#bg-a'), $('#bg-b')];
    front = 1 - front;
    layers[front].style.backgroundImage = `url("${url}")`;
    layers[front].classList.add('show', 'top');
    const back = layers[1 - front];
    back.classList.remove('top');
    setTimeout(() => back.classList.remove('show'), 1300);
    $('#bg-credit').textContent = credit;
}

function nextSlide() {
    if (!slides?.length || idle()) return;
    const slide = slides[slideIndex++ % slides.length];
    return showImage(slide.image, [slide.description, slide.author && `攝影：${slide.author}`].filter(Boolean).join('｜'));
}

async function applyBackground() {
    const custom = store.settings?.background || '';
    const seconds = store.settings?.slideSeconds ?? 15;
    const key = `${custom}|${seconds}`;
    if (key === appliedBackground) return;
    const sourceChanged = !appliedBackground || appliedBackground.split('|')[0] !== custom;
    appliedBackground = key;
    clearInterval(slideTimer);
    const video = $('#bg-video');
    const isVideo = /\.(mp4|webm)$/i.test(custom);
    video.classList.toggle('show', isVideo);
    video.classList.toggle('top', isVideo);
    if (isVideo) {
        video.src = fileSrc(custom);
        video.play().catch(() => {});
        $('#bg-credit').textContent = '';
        return;
    }
    video.pause();
    video.removeAttribute('src');
    video.load();
    if (custom) return showImage(fileSrc(custom), '');
    if (!slides) {
        const data = await api.site_json({ path: '/api/launcher/hero' }).catch(() => null);
        slides = (data?.slides || []).sort(() => Math.random() - 0.5);
    }
    if (sourceChanged) await nextSlide();
    if (seconds > 0 && slides.length > 1) slideTimer = setInterval(nextSlide, seconds * 1000);
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
