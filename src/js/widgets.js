import { $, $$, clear, fail, fmtDuration, h, toast } from './ui.js';
import { on, saveSettings, store } from './store.js';
import { icon } from './icons.js';
import { provide } from './commands.js';
import { upcomingEvent } from './views/home.js';

const WIDGETS = [
    ['brand', '輝煌標誌與標語', '.home-brand', 'showBrand'],
    ['clock', '時鐘', '#home-clock', 'showClock'],
    ['news', '公告欄', '.news', 'showNews'],
    ['dock', '分流與開始遊戲', '.dock', null],
    ['social', '社群連結列', '.social', 'showSocial'],
    ['credit', '攝影者署名', '#bg-credit', 'showCredit'],
    ['stats', '遊玩統計', '#w-stats', 'widget'],
    ['note', '便條', '#w-note', 'widget'],
    ['countdown', '活動倒數', '#w-countdown', 'widget']
];

const layout = () => store.settings?.homeWidgets ?? [];
const posOf = (id) => layout().find((w) => w.id === id);
const shown = ([id, , , key]) => (key === 'widget' ? !!posOf(id)?.visible : key ? store.settings?.[key] !== false && (id !== 'clock' || !!store.settings?.showClock) : true);

function placeAll() {
    for (const [id, , sel, key] of WIDGETS) {
        const el = $(sel);
        if (!el) continue;
        el.dataset.widget = id;
        if (key === 'widget') el.hidden = !posOf(id)?.visible;
        const pos = posOf(id);
        const custom = pos && (pos.x || pos.y || pos.scale !== 1);
        el.toggleAttribute('data-placed', !!custom);
        Object.assign(el.style, custom
            ? { left: `${pos.x}%`, top: `${pos.y}%`, right: 'auto', bottom: 'auto', transform: 'none', scale: String(pos.scale || 1), transformOrigin: '0 0' }
            : { left: '', top: '', right: '', bottom: '', transform: '', scale: '', transformOrigin: '' });
    }
}

function renderStats() {
    const box = $('#w-stats');
    if (box.hidden) return;
    const list = store.instances ?? [];
    const total = list.reduce((n, i) => n + (i.playSeconds || 0), 0);
    const top = [...list].sort((a, b) => (b.playSeconds || 0) - (a.playSeconds || 0))[0];
    const last = [...list].sort((a, b) => (b.lastPlayed || 0) - (a.lastPlayed || 0))[0];
    const ago = last?.lastPlayed ? Math.floor((Date.now() / 1000 - last.lastPlayed) / 86400) : null;
    clear(box).append(
        h('div', { class: 'w-head' }, icon('clock'), '遊玩統計'),
        h('div', { class: 'w-big' }, fmtDuration(total)),
        h('div', { class: 'w-row' }, h('span', {}, '實例'), h('b', {}, `${list.length} 個`)),
        top ? h('div', { class: 'w-row' }, h('span', {}, '最常玩'), h('b', {}, top.name)) : null,
        last?.lastPlayed ? h('div', { class: 'w-row' }, h('span', {}, '上次遊玩'), h('b', {}, ago === 0 ? '今天' : `${ago} 天前`)) : null);
}

let noteTimer = 0;

function renderNote() {
    const box = $('#w-note');
    if (box.hidden || box.contains(document.activeElement)) return;
    const area = h('textarea', { maxlength: '500', placeholder: '寫點什麼⋯例如今天要蓋的東西', 'aria-label': '便條' }, store.settings?.homeNote ?? '');
    area.addEventListener('input', () => {
        clearTimeout(noteTimer);
        noteTimer = setTimeout(() => saveSettings({ homeNote: area.value }).catch(fail), 600);
    });
    clear(box).append(h('div', { class: 'w-head' }, icon('edit'), '便條'), area);
}

function countdownTarget() {
    const s = store.settings ?? {};
    if (s.countdownAt > Date.now()) return { title: s.countdownTitle || '倒數', at: s.countdownAt };
    const event = upcomingEvent();
    return event && event.start > Date.now() ? { title: event.name, at: event.start } : event?.active ? { title: event.name, at: 0 } : null;
}

function renderCountdown() {
    const box = $('#w-countdown');
    if (box.hidden) return;
    const target = countdownTarget();
    const left = target?.at ? Math.max(0, target.at - Date.now()) : 0;
    const d = Math.floor(left / 86400000);
    const hr = Math.floor((left % 86400000) / 3600000);
    const m = Math.floor((left % 3600000) / 60000);
    const sec = Math.floor((left % 60000) / 1000);
    clear(box).append(
        h('div', { class: 'w-head' }, icon('gift'), target ? target.title : '活動倒數'),
        h('div', { class: 'w-big' }, !target ? '目前沒有活動' : !target.at ? '進行中' : d ? `${d} 天 ${hr} 小時` : `${String(hr).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(sec).padStart(2, '0')}`),
        target?.at ? h('div', { class: 'w-row' }, h('span', {}, '開始時間'), h('b', {}, new Date(target.at).toLocaleString('zh-TW', { month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false }))) : null);
}

function renderAll() {
    placeAll();
    renderStats();
    renderNote();
    renderCountdown();
}

