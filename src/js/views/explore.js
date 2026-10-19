import { api, openUrl } from '../api.js';
import { $, clear, confirm, h, toast, guard } from '../ui.js';
import { icon } from '../icons.js';
import { selectedInstance } from '../store.js';
import { joinServer } from './home.js';

let servers = null;
let loadedAt = 0;

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

async function render(force = false) {
    const box = $('#explore-list');
    if (!servers || force || Date.now() - loadedAt > 10 * 60 * 1000) {
        if (!servers) clear(box).append(h('div', { class: 'empty-state' }, '讀取中⋯'));
        try {
            servers = (await api.site_json({ path: '/api/launcher/servers' })).servers || [];
            loadedAt = Date.now();
        } catch {
            if (!servers) return clear(box).append(h('div', { class: 'empty-state' }, '暫時連不上官網，請稍後再試。'));
        }
    }
    clear(box).append(...(servers.length ? servers.map(card) : [h('div', { class: 'empty-state' }, '目前還沒有合作伺服器。')]));
}

export function initExplore() {
    $('#btn-explore-refresh').addEventListener('click', guard(() => render(true)));
    window.addEventListener('hashchange', () => { if (location.hash === '#explore') render(); });
    if (location.hash === '#explore') render();
}
