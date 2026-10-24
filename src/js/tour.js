import { $, h, modalOpen } from './ui.js';
import { saveSettings, store } from './store.js';

const TOURS = {
    instances: {
        view: 'instances',
        steps: [
            ['#btn-new-instance', '建立實例', '自己挑 Minecraft 版本、Fabric、記憶體與封面。每個實例的模組、存檔都各自獨立。'],
            ['#btn-quick-instance', '一鍵建立輝煌實例', '不想自己設定？按這裡會用輝煌推薦的版本與模組直接建好。'],
            ['#btn-import', '從其他啟動器搬家', '官方、Prism、CurseForge、Modrinth App 的實例、存檔與設定都能帶過來。'],
            ['#btn-new-group', '建立分組', '把實例收進分組，還能讓同一組共用資源包、光影、存檔與截圖。'],
            ['#instance-list .inst-card', '實例卡片', '點一下選擇實例，點兩下進入它的模組與資源。也可以把卡片拖到分組上，或把模組檔案直接拖進來安裝。'],
            ['#instance-list .inst-card .inst-more > button', '更多功能', '世界管理、存檔備份、截圖、升級版本、效能健檢、建立桌面捷徑都在這裡。'],
            ['#instance-list .inst-group-name', '分組標題', '點一下收合；按右鍵可以編輯、置頂、調整順序；拖曳也能排序。']
        ]
    },
    mods: {
        view: 'mods',
        steps: [
            ['.inst-page-name', '切換實例', '點名稱就能直接切換到其他實例，不用回到實例列表。'],
            ['.inst-page-actions', '開始遊戲與設定', '從這裡直接開始遊戲、修改實例設定，或打開更多功能。'],
            ['#kind-tabs', '模組、資源包、光影', '三種內容分開管理，切換後清單與商店會跟著換。'],
            ['#view-mods .store-tabs', '已安裝與瀏覽內容', '「已安裝」管理現有內容；「瀏覽內容」可以搜尋 Modrinth、一鍵安裝推薦組合。'],
            ['#store-installed .toolbar', '安裝與更新', '可以一次更新全部、安裝本機檔案；也能直接把檔案拖進視窗。移除模組時還能順便清掉它留下的設定檔。']
        ]
    },
    settings: {
        view: 'settings',
        steps: [
            ['#settings-nav', '設定分類', '設定依用途分成幾頁，點上方分頁切換。'],
            ['#settings-search', '搜尋設定', '不知道在哪一頁？直接打字搜尋，所有分頁裡符合的設定都會列出來。'],
            ['#save-state', '自動儲存', '所有變更都會自動儲存，不用另外按儲存。外觀分頁還能把啟動器改成你喜歡的樣子。']
        ]
    },
    log: {
        view: 'log',
        steps: [
            ['#log-instance', '選擇實例', '每個實例的日誌分開保存，多開時也能各看各的。'],
            ['#log-source', '日誌來源', '即時輸出、過去的日誌、當機報告與 Java 崩潰紀錄都能在這裡切換。'],
            ['#log-search', '搜尋', '輸入關鍵字，按 Enter 跳到下一個結果。'],
            ['#btn-log-upload', '上傳與回報', '一鍵上傳給開發者，或用「我的回報」把問題交給輝煌管理員並追蹤回覆。']
        ]
    }
};

let active = null;

const visible = (el) => {
    if (!el) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
};

export function hasTour(view = document.body.dataset.view) {
    return Object.values(TOURS).some((t) => t.view === view);
}

