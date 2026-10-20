import { api, openUrl } from '../api.js';
import { $, $$, clear, confirm, h, toast, guard } from '../ui.js';
import { icon } from '../icons.js';
import { selectedInstance } from '../store.js';
import { joinServer } from './home.js';

const TABS = {
    servers: { path: '/api/launcher/servers', key: 'servers', empty: '目前還沒有合作伺服器。', hint: '與輝煌合作的伺服器。按「用目前實例加入」會用你選中的實例直接連進去，輝煌分流的設定不會被改動。' },
    works: { path: '/api/launcher/works', key: 'works', empty: '目前還沒有推薦作品。', hint: '輝煌朋友們的作品，喜歡的話歡迎加入願望清單支持他們。' }
};
const data = {};
let tab = 'servers';

function statusLine(status) {
    if (status === undefined) return h('span', { class: 'srv-state pending' }, '查詢中⋯');
    if (!status) return h('span', { class: 'srv-state down' }, '目前連不上');
    return h('span', { class: 'srv-state up' }, h('b', {}, `${status.online}`), ` / ${status.max} 人在線　${status.latencyMs}ms`);
}

async function join(server) {
    const instance = selectedInstance();
    if (!instance) return toast('請先到「實例」建立一個遊戲實例', 'warn');
    if (server.version && server.version !== instance.mcVersion) {
        const ok = await confirm(`「${server.name}」使用 Minecraft ${server.version}，你目前的實例「${instance.name}」是 ${instance.mcVersion}，版本不同可能無法進入。\n要繼續嗎？`, '仍要加入');
        if (!ok) return;
    }
    joinServer(server.ip, server.name);
}

function card(server) {
    const state = h('div', { class: 'srv-status' }, statusLine(undefined));
    api.ping_server({ host: server.ip }).then((s) => clear(state).append(statusLine(s)), () => clear(state).append(statusLine(null)));
    const link = (name, label, url) => (url ? h('button', { type: 'button', class: 'mc', onclick: () => openUrl(url) }, icon(name), label) : null);
    return h('article', { class: 'srv-card' },
        h('div', { class: 'srv-head' },
            server.logo ? h('img', { class: 'srv-logo', src: server.logo, alt: '' }) : h('div', { class: 'srv-logo' }, icon('map')),
            h('div', { class: 'grow' },
                h('h2', {}, server.name),
                h('div', { class: 'srv-meta' },
                    h('button', { type: 'button', class: 'srv-ip', title: '複製伺服器位址', onclick: () => navigator.clipboard.writeText(server.ip).then(() => toast(`已複製 ${server.ip}`)) }, server.ip, icon('copy')),
                    server.version ? h('span', { class: 'mtag side' }, `Minecraft ${server.version}`) : null,
                    ...server.tags.map((t) => h('span', { class: 'mtag' }, t))),
                state)),
        server.description ? h('p', { class: 'srv-desc' }, server.description) : null,
        h('div', { class: 'srv-actions' },
            h('button', { type: 'button', class: 'mc primary', onclick: () => join(server) }, icon('play'), '用目前實例加入'),
            link('external-link', '官網', server.website),
            link('discord', 'Discord', server.discord)));
}

function workCard(work) {
    return h('article', { class: 'srv-card work-card' },
        work.image ? h('img', { class: 'work-cover', src: work.image, alt: work.name, loading: 'lazy' }) : null,
        h('div', { class: 'work-body' },
            h('h2', {}, work.name),
            h('div', { class: 'srv-meta' },
                work.developers.length ? h('span', { class: 'mtag side' }, work.developers.join('、')) : null,
                work.release ? h('span', { class: 'mtag' }, work.release) : null,
                work.price ? h('span', { class: 'mtag' }, work.price) : null,
                ...work.platforms.map((p) => h('span', { class: 'mtag' }, p))),
            work.description ? h('p', { class: 'srv-desc' }, work.description) : null,
            work.genres.length ? h('div', { class: 'work-genres' }, work.genres.join('・')) : null,
            h('div', { class: 'srv-actions' },
                h('button', { type: 'button', class: 'mc primary', onclick: () => openUrl(work.url) }, icon(work.comingSoon ? 'heart' : 'external-link'), work.comingSoon ? '到 Steam 加入願望清單' : '到 Steam 查看'))));
}

async function render(force = false) {
    const box = $('#explore-list');
    const conf = TABS[tab];
    $('#explore-hint').textContent = conf.hint;
    for (const b of $$('#explore-tabs button')) b.setAttribute('aria-selected', String(b.dataset.tab === tab));
    const cached = data[tab];
    if (!cached || force || Date.now() - cached.at > 10 * 60 * 1000) {
        if (!cached) clear(box).append(h('div', { class: 'empty-state' }, '讀取中⋯'));
        const want = tab;
        try {
            data[want] = { at: Date.now(), list: (await api.site_json({ path: conf.path }))[conf.key] || [] };
        } catch {
            if (!data[want] && want === tab) return clear(box).append(h('div', { class: 'empty-state' }, '暫時連不上官網，請稍後再試。'));
        }
        if (want !== tab) return;
    }
    const list = data[tab].list;
    clear(box).append(...(list.length ? list.map(tab === 'works' ? workCard : card) : [h('div', { class: 'empty-state' }, conf.empty)]));
}

export function initExplore() {
    for (const b of $$('#explore-tabs button')) b.addEventListener('click', () => { tab = b.dataset.tab; render(); });
    $('#btn-explore-refresh').addEventListener('click', guard(() => render(true)));
    window.addEventListener('hashchange', () => { if (location.hash === '#explore') render(); });
    if (location.hash === '#explore') render();
}