let editing = false;
let drag = null;

async function saveLayout(id, patch) {
    const list = layout().filter((w) => w.id !== id);
    const current = posOf(id) ?? { id, x: 0, y: 0, scale: 1, visible: true };
    await saveSettings({ homeWidgets: [...list, { ...current, ...patch, id }] }).catch(fail);
}

function editBar() {
    const visibility = (entry) => {
        const [id, label, , key] = entry;
        if (!key) return null;
        return h('label', {}, h('input', {
            type: 'checkbox',
            checked: shown(entry),
            onchange: async (e) => {
                if (key === 'widget') await saveLayout(id, { visible: e.target.checked });
                else await saveSettings({ [key]: e.target.checked }).catch(fail);
            }
        }), label);
    };
    return h('div', { class: 'home-edit-bar glass', id: 'home-edit-bar' },
        h('b', {}, '編輯主頁版面'),
        h('span', { class: 'hint' }, '拖曳元件移動位置，拖右下角的把手縮放。'),
        h('div', { class: 'check-row' }, ...WIDGETS.map(visibility)),
        h('div', { class: 'toolbar' },
            h('button', { type: 'button', class: 'mc', onclick: async () => { await saveSettings({ homeWidgets: layout().filter((w) => ['stats', 'note', 'countdown'].includes(w.id)).map((w) => ({ ...w, x: 0, y: 0, scale: 1 })) }).catch(fail); toast('已還原預設位置'); } }, icon('reload'), '還原位置'),
            h('button', { type: 'button', class: 'mc primary', onclick: () => setEditing(false) }, icon('check'), '完成')));
}

function setEditing(on) {
    editing = on;
    document.body.classList.toggle('home-editing', on);
    $('#home-edit-bar')?.remove();
    $$('.widget-grip').forEach((g) => g.remove());
    if (!on) return;
    location.hash = '#home';
    $('#view-home').append(editBar());
    for (const [id, label, sel] of WIDGETS) {
        const el = $(sel);
        if (!el || el.hidden) continue;
        el.append(h('span', { class: 'widget-grip', title: `縮放「${label}」`, 'data-grip': id }));
    }
}

function pointerDown(e) {
    if (!editing || e.button !== 0) return;
    const el = e.target.closest('[data-widget]');
    if (!el || e.target.closest('#home-edit-bar, #home-edit-toggle')) return;
    e.preventDefault();
    const stage = $('#view-home').getBoundingClientRect();
    const rect = el.getBoundingClientRect();
    drag = {
        el,
        id: el.dataset.widget,
        resize: !!e.target.closest('.widget-grip'),
        x: e.clientX,
        y: e.clientY,
        stage,
        left: rect.left - stage.left,
        top: rect.top - stage.top,
        width: rect.width,
        height: rect.height,
        scale: posOf(el.dataset.widget)?.scale || 1
    };
    el.classList.add('widget-dragging');
}

function pointerMove(e) {
    if (!drag) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    if (drag.resize) {
        const base = drag.width / drag.scale;
        const room = Math.min((drag.stage.width - drag.left) / base, (drag.stage.height - drag.top) / (drag.height / drag.scale));
        const scale = Math.min(2, room, Math.max(0.5, drag.scale * (drag.width + dx) / drag.width));
        drag.next = { scale: Math.round(scale * 100) / 100, x: (drag.left / drag.stage.width) * 100, y: (drag.top / drag.stage.height) * 100 };
    } else {
        const left = Math.min(Math.max(0, drag.left + dx), drag.stage.width - Math.min(drag.width, drag.stage.width));
        const top = Math.min(Math.max(0, drag.top + dy), drag.stage.height - Math.min(drag.height, drag.stage.height));
        drag.next = { x: Math.round((left / drag.stage.width) * 1000) / 10, y: Math.round((top / drag.stage.height) * 1000) / 10, scale: drag.scale };
    }
    Object.assign(drag.el.style, { left: `${drag.next.x}%`, top: `${drag.next.y}%`, right: 'auto', bottom: 'auto', transform: 'none', scale: String(drag.next.scale), transformOrigin: '0 0' });
}

async function pointerUp() {
    if (!drag) return;
    const { el, id, next } = drag;
    drag = null;
    el.classList.remove('widget-dragging');
    if (next) await saveLayout(id, { ...next, visible: posOf(id)?.visible ?? true });
}

export function initWidgets() {
    provide('editHome', () => setEditing(true));
    $('#home-edit-toggle').addEventListener('click', () => setEditing(!editing));
    addEventListener('pointerdown', pointerDown, true);
    addEventListener('pointermove', pointerMove);
    addEventListener('pointerup', pointerUp);
    addEventListener('keydown', (e) => { if (editing && e.key === 'Escape') setEditing(false); });
    addEventListener('hashchange', () => { if (editing && location.hash !== '#home') setEditing(false); });
    on('settings', () => {
        renderAll();
        if (editing) setEditing(true);
    });
    on('instances', renderStats);
    on('events', renderCountdown);
    setInterval(() => { if (document.body.dataset.view === 'home' && !document.hidden) renderCountdown(); }, 1000);
    renderAll();
}