export function startTour(view = document.body.dataset.view) {
    const [id, tour] = Object.entries(TOURS).find(([, t]) => t.view === view) ?? [];
    if (!tour || active) return;
    const steps = tour.steps.map(([sel, title, text]) => ({ el: $(sel), title, text })).filter((s) => visible(s.el));
    if (!steps.length) return;
    let index = 0;
    const spot = h('div', { class: 'tour-spot' });
    const count = h('span', { class: 'tour-count' });
    const title = h('b', { class: 'tour-title' });
    const text = h('p', { class: 'tour-text' });
    const prev = h('button', { type: 'button', class: 'mc', onclick: () => go(index - 1) }, '上一步');
    const next = h('button', { type: 'button', class: 'mc primary', onclick: () => go(index + 1) }, '下一步');
    const skip = h('button', { type: 'button', class: 'tour-skip', onclick: () => finish() }, '略過教學');
    const bubble = h('div', { class: 'tour-bubble', role: 'dialog', 'aria-modal': 'true', 'aria-live': 'polite' },
        h('div', { class: 'tour-head' }, count, skip), title, text, h('div', { class: 'tour-actions' }, prev, next));
    const layer = h('div', { class: 'tour-layer', onclick: (e) => { if (e.target === layer) go(index + 1); } }, spot, bubble);
    layer.popover = 'manual';
    document.body.append(layer);
    layer.showPopover();
    const place = () => {
        const step = steps[index];
        const r = step.el.getBoundingClientRect();
        const pad = 6;
        Object.assign(spot.style, { left: `${r.left - pad}px`, top: `${r.top - pad}px`, width: `${r.width + pad * 2}px`, height: `${r.height + pad * 2}px` });
        const bw = Math.min(340, innerWidth - 24);
        const bh = bubble.offsetHeight || 160;
        const below = r.bottom + pad + 12;
        const above = r.top - pad - 12 - bh;
        const clampTop = (y) => Math.max(12, Math.min(innerHeight - bh - 12, y));
        let top;
        let left = Math.max(12, Math.min(innerWidth - bw - 12, r.left + r.width / 2 - bw / 2));
        if (below + bh < innerHeight - 12) top = below;
        else if (above > 12) top = above;
        else {
            top = clampTop(r.top);
            const right = r.right + pad + 12;
            left = right + bw < innerWidth - 12 ? right : Math.max(12, r.left - pad - 12 - bw);
        }
        Object.assign(bubble.style, { width: `${bw}px`, left: `${left}px`, top: `${top}px` });
    };
    const go = (to) => {
        if (to >= steps.length) return finish();
        index = Math.max(0, to);
        const step = steps[index];
        step.el.scrollIntoView({ block: 'nearest' });
        count.textContent = `${index + 1} / ${steps.length}`;
        title.textContent = step.title;
        text.textContent = step.text;
        prev.disabled = index === 0;
        next.textContent = index === steps.length - 1 ? '完成' : '下一步';
        requestAnimationFrame(place);
        next.focus();
    };
    const onKey = (e) => {
        if (e.key === 'Escape') { e.preventDefault(); finish(); }
        else if (e.key === 'ArrowRight' || e.key === 'Enter') { e.preventDefault(); go(index + 1); }
        else if (e.key === 'ArrowLeft') { e.preventDefault(); go(index - 1); }
    };
    const finish = () => {
        removeEventListener('keydown', onKey, true);
        removeEventListener('resize', place);
        layer.remove();
        active = null;
        const seen = store.settings?.toursSeen ?? [];
        if (!seen.includes(id)) saveSettings({ toursSeen: [...seen, id] }).catch(() => {});
    };
    addEventListener('keydown', onKey, true);
    addEventListener('resize', place);
    active = { finish };
    go(0);
}

export function initTours() {
    const button = $('#tour-btn');
    const sync = () => { button.hidden = !hasTour(); };
    button.addEventListener('click', () => startTour());
    const auto = () => {
        sync();
        const view = document.body.dataset.view;
        const id = Object.entries(TOURS).find(([, t]) => t.view === view)?.[0];
        if (!id || !store.settings || (store.settings.toursSeen ?? []).includes(id)) return;
        setTimeout(() => { if (document.body.dataset.view === view && !modalOpen() && !active) startTour(view); }, 700);
    };
    addEventListener('hashchange', () => setTimeout(auto, 50));
    auto();
}
