(function () {
    var ground = document.getElementById('walkers');
    var layer = document.getElementById('walker-layer');
    if (!ground || !layer) return;

    var MAX = 300;
    var HEIGHT = 64;
    var FRAMES = +ground.getAttribute('data-frames') || 37;
    var CORE = 18;
    var SIT_EDGE = 17;
    var SIT_FLAT = 9;
    var FURNI = 36;
    var BLOCK = 24;
    var JUMP_UP = 90;
    var JUMP_DOWN = 160;
    var JUMP_GAP = 60;
    var SURFACES = '.hero, .hero-inner, .hero-actions, .promo, .section-title, .announce-item, .post-card, .container > h1, .container-split > h1, .article-block, .toc, .article h2, .article .table-wrap, .article blockquote, .link-card, .shot-card, .order-panel, ' +
        '.home-card, .stat-card, .player-card, .guild-card, .guild-candidate, .nav-row, .query-form, .admin-bar, .line-chart, #chart-container, .nr-chart-col, ' +
        '.ps-hero, .ps-group, .ps-table-wrap, .px-table-wrap, .pb-table-wrap, .gp-table-wrap, .rp-table-wrap, .dev-table-wrap, .ticket-table-wrap, .da-wrap, .inc-wrap, .ipc-card, ' +
        '.shop-row, .reward-row, .rarity-row, .gacha-row, .group-row, .dex-card, .dex-pool, .donation-card, .ban-card, .gal-item, .gal-form, ' +
        '.fm-board, .fm-row, .fm-post, .fm-form, .dino-frame, .map-frame, .mcmap-image, .lost-block, .news, .event-card, .launch, .dock-head, .shards > *, .home-brand, .srv-card, .thanks-card, .inst-grid > *, .shot, .settings-nav';
    var PROPS = 'img, svg, .hero-btn, .hero-title, .hero-slogan, .ip-box, .hero-online, .hero-gallery, .section-title, .promo-title, .promo-thumb, .announce-title, .announce-item, .post-card, .post-card-title, .post-card-desc, .logo-text, .tagline, .news-tabs > *, .news-list > li, .news-more, .dock-head .grow, .dock-total, .shards > *, .event-card button, .event-name, .instance-pick, #btn-play, .social > *, .credit, button.mc, .thanks-card img, .srv-logo, .shot';
    var AIRBORNE = ['held', 'fly', 'fall', 'jump', 'climb'];
    var MOODS = ['dizzy', 'angry', 'tug', 'munch', 'wave', 'sit', 'fish', 'sleep', 'warp-in', 'warp-out', 'spin', 'dance', 'wish', 'play', 'doze', 'stretch', 'mine', 'offer'];
    var PETS = [['kapippi', -1], ['komukumu', 1], ['hanawa', 0], ['konkon', 1], ['pikadon', -1]];
    var COINS = { aether: ['custom/brilliant/currency/aether', '+1 乙太'], paradise: ['custom/brilliant/currency/paradise_powder', '+1 聖輝'], luminous: ['custom/brilliant/currency/luminous_crystal', '+1 晶淵'] };
    var GAMES = [
        { name: 'BNO 遊戲', max: 4, carpet: 'white_wool', piece: 'card', pick: function () { return 'custom/2025childrensday/uno_' + pick(['red', 'blue', 'green', 'yellow']) + '_' + Math.floor(Math.random() * 10); } },
        { name: '撲克遊戲', max: 4, carpet: 'lime_wool', piece: 'card', pick: function () { return 'custom/2026valentinesday/poker/' + pick(['clover', 'clubs', 'heart', 'spade']) + '_' + pick(['1', '2', '3', '4', '5', '6', '7', '8', '9', '10', 'j', 'q', 'k']); } },
        { name: '麻將遊戲', max: 4, carpet: 'green_wool', piece: 'tile', pick: function () { return pick(['中', '發', '東', '南', '西', '北', '一', '五', '九']); } },
        { name: '楚河漢界', max: 2, carpet: 'orange_wool', piece: 'xq', pick: function () { return pick(['帥', '俥', '傌', '炮', '兵', '將', '車', '馬', '包', '卒']); } },
        { name: '西洋棋遊', max: 2, carpet: 'white_wool', piece: 'chess', board: true, pick: function () { return pick(['♔', '♕', '♖', '♗', '♘', '♙', '♚', '♛', '♜', '♝', '♞', '♟']); } },
        { name: '跳棋遊戲', max: 2, carpet: 'white_wool', piece: 'checker', board: true, pick: function () { return pick(['red', 'black']); } }
    ];
    var MASCOT = 'Misteln_Rice';
    var OWNER = 'RiceChen_';
    var OWNER_COLOR = '#ff8080';
    var MASCOT_EMOTES = ['各色飯娘表情符號/彩色.gif', '各色飯娘表情符號/紅色.png', '各色飯娘表情符號/橘色.png', '各色飯娘表情符號/黃色.png', '各色飯娘表情符號/綠色.png',
        '各色飯娘表情符號/淡綠色.png', '各色飯娘表情符號/青色.png', '各色飯娘表情符號/天藍色.png', '各色飯娘表情符號/藍色.png', '各色飯娘表情符號/紫色.png', '各色飯娘表情符號/洋紅色.png',
        '像素風格飯娘表情符號/1.gif', '像素風格飯娘表情符號/2.png', '像素風格飯娘表情符號/3.png', '像素風格飯娘表情符號/4.png', '像素風格飯娘表情符號/5.png', '像素風格飯娘表情符號/6.png', '像素風格飯娘表情符號/7.gif'];
    var BOXES = ['copper', 'diamond', 'emerald', 'gold', 'iron', 'netherite', 'paradise'];
    var DISCS = ['cat', 'blocks', 'chirp', 'far', 'mall', 'mellohi', 'stal', 'strad', 'wait', 'pigstep', 'otherside', 'relic', 'creator', 'precipice'];
    var ICON = 'https://www.brilliantw.net/mcstats/icon/';
    var FX = { happy: 'particle/glint', heart: 'particle/heart', angry: 'particle/angry', portal: 'particle/generic_4', smoke: 'particle/big_smoke_4', crumb: 'item/cake', chip: 'block/dirt', splash: 'particle/splash_0', tune: 'particle/note', totem: 'particle/glitter_4', rubble: 'block/stone' };
    var VANILLA = {
        fish: [['cod', '生鱈魚', 60], ['salmon', '生鮭魚', 25], ['pufferfish', '河豚', 13], ['tropical_fish', '熱帶魚', 2]],
        junk: [['lily_pad', '睡蓮', 17], ['leather_boots', '皮革靴子', 10], ['bone', '骨頭', 10], ['string', '線', 5], ['bowl', '碗', 10], ['stick', '木棒', 5], ['ink_sac', '墨囊', 1], ['rotten_flesh', '腐肉', 10]],
        treasure: [['bow', '弓', 1], ['enchanted_book', '附魔書', 1], ['name_tag', '命名牌', 1], ['nautilus_shell', '鸚鵡螺殼', 1], ['saddle', '鞍', 1]]
    };
    var mascotOnly = ground.getAttribute('data-mode') === 'mascot';
    var still = window.matchMedia && matchMedia('(prefers-reduced-motion: reduce)').matches;

    var walkers = {};
    var queue = [];
    var surfaces = [];
    var taken = [];
    var cakes = [];
    var last = 0;
    var clock = 0;
    var held = null;
    var viewTop = 0;
    var spawnT = 0;
    var chatBox = null;
    var pool = null;
    var mouse = null;
    var ready = false;
    var portal = null;
    var portalAt = 0;
    var guildOf = {};
    var emojis = null;
    var game = null;
    var gameAt = 0;
    var meteor = null;
    var meteorAt = 0;
    var music = null;
    var jukebox = null;
    var discs = null;
    var infoOf = {};
    var mascot = null;
    var coinAt = 0;
    var sign = null;
    var known = [];
    var dex = null;
    var pets = [];
    var furniture = [];
    var mined = [];
    var disabled = false;
    var meName = null;
    var menuEl = null;

    function rand(a, b) { return a + Math.random() * (b - a); }
    function pick(list) { return list[Math.floor(Math.random() * list.length)]; }
    function clamp(v, lo, hi) { return Math.max(lo, Math.min(hi, v)); }
    function all() { return Object.keys(walkers).map(function (k) { return walkers[k]; }); }

    function visible(el, r) {
        var left = r.left;
        var right = r.right;
        for (var a = el.parentElement; a && a !== document.body; a = a.parentElement) {
            var cs = getComputedStyle(a);
            if (cs.overflowX === 'visible' && cs.overflowY === 'visible') continue;
            var c = a.getBoundingClientRect();
            if (r.top < c.top || r.top > c.bottom) return null;
            left = Math.max(left, c.left);
            right = Math.min(right, c.right);
        }
        return right - left > 20 ? { left: left, right: right } : null;
    }
    function measure() {
        var origin = layer.getBoundingClientRect();
        var g = ground.getBoundingClientRect();
        var floor = g.bottom - 4;
        viewTop = -origin.top;
        var found = [ground].concat([].slice.call(document.querySelectorAll(SURFACES)));
        surfaces = found.map(function (el) {
            var r = el.getBoundingClientRect();
            if (!r.width || (!r.height && el !== ground)) return null;
            var top = el === ground ? floor : r.top;
            var v = el === ground ? r : top < floor && getComputedStyle(el).visibility !== 'hidden' && visible(el, r);
            return v ? { el: el, left: v.left - origin.left, right: v.right - origin.left, top: top - origin.top, view: top > HEIGHT && top < window.innerHeight } : null;
        }).filter(Boolean);
    }
    function drop(w) {
        w.plan.forEach(function (a) {
            if (a.type === 'pick' && taken.indexOf(a.prop) >= 0) taken.splice(taken.indexOf(a.prop), 1);
            if (a.rod) a.rod.remove();
            if (a.blocks) crumble(a.blocks);
            if (a.after) crumble(a.after);
            if (a.mine && a.mine.crack) {
                a.mine.crack.remove();
                a.mine.tool.remove();
                var i = taken.indexOf(a.mine.prop);
                if (i >= 0 && mined.every(function (m) { return m.prop !== a.mine.prop; })) taken.splice(i, 1);
            }
        });
        w.plan = [];
        MOODS.forEach(function (c) { w.el.classList.remove(c); });
        var held = w.el.querySelector('.walker-held');
        if (held) held.remove();
    }
    function busy(w) { return !!w.plan[0] && AIRBORNE.indexOf(w.plan[0].type) >= 0; }
    function calm(a) { return !a || ((a.type === 'walk' || (a.type === 'idle' && !a.cls)) && !a.game); }
    function headOf(o) {
        var t = o.plan[0] && o.plan[0].type;
        if (walkers[o.name] !== o || o.el.hidden || !o.half || t === 'held' || t === 'fly' || t === 'fall' || t === 'climb') return null;
        var top = o.y - HEIGHT + 5;
        return { el: o.el, walker: o, left: o.x - o.half * 0.7, right: o.x + o.half * 0.7, top: top, view: top - viewTop > HEIGHT && top - viewTop < window.innerHeight };
    }
    function surfaceOf(el) {
        if (el && el.walker) return headOf(el.walker);
        for (var i = 0; i < surfaces.length; i++) if (surfaces[i].el === el) return surfaces[i];
        return null;
    }
    function below(x, y) {
        var best = null;
        surfaces.forEach(function (t) {
            if (x >= t.left && x <= t.right && t.top >= y - 1 && (!best || t.top < best.top)) best = t;
        });
        return best || surfaceOf(ground);
    }
    function seen(list) {
        var near = list.filter(function (t) { return t.view; });
        return near.length ? near : list;
    }
    function relRect(el) {
        var o = layer.getBoundingClientRect();
        var r = el.getBoundingClientRect();
        return { left: r.left - o.left, top: r.top - o.top, width: r.width, height: r.height };
    }

    function burst(x, y, kind, n, image) {
        if (still) return;
        for (var i = 0; i < n; i++) {
            var p = document.createElement('span');
            var x0 = x + rand(-10, 10);
            var y0 = y + rand(-6, 6);
            var spread = kind === 'totem' || kind === 'star' ? 40 : kind === 'portal' || kind === 'smoke' || kind === 'petal' ? 24 : 12;
            var fall = kind === 'crumb' || kind === 'chip';
            var drift = kind === 'petal';
            if (drift) image = 'url(' + ICON + 'particle/cherry_' + Math.floor(rand(0, 12)) + '.png)';
            if (fall) {
                x0 = x + rand(-3, 3);
                y0 = y + rand(-2, 2);
            }
            p.className = 'walker-fx ' + kind;
            p.style.cssText = '--x0:' + x0.toFixed(1) + 'px;--y0:' + y0.toFixed(1) + 'px;--x1:' + (x0 + rand(-spread, spread)).toFixed(1) +
                'px;--y1:' + (fall ? y0 + rand(14, 30) : drift ? y0 + rand(24, 48) : y0 - rand(12, 36)).toFixed(1) + 'px;--t:' + rand(0.6, 1.1).toFixed(2) + 's';
            if (kind === 'tune') p.style.setProperty('--h', Math.floor(rand(0, 360)) + 'deg');
            if (kind === 'totem') p.style.setProperty('--h', Math.floor(rand(20, 110)) + 'deg');
            if (kind === 'zzz') p.textContent = 'Z';
            else if (kind === 'star') {
                p.textContent = '✦';
                p.style.color = image || '#ffecc3';
            } else p.style.backgroundImage = image || 'url(' + ICON + FX[kind] + '.png)';
            if (fall) p.style.backgroundPosition = rand(0, 100).toFixed(0) + '% ' + rand(0, 100).toFixed(0) + '%';
            p.addEventListener('animationend', function () { this.remove(); });
            layer.appendChild(p);
        }
    }
    function chat(text) {
        if (!chatBox) {
            chatBox = document.createElement('div');
            chatBox.className = 'walker-chat';
            chatBox.setAttribute('aria-hidden', 'true');
            document.body.appendChild(chatBox);
        }
        var line = document.createElement('div');
        [].concat(typeof text === 'string' ? [[text]] : text).forEach(function (part) {
            var span = document.createElement('span');
            span.textContent = part[0];
            if (part[1]) span.style.color = part[1];
            line.appendChild(span);
        });
        chatBox.appendChild(line);
        while (chatBox.children.length > 6) chatBox.firstChild.remove();
        setTimeout(function () { line.remove(); }, 5000);
    }

    function icon(path) { return 'url(' + ICON + path + '.png)'; }
    function viewed(w) {
        var s = surfaceOf(w.s);
        return s && !s.walker && s.view && !w.el.hidden && !w.carry && calm(w.plan[0]);
    }
    function label(w) {
        var tag = w.el.firstChild;
        tag.textContent = w.npc ? '飯娘' : w.owner ? '［服主］' + w.name : (w.guild ? '［' + w.guild + '］' : '') + w.name;
        tag.style.color = w.npc || w.waiting ? '' : w.owner ? OWNER_COLOR : w.color || '';
    }
    function applyInfo(w) {
        var i = infoOf[w.name] || {};
        var was = w.waiting;
        w.color = i.c;
        w.veteran = i.t > 3 * 60 * 60 * 1000;
        w.speed = w.pace * (i.t !== undefined && i.t < 10 * 60 * 1000 ? 1.35 : 1);
        w.waiting = Boolean(i.w);
        w.el.classList.toggle('waiting', w.waiting);
        w.donor = i.d || null;
        w.el.classList.toggle('donor', Boolean(w.donor));
        if (w.donor) w.el.style.setProperty('--donor', w.donor.c);
        label(w);
        if (was === undefined || was === w.waiting || w.el.hidden || (held && held.w === w)) return;
        putBack(w, true);
        drop(w);
        if (!w.waiting) {
            var back = pick(seen(surfaces));
            w.plan.push({ type: 'idle', t: rand(0, 2) }, { type: 'jump', to: w.s || ground, x: w.x }, { type: 'rain', to: back.el });
        }
    }
    function gain(w, text, color) {
        var el = document.createElement('span');
        el.className = 'walker-gain';
        el.setAttribute('aria-hidden', 'true');
        el.textContent = text;
        if (color) el.style.color = color;
        el.style.left = Math.round(w.x) + 'px';
        el.style.top = Math.round(w.y - HEIGHT - 10) + 'px';
        el.addEventListener('animationend', function () { el.remove(); });
        layer.appendChild(el);
    }
    function totem(w) {
        var el = document.createElement('span');
        el.className = 'walker-totem';
        el.setAttribute('aria-hidden', 'true');
        el.style.backgroundImage = icon('item/totem_of_undying');
        el.style.left = Math.round(w.x - 16) + 'px';
        el.style.top = Math.round(w.y - HEIGHT - 12) + 'px';
        el.addEventListener('animationend', function () { el.remove(); });
        layer.appendChild(el);
        burst(w.x, w.y - 32, 'totem', 26);
        w.plan.unshift({ type: 'idle', t: 1.2 });
    }
    function night() { return (new Date().getUTCHours() + 8) % 24 < 6; }
    function weighted(list) {
        var total = list.reduce(function (sum, e) { return sum + e.w; }, 0);
        var r = Math.random() * total;
        for (var i = 0; i < list.length; i++) if ((r -= list[i].w) < 0) return list[i];
        return list[list.length - 1];
    }
    function loadPool() {
        if (pool) return;
        pool = { base: 0, fish: [] };
        (window.walkerFetch || fetch)('https://www.brilliantw.net/api/fishpool').then(function (r) { return r.ok ? r.json() : null; }).then(function (d) { if (d && d.fish) pool = d; }).catch(function () { });
    }
    function rollLoot() {
        if (pool && pool.fish.length && Math.random() < pool.base) return weighted(pool.fish);
        var r = Math.random();
        var kind = r < 0.85 ? 'fish' : r < 0.95 ? 'junk' : 'treasure';
        var e = weighted(VANILLA[kind].map(function (v) { return { icon: 'item/' + v[0], name: v[1], w: v[2] }; }));
        return { icon: e.icon, name: e.name, color: kind === 'treasure' ? '#FFFF55' : '#FFFFFF', rare: kind === 'treasure' };
    }
    function crumble(blocks) {
        blocks.slice().reverse().forEach(function (b, i) {
            setTimeout(function () {
                burst(parseFloat(b.style.left) + BLOCK / 2, parseFloat(b.style.top) + BLOCK / 2, 'chip', 6);
                b.remove();
            }, 220 * i);
        });
        blocks.length = 0;
    }

    function add(name, npc) {
        var el = document.createElement('div');
        el.className = 'walker';
        el.hidden = true;
        var tag = document.createElement('span');
        tag.className = 'walker-name';
        tag.textContent = name;
        var body = document.createElement('div');
        body.className = 'walker-body';
        body.title = name;
        var img = new Image();
        img.alt = '';
        var s = (infoOf[name] && infoOf[name].w && surfaceOf(ground)) || pick(seen(surfaces));
        var w = walkers[name] = { name: name, el: el, body: body, land: 0, s: s.el, x: rand(s.left + 4, s.right - 4), y: s.top, half: 0, pace: rand(25, 45), speed: 0, npc: Boolean(npc), patience: rand(1, 6), fresh: ready, drop: 0, plan: [{ type: 'idle', t: rand(0, 3) }] };
        el.walker = w;
        if (name === meName && !npc) {
            w.me = true;
            el.classList.add('me');
        }
        w.owner = !npc && name === OWNER;
        el.classList.toggle('owner', w.owner);
        w.guild = guildOf[name];
        if (!still) {
            w.s = null;
            w.y = Math.min(viewTop, s.top - 120);
            w.plan.unshift({ type: 'fall', to: s.el });
        }
        img.onload = function () {
            var width = (img.naturalWidth / FRAMES) * HEIGHT / img.naturalHeight;
            body.style.width = width + 'px';
            w.wide = width / 2;
            w.half = Math.min(w.wide, CORE);
            el.hidden = false;
            place(w);
            burst(w.x, w.y - 30, 'portal', 14);
        };
        img.onerror = function () { remove(name); };
        img.draggable = false;
        grabbable(w, body);
        img.src = 'https://www.brilliantw.net/api/skin/' + encodeURIComponent(npc ? MASCOT : name) + '?walk=1&v=pose';
        body.appendChild(img);
        el.insertBefore(body, el.firstChild);
        el.insertBefore(tag, el.firstChild);
        if (npc) el.classList.add('npc');
        applyInfo(w);
        layer.appendChild(el);
        return w;
    }
    function sceneShown() {
        return [].some.call(document.querySelectorAll('.lost, .dino-stage'), function (el) { return el.offsetParent !== null; });
    }
    function spawnMascot() {
        var o = layer.getBoundingClientRect();
        var x = -o.left + window.innerWidth / 2;
        var y = viewTop + window.innerHeight / 2;
        var under = below(x, y);
        if (!under) return;
        mascot = add('飯娘', true);
        mascot.s = null;
        mascot.x = clamp(x, under.left + 4, under.right - 4);
        mascot.y = Math.min(y, under.top - 10);
        mascot.plan = [{ type: 'fall', to: under.el }, { type: 'idle', t: 1 }];
    }
    function remove(name, leaving) {
        var w = walkers[name];
        if (!w) return;
        putBack(w, true);
        drop(w);
        if (held && held.w === w) held = null;
        delete walkers[name];
        if (w === mascot) {
            mascot = null;
            pets.forEach(function (p) { p.el.remove(); });
            pets = [];
        }
        if (!leaving) return w.el.remove();
        burst(w.x, w.y - 30, 'smoke', 12);
        w.el.classList.add('gone');
        setTimeout(function () { w.el.remove(); }, 300);
        chat(name + ' 離開了遊戲');
    }
    function place(w) {
        w.el.style.transform = 'translate(' + Math.round(w.x - (w.wide || w.half)) + 'px,' + Math.round(w.y - HEIGHT) + 'px)';
        if (w.carry) {
            w.carry.clone.style.left = Math.round(w.x - w.carry.width / 2) + 'px';
            w.carry.clone.style.top = Math.round(w.y - HEIGHT - w.carry.height - 14) + 'px';
        }
    }
    function pose(w, name) {
        w.el.classList.toggle('walking', name === 'walk' || name === 'crawl');
        w.el.classList.toggle('crawl', name === 'crawl');
        w.el.classList.toggle('climb', name === 'climb');
        w.el.classList.toggle('crouch', name === 'crouch');
        w.el.classList.toggle('air', name === 'air');
    }
    function land(w, el) {
        w.s = el;
        w.land = 0.25;
        w.el.classList.add('land');
        w.body.style.rotate = '';
        w.plan.shift();
        var b = el.walker && el.walker.plan[0];
        if (b && b.type === 'idle' && !b.cls) b.t += rand(2, 5);
        if (w.fresh) {
            w.fresh = false;
            welcome(w);
            if (w.owner) {
                chat([[w.name, OWNER_COLOR], [' 上線了，大家好！', '#FFFFFF']]);
                w.plan.push({ type: 'idle', t: 1.6, cls: 'wave' });
            }
        }
    }
    function welcome(n) {
        var s = surfaceOf(n.s);
        if (!s || s.walker) return;
        var helpers = all().filter(function (h) {
            return h !== n && h.s === n.s && !h.el.hidden && !h.carry && calm(h.plan[0]) && Math.abs(h.x - n.x) < 350;
        }).sort(function (a, b) { return Math.abs(a.x - n.x) - Math.abs(b.x - n.x); }).slice(0, 3);
        if (!helpers.length) return;
        var used = { '-1': 0, '1': 0 };
        helpers.forEach(function (h) {
            var side = h.x < n.x ? -1 : 1;
            var x = n.x + side * (n.half + h.half + 8 + used[side] * (h.half * 2 + 6));
            used[side]++;
            drop(h);
            h.plan.push({ type: 'walk', x: clamp(x, s.left + 4, s.right - 4), fast: true }, { type: 'idle', t: rand(1.4, 2), cls: 'wave', left: side > 0 }, { type: 'idle', t: 0.4 });
            h.cd = clock + rand(20, 40);
        });
        drop(n);
        n.plan.push({ type: 'idle', t: 0.9 }, { type: 'idle', t: 1.6, cls: 'wave', left: helpers[0].x < n.x }, { type: 'idle', t: 0.5 });
        n.cd = clock + rand(20, 40);
    }

    function pointer(e) {
        var o = layer.getBoundingClientRect();
        return { x: e.clientX - o.left, y: e.clientY - o.top, t: e.timeStamp };
    }
    function grabbable(w, body) {
        if (still) return;
        var timer = 0;
        var start = null;
        function grab() {
            clearTimeout(timer);
            if (!start || held) return;
            w.el.classList.remove('pressed');
            putBack(w, true);
            held = { w: w, dx: w.x - start.x, dy: w.y - start.y, last: start, vx: 0, vy: 0 };
            w.s = null;
            drop(w);
            w.plan.push({ type: 'held' });
            w.el.classList.add('held');
        }
        body.addEventListener('contextmenu', function (e) { e.preventDefault(); });
        body.addEventListener('pointerdown', function (e) {
            if (e.button || held) return;
            e.preventDefault();
            start = pointer(e);
            w.el.classList.add('pressed');
            try { body.setPointerCapture(e.pointerId); } catch (err) { }
            clearTimeout(timer);
            timer = setTimeout(grab, 300);
        });
        body.addEventListener('pointermove', function (e) {
            var p = pointer(e);
            if (!held || held.w !== w) {
                if (start && Math.abs(p.x - start.x) + Math.abs(p.y - start.y) > 6) grab();
                if (!held || held.w !== w) return;
            }
            var dt = Math.max(1, p.t - held.last.t) / 1000;
            held.vx = held.vx * 0.5 + (p.x - held.last.x) / dt * 0.5;
            held.vy = held.vy * 0.5 + (p.y - held.last.y) / dt * 0.5;
            held.last = p;
            w.x = p.x + held.dx;
            w.y = p.y + held.dy;
            w.el.classList.toggle('left', held.vx < 0);
        });
        function release(tap) {
            clearTimeout(timer);
            var pressed = start;
            start = null;
            w.el.classList.remove('pressed');
            if (!held || held.w !== w) {
                if (tap && pressed) (w.me ? menu(w) : poke(w));
                return;
            }
            w.el.classList.remove('held');
            w.plan = [{ type: 'fly', vx: clamp(held.vx, -1500, 1500), vy: clamp(held.vy, -1500, 1500) }, { type: 'idle', t: rand(1, 2) }];
            held = null;
        }
        body.addEventListener('pointerup', function () { release(true); });
        body.addEventListener('pointercancel', function () { release(false); });
    }
    document.addEventListener('touchmove', function (e) { if (held) e.preventDefault(); }, { passive: false });

    function poke(w) {
        if (w.el.classList.contains('sleep')) {
            drop(w);
            burst(w.x, w.y - 30, 'angry', 8);
            w.plan.push({ type: 'idle', t: 2.5, cls: 'angry' });
            return;
        }
        w.pokes = (w.pokes || []).filter(function (t) { return clock - t < 2; });
        w.pokes.push(clock);
        if (w.pokes.length >= 4) {
            w.pokes = [];
            burst(w.x, w.y - HEIGHT, 'angry', 8);
            var now = busy(w) && w.plan[0].type !== 'held' ? w.plan.shift() : null;
            putBack(w);
            drop(w);
            if (now) w.plan.push(now);
            w.plan.push({ type: 'idle', t: 2.5, cls: 'angry' });
            return;
        }
        if (w.npc) bubble(w, 'https://www.brilliantw.net/img/' + encodeURI('關於飯娘/' + pick(MASCOT_EMOTES)));
        burst(w.x, w.y - HEIGHT, 'happy', 5);
        burst(w.x, w.y - HEIGHT - 6, 'heart', 1);
        if (busy(w) || w.carry || !surfaceOf(w.s)) return;
        drop(w);
        w.plan.push({ type: 'jump', to: w.s, x: w.x }, { type: 'idle', t: rand(0.5, 1.5) });
    }
    function bite(w, image) {
        burst(w.x + (w.el.classList.contains('left') ? -1 : 1) * w.half * 0.4, w.y - 36, 'crumb', 4, image);
    }
    function greet(a, b) {
        var mid = (a.x + b.x) / 2;
        var gap = (a.half + b.half) / 2 + 6;
        var s = surfaceOf(a.s);
        var right = a.x <= b.x ? b : a;
        [a, b].forEach(function (p) {
            var side = p === right ? 1 : -1;
            drop(p);
            p.plan.push({ type: 'walk', x: clamp(mid + side * gap, s.left + 4, s.right - 4) }, { type: 'idle', t: rand(1.2, 1.8), cls: 'wave', left: side > 0 }, { type: 'idle', t: 0.4 });
            p.cd = clock + (a.guild && a.guild === b.guild ? rand(8, 15) : rand(20, 40));
        });
    }
    function knock(w, a) {
        for (var k in walkers) {
            var o = walkers[k];
            if (o === w || o.el.hidden || (a.hits && a.hits.indexOf(o) >= 0)) continue;
            var t = o.plan[0] && o.plan[0].type;
            if (t === 'held' || t === 'fly') continue;
            if (Math.abs(o.x - w.x) > (o.half + w.half) * 0.8 || w.y < o.y - HEIGHT + 8 || w.y - HEIGHT + 8 > o.y) continue;
            (a.hits = a.hits || []).push(o);
            putBack(o, true);
            drop(o);
            o.s = null;
            o.el.classList.remove('pressed');
            o.plan.push({ type: 'fly', vx: a.vx * 0.6 + (o.x < w.x ? -120 : 120), vy: -260 - Math.abs(a.vy) * 0.3, hits: [w] }, { type: 'idle', t: rand(1, 2) });
            burst(o.x, o.y - 40, 'smoke', 4);
            a.vx *= 0.75;
        }
    }
    function cheer() {
        for (var k in walkers) {
            var w = walkers[k];
            var s = surfaceOf(w.s);
            if (!s || !s.view || w.el.hidden || busy(w)) continue;
            putBack(w);
            drop(w);
            w.plan.push({ type: 'idle', t: rand(0, 0.4) }, { type: 'jump', to: w.s, x: w.x }, { type: 'jump', to: w.s, x: w.x }, { type: 'idle', t: 1 });
            burst(w.x, w.y - HEIGHT - 4, 'happy', 3);
        }
    }

    function dropCake(x, y, coin, food) {
        if (disabled || cakes.length >= (coin ? 8 : 5)) return;
        var t = below(x, y);
        if (!t) return;
        var el = document.createElement('span');
        el.className = 'walker-cake' + (coin ? ' coin' : '');
        el.style.backgroundImage = icon(coin ? COINS[coin][0] : food || 'item/cake');
        el.setAttribute('aria-hidden', 'true');
        layer.appendChild(el);
        var c = { el: el, coin: coin || false, food: food, x: clamp(x, t.left + 8, t.right - 8), y: y, vy: 0, to: t.el, s: null, born: clock };
        cakes.push(c);
        placeCake(c);
    }
    function placeCake(c) {
        var size = c.coin ? 18 : 24;
        c.el.style.translate = Math.round(c.x - size / 2) + 'px ' + Math.round(c.y - size) + 'px';
    }
    function stepQueue() {
        var count = all().filter(function (w) { return w.waiting && !w.el.hidden; }).length;
        var g = surfaceOf(ground);
        if (!count || !g) {
            if (sign) sign.remove();
            sign = null;
            return;
        }
        if (!sign) {
            sign = document.createElement('span');
            sign.className = 'walker-queue-sign';
            sign.setAttribute('aria-hidden', 'true');
            layer.appendChild(sign);
        }
        sign.textContent = '等待重啟中 · ' + count + ' 人';
        sign.style.transform = 'translate(' + Math.round(g.left + 16) + 'px,' + Math.round(g.top - 92) + 'px)';
    }
    function stepCoins() {
        if (!coinAt) coinAt = clock + rand(40, 80);
        if (clock < coinAt) return;
        coinAt = clock + rand(50, 110);
        var spots = surfaces.filter(function (t) { return t.view && t.right - t.left > 100; });
        if (!spots.length) return;
        for (var n = Math.floor(rand(1, 4)); n > 0; n--) {
            var t = pick(spots);
            dropCake(rand(t.left + 12, t.right - 12), viewTop + rand(4, 40), 'aether');
        }
    }
    function removeCake(c, poof) {
        var i = cakes.indexOf(c);
        if (i < 0) return;
        cakes.splice(i, 1);
        if (poof) burst(c.x, c.y - 12, 'smoke', 6);
        c.el.remove();
    }
    function stepCake(c, dt) {
        var t = surfaceOf(c.s || c.to);
        if (!t || clock - c.born > 20 || (c.eater && clock - c.eatAt > 2.5)) return removeCake(c, true);
        if (c.s) c.y = t.top;
        else {
            c.vy += 900 * dt;
            c.y += c.vy * dt;
            if (c.y >= t.top) {
                c.y = t.top;
                c.s = c.to;
                for (var k in walkers) {
                    var w = walkers[k];
                    var ws = surfaceOf(w.s);
                    if (!ws || ws.walker || w.el.hidden || w.carry || !calm(w.plan[0]) || Math.abs(w.x - c.x) > 400 || Math.abs(w.y - c.y) > 300) continue;
                    drop(w);
                    if (w.s === c.s || planMove(w, ws, t)) goEat(w, c);
                }
            }
        }
        placeCake(c);
    }
    function cakeOn(el) {
        for (var i = 0; i < cakes.length; i++) if (cakes[i].s === el && !cakes[i].eater) return cakes[i];
        return null;
    }
    function goEat(w, c) {
        var t = surfaceOf(c.s);
        var side = w.x < c.x ? -1 : 1;
        w.plan.push({ type: 'walk', x: clamp(c.x + side * (w.half + 6), t.left + 4, t.right - 4), fast: true });
        w.plan.push({ type: 'eat', cake: c });
    }

    function sync(names, guilds, info) {
        names = (names || []).slice(0, MAX);
        known = names;
        if (info) {
            infoOf = info;
            all().forEach(applyInfo);
        }
        if (guilds) {
            guildOf = guilds;
            all().forEach(function (w) {
                w.guild = guildOf[w.name];
                label(w);
            });
        }
        Object.keys(walkers).forEach(function (name) { if (names.indexOf(name) < 0 && !walkers[name].npc && !walkers[name].me) remove(name, true); });
        queue = disabled ? [] : names.filter(function (name) { return !walkers[name]; });
    }
    function spawn() {
        if (!queue.length) {
            ready = true;
            return;
        }
        measure();
        add(queue.shift());
    }

    function reach(s, t) {
        if (t.el === s.el) return false;
        var overlap = Math.min(s.right, t.right) - Math.max(s.left, t.left);
        var gap = Math.max(t.left - s.right, s.left - t.right);
        return overlap > 24 || (gap >= 0 && gap <= JUMP_GAP && Math.abs(t.top - s.top) <= JUMP_UP);
    }
    function planMove(w, s, to) {
        var options = to ? (reach(s, to) ? [to] : []) : surfaces.filter(function (t) { return reach(s, t); });
        if (!options.length) return false;
        var t = pick(seen(options));
        var lo = Math.max(s.left, t.left) + 6;
        var hi = Math.min(s.right, t.right) - 6;
        var rise = s.top - t.top;
        if (lo > hi) {
            var right = t.left > s.right;
            w.plan.push({ type: 'walk', x: right ? s.right - 4 : s.left + 4 });
            w.plan.push({ type: 'jump', to: t.el, x: right ? t.left + 12 : t.right - 12 });
        } else if (rise > 0) {
            var edge = Math.abs(w.x - t.left) < Math.abs(w.x - t.right) ? t.left - 6 : t.right + 6;
            var x = clamp(edge, s.left + 4, s.right - 4);
            var inward = x < (t.left + t.right) / 2 ? 14 : -14;
            w.plan.push({ type: 'walk', x: x });
            if (rise <= JUMP_UP) w.plan.push({ type: 'jump', to: t.el, x: x + inward });
            else if (rise <= JUMP_UP + BLOCK * 6 && Math.random() < 0.5) w.plan.push({ type: 'pillar', to: t.el, x: x + inward });
            else {
                w.plan.push({ type: 'climb', to: t.el, face: inward < 0 });
                w.plan.push({ type: 'walk', x: x + inward });
            }
        } else {
            var down = Math.abs(w.x - s.left) < Math.abs(w.x - s.right) ? s.left + 4 : s.right - 4;
            var dx = clamp(down, lo, hi);
            w.plan.push({ type: 'walk', x: dx });
            w.plan.push(-rise <= JUMP_DOWN ? { type: 'jump', to: t.el, x: clamp(dx + rand(-20, 20), t.left + 6, t.right - 6) } : { type: 'fall', to: t.el });
        }
        return true;
    }
    function beside(s, r) {
        return r.top + r.height > s.top - HEIGHT - 10 && r.top < s.top + 40 && r.left + r.width > s.left && r.left < s.right;
    }
    function planPick(w, s) {
        var props = [].slice.call(layer.parentElement.querySelectorAll(PROPS)).filter(function (p) {
            if (p.closest('.walker-layer') || p === s.el || p.contains(s.el)) return false;
            if (taken.some(function (t) { return t.contains(p) || p.contains(t); })) return false;
            var b = p.getBoundingClientRect();
            if (b.width < 10 || b.height < 10 || b.width > 360 || b.height > 160 || !visible(p, b)) return false;
            return beside(s, relRect(p));
        });
        if (!props.length) return false;
        var prop = pick(props);
        var r = relRect(prop);
        var half = w.half || 15;
        var x = w.x < r.left + r.width / 2 ? r.left - half - 2 : r.left + r.width + half + 2;
        taken.push(prop);
        var spot = clamp(x, s.left + 4, s.right - 4);
        var away = clamp(spot + (Math.random() < 0.5 ? -1 : 1) * rand(60, 140), s.left + 4, s.right - 4);
        w.plan.push({ type: 'walk', x: spot }, { type: 'pick', prop: prop }, { type: 'walk', x: away }, { type: 'look', t: rand(1.2, 2.2) },
            { type: 'walk', x: spot }, { type: 'putback' });
        return true;
    }
    function planStack(w, s) {
        var ridden = all().map(function (p) { return p.s; });
        var o = pick(all().filter(function (o) {
            if (o === w || Math.abs(o.x - w.x) > 200 || ridden.indexOf(o.el) >= 0 || !headOf(o)) return false;
            var root = o;
            var h = 0;
            while (root.s && root.s.walker && h < 8) {
                if (root === w) return false;
                root = root.s.walker;
                h++;
            }
            return root !== w && root.s === s.el && h < 3;
        }));
        if (!o) return false;
        var side = w.x < o.x ? -1 : 1;
        w.plan.push({ type: 'walk', x: clamp(o.x + side * (o.half + w.half + 2), s.left + 4, s.right - 4) });
        w.plan.push({ type: 'jump', to: o.el, ride: o });
        return true;
    }
    function planSnatch(w, s) {
        var c = pick(all().filter(function (o) {
            return o !== w && o.s === s.el && o.carry && o.plan[0] && o.plan[0].type === 'look' && clock > (o.tugUntil || 0) && Math.abs(o.x - w.x) < 250;
        }));
        if (!c) return false;
        c.tugUntil = clock + 8;
        var side = w.x < c.x ? -1 : 1;
        w.plan.push({ type: 'walk', x: clamp(c.x + side * (c.half + w.half - 6), s.left + 4, s.right - 4), fast: true });
        w.plan.push({ type: 'tug', other: c });
        return true;
    }
    function planGuild(w, s) {
        var byGuild = Boolean(w.guild);
        if (!byGuild && (!w.color || Math.random() < 0.4)) return false;
        var mate = all().filter(function (o) {
            return o !== w && !o.npc && (byGuild ? o.guild === w.guild : o.color === w.color) && o.s === s.el && !o.el.hidden && Math.abs(o.x - w.x) < 500;
        }).sort(function (a, b) { return Math.abs(a.x - w.x) - Math.abs(b.x - w.x); })[0];
        if (!mate) return false;
        w.plan.push({ type: 'follow', who: mate, t: rand(8, 16) });
        return true;
    }
    function adopt() {
        if (!meName || disabled) return;
        var w = walkers[meName];
        if (w) {
            w.me = true;
            w.el.classList.add('me');
        } else if (queue.indexOf(meName) < 0) {
            measure();
            if (!surfaces.length) return;
            w = add(meName);
            w.me = true;
            w.el.classList.add('me');
        }
        var first = true;
        try {
            first = !sessionStorage.getItem('walker-hi');
            sessionStorage.setItem('walker-hi', '1');
        } catch (e) { }
        if (first) setTimeout(function () { welcomeMe(0); }, 3500);
    }
    function welcomeMe(tries) {
        var w = walkers[meName];
        if (!w || w.el.hidden || !surfaceOf(w.s)) return tries < 8 && setTimeout(function () { welcomeMe(tries + 1); }, 1500);
        chat([['飯娘', '#E6BBF6'], ['：歡迎回來，', '#FFFFFF'], [meName, '#7fe3ff'], ['！', '#FFFFFF']]);
        callMascot(w);
    }
    function callMascot(w) {
        var m = mascot;
        if (!m || walkers[m.name] !== m || m.el.hidden) return burst(w.x, w.y - HEIGHT, 'heart', 3);
        var s = surfaceOf(w.s);
        if (!s || s.walker) return;
        var side = m.x < w.x ? -1 : 1;
        drop(m);
        if (m.s !== w.s) m.plan.push({ type: 'rain', to: w.s, x: clamp(w.x + side * 50, s.left + 4, s.right - 4) });
        m.plan.push({ type: 'walk', x: clamp(w.x + side * (w.half + m.half + 6), s.left + 4, s.right - 4), fast: true },
            { type: 'call', fn: function () { burst((w.x + m.x) / 2, w.y - HEIGHT - 4, 'heart', 3); } },
            { type: 'idle', t: 1.6, cls: 'wave', left: side > 0 }, { type: 'idle', t: 0.4 });
    }
    function closeMenu() {
        if (!menuEl) return;
        menuEl.parentElement.classList.remove('menu-open');
        menuEl.remove();
        menuEl = null;
    }
    function act(w, kind) {
        if (kind === 'follow') {
            w.follow = !w.follow;
            drop(w);
            return burst(w.x, w.y - HEIGHT, 'happy', 4);
        }
        if (kind === 'emote') {
            if (!emojis) bubble(w);
            return setTimeout(function () {
                if (w.bubble) w.bubble.remove();
                w.bubble = null;
                bubble(w);
            }, emojis && emojis.length ? 0 : 700);
        }
        if (kind === 'mascot') return callMascot(w);
        if (busy(w)) return;
        putBack(w);
        drop(w);
        if (kind === 'wave') w.plan.push({ type: 'idle', t: 1.6, cls: 'wave' }, { type: 'idle', t: 0.4 });
        if (kind === 'jump') w.plan.push({ type: 'jump', to: w.s, x: w.x }, { type: 'jump', to: w.s, x: w.x }, { type: 'idle', t: 0.5 });
        if (kind === 'sit') w.plan.push({ type: 'idle', t: 8, cls: 'sit', sit: SIT_FLAT });
        if (kind === 'dance') w.plan.push({ type: 'idle', t: 6, cls: 'dance' });
        if (kind === 'sleep') w.plan.push({ type: 'idle', t: 10, cls: 'sleep' });
    }
    function menu(w) {
        if (menuEl && menuEl.parentElement === w.el) return closeMenu();
        closeMenu();
        menuEl = document.createElement('div');
        menuEl.className = 'walker-menu';
        [['揮手', 'wave'], ['跳一下', 'jump'], ['坐下', 'sit'], ['跳舞', 'dance'], ['睡覺', 'sleep'], ['冒表情', 'emote'],
            [w.follow ? '停止跟隨' : '跟著游標', 'follow'], ['叫飯娘', 'mascot']].forEach(function (item) {
            var b = document.createElement('button');
            b.type = 'button';
            b.textContent = item[0];
            b.addEventListener('click', function (e) {
                e.stopPropagation();
                closeMenu();
                act(w, item[1]);
            });
            menuEl.appendChild(b);
        });
        w.el.appendChild(menuEl);
        w.el.classList.add('menu-open');
    }
    document.addEventListener('click', function (e) {
        if (menuEl && e.target.closest && !e.target.closest('.walker-menu, .walker.me')) closeMenu();
    });
    function bubble(w, src) {
        if (src) {
            if (w.bubble) w.bubble.remove();
            var own = document.createElement('span');
            own.className = 'walker-bubble';
            own.innerHTML = '<img alt="">';
            own.firstChild.src = src;
            w.el.appendChild(own);
            w.bubble = own;
            setTimeout(function () {
                own.remove();
                if (w.bubble === own) w.bubble = null;
            }, 2600);
            return;
        }
        if (!emojis) {
            emojis = [];
            (window.walkerFetch || fetch)('https://www.brilliantw.net/api/emojis').then(function (r) { return r.ok ? r.json() : null; }).then(function (d) { if (d && d.emojis) emojis = d.emojis; }).catch(function () { });
            return;
        }
        if (!emojis.length || w.bubble) return;
        var e = pick(emojis);
        var b = document.createElement('span');
        b.className = 'walker-bubble';
        b.innerHTML = '<img alt="">';
        b.firstChild.src = 'https://cdn.discordapp.com/emojis/' + e.id + '.webp?size=44';
        b.firstChild.alt = ':' + e.name + ':';
        w.el.appendChild(b);
        w.bubble = b;
        setTimeout(function () {
            b.remove();
            w.bubble = null;
        }, 2600);
    }

    function startGame() {
        var spots = surfaces.filter(function (t) { return t.view && t.right - t.left >= 200; });
        var options = spots.map(function (t) {
            return { t: t, crowd: all().filter(function (w) { return w.s === t.el && viewed(w); }) };
        }).filter(function (o) { return o.crowd.length >= 2; });
        if (!options.length) return;
        var o = pick(options);
        var kind = pick(GAMES);
        var anchor = pick(o.crowd);
        var x = clamp(anchor.x, o.t.left + 80, o.t.right - 80);
        var players = o.crowd.sort(function (a, b) { return Math.abs(a.x - x) - Math.abs(b.x - x); }).slice(0, kind.max);
        if (players.length < 2) return;
        var el = document.createElement('div');
        el.className = 'walker-table' + (kind.board ? ' board' : '');
        el.setAttribute('aria-hidden', 'true');
        el.innerHTML = '<span class="walker-table-name"></span><span class="walker-table-top"></span><span class="walker-table-leg"></span>';
        el.firstChild.textContent = kind.name;
        if (!kind.board) el.children[1].style.backgroundImage = icon('block/' + kind.carpet);
        layer.appendChild(el);
        game = { el: el, s: o.t.el, x: x, kind: kind, players: players, turn: 0, next: clock + 1.5, began: clock, end: 0 };
        var count = { '-1': 0, '1': 0 };
        players.forEach(function (p, i) {
            var side = i % 2 ? 1 : -1;
            var slot = x + side * (44 + count[side] * (p.half * 2 + 4));
            count[side]++;
            drop(p);
            p.plan.push({ type: 'walk', x: clamp(slot, o.t.left + 4, o.t.right - 4), fast: true, game: game }, { type: 'idle', t: 60, cls: 'sit', sit: SIT_FLAT, left: side > 0, game: game });
        });
    }
    function seated(g) {
        return g.players.filter(function (p) { return walkers[p.name] === p && p.plan[0] && p.plan[0].game === g && p.plan[0].on; });
    }
    function piece(g) {
        var top = g.el.children[1];
        var p = document.createElement('span');
        p.className = 'walker-piece ' + g.kind.piece;
        var v = g.kind.pick();
        if (g.kind.piece === 'card') p.style.backgroundImage = icon(v);
        else if (g.kind.piece === 'checker') p.classList.add(v);
        else {
            p.textContent = v;
            if (g.kind.piece === 'xq' && '帥俥傌炮兵'.indexOf(v) >= 0) p.classList.add('red');
            if (g.kind.piece === 'tile' && v === '中') p.classList.add('red');
            if (g.kind.piece === 'tile' && v === '發') p.classList.add('green');
        }
        top.appendChild(p);
        while (top.children.length > 5) top.firstChild.remove();
    }
    function endGame(g) {
        var sitting = seated(g);
        var winner = pick(sitting);
        sitting.forEach(function (p) {
            drop(p);
            if (p === winner) {
                p.plan.push({ type: 'jump', to: p.s, x: p.x }, { type: 'jump', to: p.s, x: p.x }, { type: 'idle', t: 0.8 });
                burst(p.x, p.y - HEIGHT, 'happy', 8);
            } else {
                p.plan.push({ type: 'idle', t: 1.5, cls: 'angry' });
                burst(p.x, p.y - HEIGHT, 'angry', 4);
            }
        });
        g.players.forEach(function (p) {
            if (p.plan[0] && p.plan[0].game === g) drop(p);
        });
        burst(g.x, (surfaceOf(g.s) || { top: 0 }).top - 20, 'smoke', 6);
        g.el.remove();
        game = null;
        gameAt = clock + rand(60, 120);
    }
    function stepGame() {
        if (!game) {
            if (!gameAt) gameAt = clock + rand(25, 50);
            if (clock > gameAt) {
                gameAt = clock + rand(20, 40);
                startGame();
            }
            return;
        }
        var g = game;
        var t = surfaceOf(g.s);
        if (!t) return endGame(g);
        g.el.style.translate = Math.round(g.x - 32) + 'px ' + Math.round(t.top - 30) + 'px';
        var sitting = seated(g);
        if (!g.end) {
            if (sitting.length === g.players.length || (clock - g.began > 8 && sitting.length >= 2)) g.end = clock + rand(9, 15);
            else if (clock - g.began > 12) return endGame(g);
            return;
        }
        if (sitting.length < 2 || clock > g.end) return endGame(g);
        if (clock > g.next) {
            g.next = clock + rand(0.7, 1.1);
            var p = sitting[g.turn++ % sitting.length];
            p.el.classList.remove('play');
            void p.el.offsetWidth;
            p.el.classList.add('play');
            piece(g);
        }
    }

    function startMeteor() {
        var shown = all().filter(viewed);
        if (!shown.length) return;
        var o = layer.getBoundingClientRect();
        var x0 = -o.left + window.innerWidth * rand(0.55, 0.95);
        var y0 = viewTop + rand(30, 90);
        var el = document.createElement('span');
        el.className = 'walker-meteor';
        el.setAttribute('aria-hidden', 'true');
        layer.appendChild(el);
        meteor = { el: el, x0: x0, y0: y0, dx: -rand(320, 520), dy: rand(90, 170), t: 0 };
        chat([['✦ ', '#55FFFF'], ['一顆流星劃過天際……', '#FFFFFF'], ['蹲下並手持', '#AAAAAA'], ['水芥', '#FFFFFF'], ['許願', '#AAAAAA']]);
        shown.forEach(function (w) {
            if (Math.random() < 0.3) return;
            drop(w);
            w.plan.push({ type: 'idle', t: rand(0.3, 1.2) }, {
                type: 'idle', t: rand(2.5, 4), cls: 'wish', done: function () { grant(w); }
            });
        });
    }
    function grant(w) {
        var item = w.el.querySelector('.walker-held');
        if (item) item.remove();
        if (Math.random() >= 0.16) {
            burst(w.x, w.y - HEIGHT, 'smoke', 3);
            return;
        }
        var star = document.createElement('span');
        star.className = 'walker-shine';
        star.setAttribute('aria-hidden', 'true');
        star.style.translate = Math.round(w.x) + 'px ' + Math.round(w.y - HEIGHT - 10) + 'px';
        star.addEventListener('animationend', function () { star.remove(); });
        layer.appendChild(star);
        burst(w.x, w.y - HEIGHT, 'happy', 10);
        chat([['✦ ', '#55FFFF'], ['流星回應了 ', '#AAAAAA'], [w.name, '#FFFFFF'], [' 的願望！', '#AAAAAA']]);
    }
    function stepMeteor(dt) {
        if (!meteor) {
            if (!meteorAt) meteorAt = clock + rand(60, 150);
            if (clock > meteorAt) {
                meteorAt = clock + rand(150, 300);
                startMeteor();
            }
            return;
        }
        var m = meteor;
        m.t += dt / 1.5;
        var k = Math.min(1, m.t);
        var x = m.x0 + m.dx * k;
        var y = m.y0 + m.dy * k - 30 * Math.sin(Math.PI * k);
        m.el.style.transform = 'translate(' + Math.round(x) + 'px,' + Math.round(y) + 'px) rotate(' + Math.round(Math.atan2(m.dy, m.dx) * 180 / Math.PI + 180) + 'deg)';
        m.el.style.opacity = k < 0.85 ? 1 : (1 - k) / 0.15;
        if (k >= 1) {
            m.el.remove();
            meteor = null;
        }
    }

    function stepMusic(dt) {
        var floor = surfaceOf(ground);
        if (!jukebox) {
            if (!floor) return;
            jukebox = document.createElement('span');
            jukebox.className = 'walker-jukebox';
            jukebox.setAttribute('role', 'button');
            jukebox.setAttribute('tabindex', '0');
            jukebox.setAttribute('aria-label', '留聲機器：播放唱片');
            jukebox.title = '留聲機器：點一下播放唱片';
            jukebox.style.backgroundImage = icon('item/jukebox');
            jukebox.addEventListener('click', toggleMusic);
            jukebox.addEventListener('keydown', function (e) { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleMusic(); } });
            layer.appendChild(jukebox);
        }
        if (floor) jukebox.style.transform = 'translate(' + Math.round(floor.right - 40) + 'px,' + Math.round(floor.top - 28) + 'px)';
        if (!music || !music.ready) return;
        if ((music.note -= dt) <= 0) {
            music.note = 0.35;
            if (floor) burst(floor.right - 26, floor.top - 34, 'tune', 1);
        }
        all().forEach(function (w) {
            if (!viewed(w)) return;
            drop(w);
            w.plan.push({ type: 'idle', t: 600, cls: 'dance', music: music });
        });
        all().forEach(function (w) {
            var a = w.plan[0];
            if (a && a.music === music && Math.random() < dt * 0.4) burst(w.x, w.y - HEIGHT - 4, 'tune', 1);
        });
    }
    function toggleMusic() {
        if (music) return stopMusic();
        var AC = window.AudioContext || window.webkitAudioContext;
        var mine = music = { note: 0, ac: AC ? new AC() : null };
        jukebox.classList.add('loading');
        if (!discs) discs = (window.walkerFetch || fetch)('https://www.brilliantw.net/api/discs').then(function (r) { return r.ok ? r.json() : null; }).then(function (d) { return (d && d.discs) || []; }).catch(function () { return []; });
        discs.then(function (list) {
            if (music !== mine) return;
            if (!list.length || !mine.ac) return playDisc(mine);
            playRecord(mine, pick(list)).catch(function () { if (music === mine) playDisc(mine); });
        });
    }
    function playDisc(m) {
        var disc = pick(DISCS);
        var audio = new Audio('https://www.brilliantw.net/gramophone/sound/minecraft/records/' + disc + '.ogg');
        audio.volume = 0.35;
        m.stop = function () { audio.pause(); };
        audio.addEventListener('ended', function () { if (music === m) stopMusic(); });
        audio.addEventListener('playing', function () {
            if (music !== m || m.ready) return;
            begin(m);
            chat([['正在播放：', '#FFAA00'], ['C418 - ' + disc, '#FFFFFF']]);
        });
        audio.play().catch(function () { if (music === m) stopMusic(); });
    }
    function begin(m) {
        m.ready = true;
        jukebox.classList.remove('loading');
        jukebox.classList.add('on');
    }
    function playRecord(m, rec) {
        var engine = window.GramophoneEngine;
        if (!engine) return Promise.reject(new Error('no engine'));
        return (window.walkerFetch || fetch)('https://www.brilliantw.net/gramophone/rec/' + encodeURIComponent(rec.server) + '/' + encodeURIComponent(rec.id), { credentials: 'same-origin' })
            .then(function (r) { if (!r.ok) throw new Error(r.status); return r.json(); })
            .then(function (tl) {
                return Promise.all(tl.files.map(engine.buffer)).then(function (loaded) {
                    if (music !== m) return;
                    var ac = m.ac;
                    var source = ac.createBufferSource();
                    source.buffer = engine.mix(tl, loaded);
                    source.connect(ac.destination);
                    if (ac.resume) ac.resume();
                    source.start(ac.currentTime + 0.05);
                    source.onended = function () { if (music === m) stopMusic(); };
                    m.stop = function () { try { source.stop(); } catch (e) { } };
                    begin(m);
                    chat([['正在播放：', '#FFAA00'], [rec.name || '留聲片', '#FFFFFF'], [' — ' + (rec.owner || ''), '#AAAAAA']]);
                });
            });
    }
    function stopMusic() {
        var old = music;
        music = null;
        if (!old) return;
        if (old.stop) old.stop();
        clearTimeout(old.timer);
        if (old.ac && old.ac.close) old.ac.close();
        jukebox.classList.remove('on', 'loading');
        all().forEach(function (w) {
            var a = w.plan[0];
            if (a && a.music === old) a.t = 0;
        });
    }

    function loadDex() {
        if (dex || mascotOnly) return;
        dex = { gacha: [], dessert: [], furniture: [] };
        (window.walkerFetch || fetch)('https://www.brilliantw.net/api/walkerdex').then(function (r) { return r.ok ? r.json() : null; }).then(function (d) { if (d) dex = d; }).catch(function () { });
    }
    function hold(w, path, cls) {
        var el = document.createElement('span');
        el.className = 'walker-held' + (cls ? ' ' + cls : '');
        el.style.backgroundImage = icon(path);
        w.el.appendChild(el);
        return el;
    }
    function planSnack(w) {
        if (!dex.dessert.length) return false;
        var snack = pick(dex.dessert);
        var held;
        w.plan.push({ type: 'call', fn: function () { held = hold(w, snack.icon); gain(w, snack.name, snack.color); } },
            { type: 'idle', t: 2.2, cls: 'munch', img: icon(snack.icon), done: function () { if (held) held.remove(); burst(w.x, w.y - HEIGHT - 6, 'heart', 1); } });
        return true;
    }
    function planGacha(w) {
        if (!dex.gacha.length) return false;
        var machine = pick(dex.gacha);
        var prize = pick(machine.items);
        w.plan.push({ type: 'call', fn: function () { openGacha(w, machine, prize); } }, { type: 'idle', t: 1.2, cls: 'offer' }, { type: 'idle', t: 2 });
        return true;
    }
    function openGacha(w, machine, prize) {
        var el = document.createElement('div');
        el.className = 'walker-catch shake';
        el.setAttribute('aria-hidden', 'true');
        el.innerHTML = '<span class="walker-catch-name"></span>';
        el.style.backgroundImage = icon(machine.icon);
        el.style.left = Math.round(w.x - 14) + 'px';
        el.style.top = Math.round(w.y - HEIGHT - 38) + 'px';
        layer.appendChild(el);
        setTimeout(function () {
            el.classList.remove('shake');
            el.style.backgroundImage = icon(prize.icon);
            el.firstChild.textContent = prize.name;
            el.firstChild.style.color = prize.color;
            burst(w.x, w.y - HEIGHT - 24, 'happy', 8);
            burst(w.x, w.y - HEIGHT - 24, 'smoke', 4);
            chat([[w.name, '#FFFFFF'], [' 從【' + machine.name + '】轉到了 ', '#AAAAAA'], [prize.name, prize.color]]);
        }, 1000);
        setTimeout(function () { el.classList.add('gone'); }, 2800);
        setTimeout(function () { el.remove(); }, 3200);
    }
    function planFurniture(w, s) {
        if (!dex.furniture.length || s.right - s.left < 90) return false;
        var item = pick(dex.furniture);
        var dir = w.el.classList.contains('left') ? -1 : 1;
        var x = clamp(w.x + dir * (w.half + 22), s.left + 18, s.right - 18);
        var seat = item.seat;
        var flip = seat && seat.back ? Math.random() < 0.5 : dir < 0;
        var f = null;
        var end = { type: 'call', fn: function () { if (f) breakFurniture(f); } };
        w.plan.push({ type: 'idle', t: 0.35, cls: 'offer', left: dir < 0 }, { type: 'call', fn: function () { f = placeFurniture(w, s.el, x, item, flip, end); } }, { type: 'idle', t: 0.3 });
        if (seat) {
            var faceLeft = seat.back ? (seat.back > 0) !== flip : Math.random() < 0.5;
            var seatX = x - FURNI / 2 + (flip ? 1 - seat.x : seat.x) * FURNI;
            w.plan.push({ type: 'walk', x: seatX }, { type: 'idle', t: rand(4, 8), cls: 'sit', sit: SIT_EDGE - FURNI * (1 - seat.y), left: faceLeft });
        } else {
            w.plan.push({ type: 'walk', x: clamp(x - dir * (w.half + 18), s.left + 4, s.right - 4) }, { type: 'look', t: rand(2, 3.5), left: dir < 0 });
        }
        w.plan.push(end, { type: 'idle', t: 0.5 });
        return true;
    }
    function placeFurniture(w, el, x, item, flip, end) {
        var f = { el: document.createElement('span'), s: el, x: x, item: item, flip: flip, owner: w, end: end };
        f.el.className = 'walker-furniture';
        f.el.setAttribute('aria-hidden', 'true');
        f.el.style.backgroundImage = icon(item.icon);
        layer.appendChild(f.el);
        furniture.push(f);
        placeOne(f);
        var t = surfaceOf(el);
        if (t) burst(x, t.top - 8, 'smoke', 5);
        return f;
    }
    function placeOne(f) {
        var t = surfaceOf(f.s);
        if (!t) return breakFurniture(f);
        f.el.style.translate = Math.round(f.x - FURNI / 2) + 'px ' + Math.round(t.top - FURNI) + 'px';
        f.el.style.transform = f.flip ? 'scaleX(-1)' : '';
    }
    function breakFurniture(f) {
        var i = furniture.indexOf(f);
        if (i < 0) return;
        furniture.splice(i, 1);
        var t = surfaceOf(f.s);
        if (t) burst(f.x, t.top - 14, 'crumb', 10, icon(f.item.icon));
        f.el.remove();
    }
    function aura(w, dt, moving) {
        if (w.npc) {
            if (Math.random() < dt * (moving ? 1.6 : 0.6)) burst(w.x + rand(-14, 14), w.y - HEIGHT, 'petal', 1);
        } else if (w.donor && moving && Math.random() < dt * (0.8 + w.donor.l / 2500)) {
            burst(w.x - (w.el.classList.contains('left') ? -8 : 8), w.y - 6, 'star', 1, w.donor.c);
        }
    }
    function buddy(w, s) {
        return all().filter(function (o) {
            return o !== w && !o.npc && o.s === s.el && !o.el.hidden && !o.carry && !o.waiting && calm(o.plan[0]) && Math.abs(o.x - w.x) < 500;
        }).sort(function (a, b) { return Math.abs(a.x - w.x) - Math.abs(b.x - w.x); })[Math.floor(rand(0, 3))];
    }
    function meet(w, o, s) {
        var side = o.x < w.x ? 1 : -1;
        var x = clamp(o.x + side * (o.half + w.half + 4), s.left + 4, s.right - 4);
        drop(o);
        o.plan.push({ type: 'idle', t: 8, left: side < 0, hold: w });
        w.cd = clock + 15;
        o.cd = clock + rand(20, 40);
        return x;
    }
    function free(o, w, steps) {
        var a = o.plan[0];
        if (walkers[o.name] !== o || !a || a.hold !== w) return;
        o.plan.shift();
        o.plan.unshift.apply(o.plan, steps);
    }
    function planVisit(w, s) {
        var o = buddy(w, s);
        if (!o) return false;
        var x = meet(w, o, s);
        var five = Math.random() < 0.5;
        w.plan.push({ type: 'walk', x: x }, {
            type: 'call', fn: function () {
                w.el.classList.toggle('left', o.x < w.x);
                bubble(w);
                setTimeout(function () { if (walkers[o.name] === o) bubble(o); }, 900);
                free(o, w, [{ type: 'idle', t: 1.4, cls: 'wave', left: w.x < o.x }, { type: 'idle', t: 1.6, left: w.x < o.x }].concat(five ? [{ type: 'jump', to: o.s, x: o.x }] : []));
            }
        }, { type: 'idle', t: 1.4, cls: 'wave' }, { type: 'idle', t: 1.6 });
        if (five) w.plan.push({ type: 'call', fn: function () { burst((w.x + o.x) / 2, w.y - HEIGHT, 'happy', 8); } }, { type: 'jump', to: s.el, x: x });
        w.plan.push({ type: 'idle', t: 0.6 });
        return true;
    }
    function planTreat(w, s) {
        if (!dex || !dex.dessert.length) return false;
        var o = buddy(w, s);
        if (!o) return false;
        var snack = pick(dex.dessert);
        var x = meet(w, o, s);
        var held = null;
        w.plan.push({ type: 'walk', x: x }, {
            type: 'call', fn: function () {
                w.el.classList.toggle('left', o.x < w.x);
                held = hold(w, snack.icon);
            }
        }, { type: 'idle', t: 1, cls: 'offer' }, {
            type: 'call', fn: function () {
                if (held) held.remove();
                if (walkers[o.name] !== o || !o.plan[0] || o.plan[0].hold !== w) return;
                chat([[w.name, OWNER_COLOR], [' 請 ', '#AAAAAA'], [o.name, '#FFFFFF'], [' 吃了 ', '#AAAAAA'], [snack.name, snack.color]]);
                var got = null;
                free(o, w, [{ type: 'call', fn: function () { got = hold(o, snack.icon); } },
                    { type: 'idle', t: 2.2, cls: 'munch', img: icon(snack.icon), done: function () { if (got) got.remove(); burst(o.x, o.y - HEIGHT - 6, 'heart', 2); } },
                    { type: 'idle', t: 1, cls: 'wave', left: w.x < o.x }]);
            }
        }, { type: 'idle', t: 2.4, left: o.x < x }, { type: 'idle', t: 0.8, cls: 'wave' });
        return true;
    }
    function planCuddle(w) {
        var back = w.el.classList.contains('left');
        var pet = pick(pets);
        w.plan.push({ type: 'call', fn: function () { w.hug = { left: back, until: clock + 2.6 }; pet.joy = clock + 2.2; } },
            { type: 'idle', t: 2.2, cls: 'offer', left: !back }, { type: 'idle', t: 0.4, left: back });
    }
    function planKiss(w) {
        w.plan.push({ type: 'idle', t: 0.3, cls: 'wave' }, {
            type: 'call', fn: function () {
                burst(w.x, w.y - HEIGHT, 'heart', 6);
                all().forEach(function (o) {
                    if (o === w || o.s !== w.s || o.el.hidden || Math.abs(o.x - w.x) > 220) return;
                    burst(o.x, o.y - HEIGHT - 4, 'heart', 2);
                    if (!calm(o.plan[0]) || o.carry) return;
                    drop(o);
                    o.plan.push({ type: 'idle', t: rand(0.1, 0.5), left: w.x < o.x }, { type: 'jump', to: o.s, x: o.x });
                });
            }
        }, { type: 'idle', t: 1.2, cls: 'wave' });
    }
    function planCookies(w, s) {
        var n = 2 + Math.min(3, Math.floor(w.donor.l / 2500));
        w.plan.push({ type: 'idle', t: 0.5, cls: 'offer' }, {
            type: 'call', fn: function () {
                for (var i = 0; i < n; i++) dropCake(clamp(w.x + rand(-110, 110), s.left + 10, s.right - 10), w.y - HEIGHT - rand(10, 40), false, 'item/cookie');
                burst(w.x, w.y - HEIGHT, 'star', 8, w.donor.c);
                chat([['✦ ', w.donor.c], [w.name, w.donor.c], [' 撒了一把餅乾！', '#FFFFFF']]);
            }
        }, { type: 'idle', t: 1.2, cls: 'wave' });
    }
    function planFirework(w) {
        w.plan.push({ type: 'idle', t: 0.3, cls: 'stretch' }, { type: 'call', fn: function () { launch(w); } }, { type: 'idle', t: 1.4, cls: 'stretch' }, { type: 'idle', t: 0.6, cls: 'wave' });
    }
    function launch(w) {
        var x = w.x;
        var top = w.y - HEIGHT - 150;
        var el = document.createElement('span');
        el.className = 'walker-rocket';
        el.setAttribute('aria-hidden', 'true');
        el.style.backgroundImage = icon('item/firework_rocket');
        el.style.translate = Math.round(x - 9) + 'px ' + Math.round(w.y - HEIGHT - 10) + 'px';
        layer.appendChild(el);
        burst(x, w.y - HEIGHT, 'smoke', 4);
        var colors = [w.donor.c, '#ffffff', '#ffd36b'];
        setTimeout(function () {
            el.remove();
            if (still) return;
            for (var i = 0; i < 24; i++) {
                var angle = i / 24 * Math.PI * 2;
                var r = rand(45, 75);
                var p = document.createElement('span');
                p.className = 'walker-fx star';
                p.textContent = '✦';
                p.style.color = colors[i % colors.length];
                p.style.cssText += '--x0:' + x.toFixed(1) + 'px;--y0:' + top.toFixed(1) + 'px;--x1:' + (x + Math.cos(angle) * r).toFixed(1) + 'px;--y1:' + (top + Math.sin(angle) * r + 18).toFixed(1) + 'px;--t:' + rand(0.9, 1.3).toFixed(2) + 's';
                p.addEventListener('animationend', function () { this.remove(); });
                layer.appendChild(p);
            }
        }, 900);
    }
    function planTrade(w, s) {
        if (!dex.dessert.length && !dex.gacha.length) return false;
        var o = all().filter(function (p) {
            return p !== w && !p.npc && p.s === s.el && !p.el.hidden && !p.carry && calm(p.plan[0]) && Math.abs(p.x - w.x) < 220;
        })[0];
        if (!o) return false;
        var mid = (w.x + o.x) / 2;
        var gap = (w.half + o.half) / 2 + 10;
        var right = w.x <= o.x ? o : w;
        [w, o].forEach(function (p) {
            var side = p === right ? 1 : -1;
            drop(p);
            p.plan.push({ type: 'walk', x: clamp(mid + side * gap, s.left + 4, s.right - 4) }, { type: 'idle', t: 2.8, cls: 'offer', left: side > 0 });
            p.cd = clock + rand(20, 40);
        });
        w.plan.splice(1, 0, { type: 'call', fn: function () { setTimeout(function () { trade(w, o); }, 500); } });
        return true;
    }
    function barterItem() {
        var pool = dex.dessert.concat(dex.gacha.length ? pick(dex.gacha).items : []);
        return Math.random() < 0.3 || !pool.length ? { icon: 'item/emerald' } : pick(pool);
    }
    function trade(a, b) {
        if (walkers[a.name] !== a || walkers[b.name] !== b) return;
        var items = [barterItem(), barterItem()];
        var els = [a, b].map(function (p, i) {
            var el = document.createElement('span');
            el.className = 'walker-trade';
            el.setAttribute('aria-hidden', 'true');
            el.style.backgroundImage = icon(items[i].icon);
            el.style.left = Math.round(p.x - 10) + 'px';
            el.style.top = Math.round(p.y - HEIGHT - 26) + 'px';
            layer.appendChild(el);
            return el;
        });
        setTimeout(function () {
            var l0 = els[0].style.left, t0 = els[0].style.top;
            els[0].style.left = els[1].style.left;
            els[0].style.top = els[1].style.top;
            els[1].style.left = l0;
            els[1].style.top = t0;
        }, 700);
        setTimeout(function () {
            els.forEach(function (el) { el.remove(); });
            [a, b].forEach(function (p) { burst(p.x, p.y - HEIGHT, 'happy', 6); });
        }, 1600);
    }
    function planMine(w, s) {
        var props = [].slice.call(layer.parentElement.querySelectorAll(PROPS)).filter(function (p) {
            if (p.closest('.walker-layer') || p === s.el || p.contains(s.el)) return false;
            if (taken.some(function (t) { return t.contains(p) || p.contains(t); })) return false;
            var b = p.getBoundingClientRect();
            if (b.width < 10 || b.height < 10 || b.width > 360 || b.height > 160 || !visible(p, b)) return false;
            return beside(s, relRect(p));
        });
        if (!props.length) return false;
        var prop = pick(props);
        var r = relRect(prop);
        var near = w.x < r.left + r.width / 2;
        var x = clamp(near ? r.left - w.half : r.left + r.width + w.half, s.left + 4, s.right - 4);
        taken.push(prop);
        w.plan.push({ type: 'walk', x: x }, { type: 'pick', prop: prop, mine: true });
        return true;
    }
    function startMine(w, prop) {
        var crack = document.createElement('span');
        crack.className = 'walker-crack';
        crack.setAttribute('aria-hidden', 'true');
        layer.appendChild(crack);
        var tool = hold(w, 'custom/brilliant/currency/crystal_pickaxe', 'tool');
        var job = { prop: prop, crack: crack, tool: tool, t: 0 };
        w.plan.unshift({
            type: 'idle', t: 2.4, cls: 'mine', mine: job, done: function () { breakProp(w, job); }
        });
    }
    function mineTick(a) {
        var job = a.mine;
        job.t = clamp(1 - a.t / 2.4, 0, 1);
        var r = relRect(job.prop);
        job.crack.style.cssText = 'left:' + r.left + 'px;top:' + r.top + 'px;width:' + r.width + 'px;height:' + r.height + 'px;background-image:' +
            icon('block/destroy_stage_' + Math.min(9, Math.floor(job.t * 10)));
    }
    function breakProp(w, job) {
        job.crack.remove();
        job.tool.remove();
        var r = relRect(job.prop);
        job.prop.style.visibility = 'hidden';
        burst(r.left + r.width / 2, r.top + r.height / 2, 'rubble', 14);
        burst(r.left + r.width / 2, r.top + r.height / 2, 'smoke', 6);
        dropCake(r.left + r.width / 2, r.top + r.height / 2, Math.random() < 0.5 ? 'paradise' : 'luminous');
        mined.push({ prop: job.prop, at: clock + rand(5, 10) });
    }
    function restoreMined(m) {
        mined.splice(mined.indexOf(m), 1);
        var i = taken.indexOf(m.prop);
        if (i >= 0) taken.splice(i, 1);
        m.prop.style.visibility = '';
        if (m.prop.animate) m.prop.animate([{ opacity: 0, transform: 'scale(0.9)' }, { opacity: 1, transform: 'none' }], { duration: 600, easing: 'ease-out' });
    }
    function stepExtras(dt) {
        furniture.slice().forEach(function (f) { if (walkers[f.owner.name] !== f.owner || f.owner.plan.indexOf(f.end) < 0) breakFurniture(f); else placeOne(f); });
        mined.slice().forEach(function (m) { if (clock > m.at || !document.contains(m.prop)) restoreMined(m); });
        if (!mascot || walkers[mascot.name] !== mascot) return;
        if (!pets.length && !mascot.el.hidden) {
            pets = PETS.map(function (p, i) {
                var el = document.createElement('span');
                el.className = 'walker-pet';
                el.setAttribute('aria-hidden', 'true');
                el.appendChild(document.createElement('span')).style.backgroundImage = 'url(https://www.brilliantw.net/img/' + encodeURI('關於飯娘/寵物/' + p[0] + '.png') + ')';
                layer.appendChild(el);
                return { el: el, x: mascot.x, y: mascot.y, i: i, face: p[1] };
            });
        }
        var faceLeft = mascot.hug && clock < mascot.hug.until ? mascot.hug.left : mascot.el.classList.contains('left');
        var dir = faceLeft ? 1 : -1;
        pets.forEach(function (p) {
            var tx = mascot.x + dir * (mascot.half + 10 + p.i * 26);
            var ty = mascot.y - mascot.drop;
            var k = Math.min(1, dt * (4 - p.i * 0.4));
            var nx = p.x + (tx - p.x) * k;
            p.el.classList.toggle('flip', p.face !== 0 && (p.face < 0) !== faceLeft);
            var moving = dt > 0 ? clamp(Math.abs(nx - p.x) / dt / 30, 0, 1) : 0;
            p.amp = (p.amp || 0) + (moving - (p.amp || 0)) * Math.min(1, dt * 8);
            p.x = nx;
            p.y += (ty - p.y) * k;
            var hop = p.amp * Math.abs(Math.sin(clock * 10 - p.i * 0.9)) * 6;
            if (p.joy > clock) {
                hop = Math.max(hop, Math.abs(Math.sin(clock * 9)) * 14);
                if (Math.random() < dt * 4) burst(p.x, p.y - 26, 'heart', 1);
            }
            p.el.style.transform = 'translate(' + Math.round(p.x - 11) + 'px,' + Math.round(p.y - 22 - hop) + 'px)';
        });
    }
    function farewell() {
        stopMusic();
        if (game) endGame(game);
        cakes.slice().forEach(function (c) { removeCake(c, true); });
        furniture.slice().forEach(breakFurniture);
        mined.slice().forEach(restoreMined);
        if (portal) {
            portal.a.el.remove();
            portal.b.el.remove();
            portal = null;
        }
        if (meteor) {
            meteor.el.remove();
            meteor = null;
        }
        if (jukebox) {
            jukebox.remove();
            jukebox = null;
        }
        queue = [];
        all().forEach(function (w) {
            if (held && held.w === w) held = null;
            var s = surfaceOf(w.s);
            if (w.el.hidden || !s || busy(w)) {
                burst(w.x, w.y - 30, 'smoke', 6);
                return remove(w.name);
            }
            putBack(w, true);
            drop(w);
            w.plan.push({ type: 'idle', t: rand(0, 0.4), cls: 'bye' }, { type: 'idle', t: 1.4, cls: 'wave' }, {
                type: 'call', fn: function () { burst(w.x, w.y - 30, 'smoke', 8); remove(w.name); }
            });
        });
        setTimeout(function () {
            if (!disabled) return;
            all().forEach(function (w) {
                burst(w.x, w.y - 30, 'smoke', 8);
                remove(w.name);
            });
        }, 3000);
    }
    function syncButton() {
        var b = document.querySelector('.walker-toggle');
        if (!b) return;
        var label = disabled ? '開啟虛擬夥伴' : '關閉虛擬夥伴';
        b.setAttribute('aria-pressed', String(!disabled));
        b.title = label;
        b.setAttribute('aria-label', label);
    }
    window.toggleWalkers = function () {
        disabled = !disabled;
        try { localStorage.setItem('walkers', disabled ? 'off' : 'on'); } catch (e) { }
        syncButton();
        if (disabled) farewell();
        else {
            queue = known.filter(function (name) { return !walkers[name]; });
            adopt();
        }
    };

    function planEdge(w, s, fish) {
        if (s.el === ground || s.right - s.left < 80) return false;
        var right = Math.abs(w.x - s.right) < Math.abs(w.x - s.left);
        w.plan.push({ type: 'walk', x: right ? s.right - 6 : s.left + 6 });
        w.plan.push(fish ? { type: 'fish', left: !right, t: 0, wait: rand(4, 10) } : { type: 'idle', t: rand(4, 10), cls: 'sit', sit: SIT_EDGE, left: !right });
        return true;
    }
    function spot(end) {
        var t = surfaceOf(end.s);
        return t && { s: end.s, x: end.side < 0 ? t.left + 24 : t.right - 24, y: t.top };
    }
    function planPortal(w, s) {
        var ends = [portal.a, portal.b].filter(function (e) { return e.s === s.el && spot(e); });
        if (!ends.length) return false;
        var end = ends.sort(function (a, b) { return Math.abs(spot(a).x - w.x) - Math.abs(spot(b).x - w.x); })[0];
        var at = spot(end);
        if (!at) return false;
        w.plan.push({ type: 'walk', x: at.x }, { type: 'warp', t: 0, other: end === portal.a ? portal.b : portal.a });
        return true;
    }
    function openPortal() {
        var seenList = surfaces.filter(function (t) { return t.view && t.right - t.left > 120; });
        if (!seenList.length) return;
        var a = seenList.reduce(function (m, t) { return t.left < m.left ? t : m; });
        var b = seenList.reduce(function (m, t) { return t.right > m.right ? t : m; });
        portal = { a: { s: a.el, side: -1 }, b: { s: b.el, side: 1 }, until: clock + 15 };
        [portal.a, portal.b].forEach(function (end) {
            end.el = document.createElement('div');
            end.el.className = 'walker-portal';
            end.el.setAttribute('aria-hidden', 'true');
            layer.appendChild(end.el);
        });
        all().filter(function (w) {
            return (w.s === a.el || w.s === b.el) && !w.el.hidden && !w.carry && calm(w.plan[0]);
        }).sort(function () { return Math.random() - 0.5; }).slice(0, 3).forEach(function (w) {
            var ws = surfaceOf(w.s);
            drop(w);
            if (!planPortal(w, ws)) w.plan.push({ type: 'idle', t: 1 });
        });
    }
    function stepPortal() {
        if (!portal) {
            if (!portalAt) portalAt = clock + rand(20, 40);
            if (clock > portalAt) openPortal();
            return;
        }
        var closing = clock > portal.until;
        [portal.a, portal.b].forEach(function (end) {
            var at = spot(end);
            if (!at) return;
            end.el.style.translate = Math.round(at.x - 20) + 'px ' + Math.round(at.y - 64) + 'px';
            if (Math.random() < 0.08) burst(at.x, at.y - 32, 'portal', 1);
            if (closing) end.el.classList.add('closing');
        });
        if (closing) {
            var old = portal;
            setTimeout(function () { old.a.el.remove(); old.b.el.remove(); }, 400);
            portal = null;
            portalAt = clock + rand(45, 90);
        }
    }
    function castRod(w, a) {
        loadPool();
        w.el.classList.toggle('left', a.left);
        w.el.classList.add('fish');
        a.rod = document.createElement('div');
        a.rod.className = 'walker-rod' + (a.left ? ' flip' : '');
        a.rod.setAttribute('aria-hidden', 'true');
        a.rod.innerHTML = '<span class="walker-rod-line"></span><span class="walker-rod-bob"></span><span class="walker-rod-pole"></span>';
        a.rod.lastChild.style.backgroundImage = 'url(' + ICON + 'item/fishing_rod.png)';
        layer.appendChild(a.rod);
    }
    function placeRod(w, a) {
        var dir = a.left ? -1 : 1;
        var s = surfaceOf(w.s);
        var hx = w.x + dir * w.half * 0.45;
        var hy = w.y - 34;
        a.bx = hx + dir * 22;
        a.by = (s ? s.top : w.y) + 46;
        var tipY = hy - 22;
        var kids = a.rod.children;
        kids[0].style.cssText = 'left:' + Math.round(a.bx) + 'px;top:' + Math.round(tipY) + 'px;height:' + Math.round(a.by - tipY) + 'px';
        kids[1].style.left = Math.round(a.bx - 3) + 'px';
        kids[1].style.top = Math.round(a.by - 3) + 'px';
        kids[2].style.left = Math.round(dir > 0 ? hx - 2 : hx - 24) + 'px';
        kids[2].style.top = Math.round(hy - 24) + 'px';
    }
    function reel(w, a) {
        a.rod.remove();
        w.el.classList.remove('fish');
        var loot = rollLoot();
        var el = document.createElement('div');
        el.className = 'walker-catch';
        el.setAttribute('aria-hidden', 'true');
        el.innerHTML = '<span class="walker-catch-name"></span>';
        el.style.backgroundImage = 'url(' + ICON + loot.icon + '.png)';
        el.firstChild.textContent = loot.name;
        el.firstChild.style.color = loot.color;
        el.style.left = Math.round(a.bx - 14) + 'px';
        el.style.top = Math.round(a.by - 14) + 'px';
        layer.appendChild(el);
        requestAnimationFrame(function () {
            requestAnimationFrame(function () {
                el.style.left = Math.round(w.x - 14) + 'px';
                el.style.top = Math.round(w.y - HEIGHT - 38) + 'px';
            });
        });
        setTimeout(function () { el.classList.add('gone'); }, 2600);
        setTimeout(function () { el.remove(); }, 3000);
        burst(a.bx, a.by, 'splash', 5);
        burst(w.x, w.y - HEIGHT - 6, 'happy', loot.rare ? 8 : 3);
        w.plan.unshift({ type: 'idle', t: 2.8, cls: 'sit', sit: SIT_EDGE, left: a.left });
        if (Math.random() < 0.4) w.plan.splice(1, 0, { type: 'fish', left: a.left, t: 0, wait: rand(4, 10) });
    }
    function fan() {
        mouse.fan = true;
        var t = null;
        surfaces.forEach(function (o) {
            if (mouse.x > o.left + 10 && mouse.x < o.right - 10 && o.top >= mouse.y - 10 && o.top - mouse.y < 140 && (!t || o.top < t.top)) t = o;
        });
        if (!t) return;
        var w = all().filter(function (o) {
            var os = surfaceOf(o.s);
            return os && !os.walker && (o.s === t.el || reach(os, t)) && !o.el.hidden && !o.carry && calm(o.plan[0]) && Math.abs(o.x - mouse.x) < 500 && Math.abs(o.y - t.top) < 300;
        }).sort(function (a, b) { return Math.abs(a.x - mouse.x) + Math.abs(a.y - t.top) - Math.abs(b.x - mouse.x) - Math.abs(b.y - t.top); })[0];
        if (!w) return;
        var x = clamp(mouse.x + (w.x < mouse.x ? -1 : 1) * (w.half + 10), t.left + 4, t.right - 4);
        drop(w);
        if (w.s !== t.el) planMove(w, surfaceOf(w.s), t);
        w.plan.forEach(function (a) { a.fast = true; });
        w.plan.push({ type: 'walk', x: x, fast: true }, { type: 'idle', t: 30, cls: 'sit', sit: SIT_FLAT, left: mouse.x < x, watch: mouse.since });
    }
    function planWait(w) {
        var g = surfaceOf(ground);
        if (!g) return w.plan.push({ type: 'idle', t: 2 });
        var line = all().filter(function (o) { return o.waiting; }).map(function (o) { return o.name; }).sort();
        var x = clamp(g.left + 30 + line.indexOf(w.name) * 28, g.left + 4, g.right - 4);
        if (w.s !== ground) return w.plan.push({ type: 'idle', t: rand(0, 2.5) }, { type: 'rain', to: ground, x: x });
        if (Math.abs(w.x - x) > 4) w.plan.push({ type: 'walk', x: x, fast: true });
        w.plan.push({ type: 'idle', t: rand(20, 40), cls: 'doze', left: false });
    }
    function hello(w) {
        var m = mascot;
        if (!m || walkers[m.name] !== m || !calm(m.plan[0]) || m.s !== w.s) return;
        drop(m);
        m.plan.push({ type: 'idle', t: 1.6, cls: 'wave', left: w.x < m.x }, { type: 'idle', t: 0.3 });
        if (w.owner) {
            burst((w.x + m.x) / 2, m.y - HEIGHT - 6, 'heart', 5);
            bubble(m, 'https://www.brilliantw.net/img/' + encodeURI('關於飯娘/' + pick(MASCOT_EMOTES)));
        }
    }
    function plan(w, s) {
        var roll = Math.random();
        if (w.waiting) return planWait(w);
        if (w.me && w.follow && mouse && !s.walker) {
            w.plan.push({ type: 'walk', x: clamp(mouse.x, s.left + 4, s.right - 4), fast: Math.abs(mouse.x - w.x) > 160 }, { type: 'idle', t: 0.3 });
            return;
        }
        if (s.walker) {
            var dir = Math.random() < 0.5 ? -1 : 1;
            var b = below(w.x + dir * 30, s.top + 1);
            w.plan.push({ type: 'idle', t: rand(2, 6) });
            w.plan.push({ type: 'jump', to: b.el, x: clamp(w.x + dir * 30, b.left + 4, b.right - 4) });
            return;
        }
        var cake = cakeOn(s.el);
        if (cake) return goEat(w, cake);
        if (mascot && !w.npc && mascot.s === s.el && !mascot.el.hidden && clock > (w.mascotCd || 0) && Math.abs(mascot.x - w.x) < 350 && roll < 0.35) {
            w.mascotCd = clock + rand(45, 90);
            var side = w.x < mascot.x ? -1 : 1;
            w.plan.push({ type: 'walk', x: clamp(mascot.x + side * (mascot.half + w.half + 6), s.left + 4, s.right - 4), fast: true },
                { type: 'call', fn: function () { hello(w); } }, { type: 'idle', t: rand(1.2, 1.8), cls: 'wave', left: side > 0 }, { type: 'idle', t: 0.4 });
            return;
        }
        if (w.veteran && roll > 0.96) {
            w.plan.push({ type: 'idle', t: 1.6, cls: 'stretch', done: function () { burst(w.x + 8, w.y - 40, 'zzz', 1); } }, { type: 'idle', t: 0.6 });
            return;
        }
        var fun = Math.random();
        if (w.npc && fun > 0.88 && pets.length) {
            if (fun > 0.94) planCuddle(w);
            else planKiss(w);
            return;
        }
        if (w.owner && !mascotOnly && fun > 0.9 && (fun > 0.96 ? planTreat(w, s) : planVisit(w, s))) return;
        if (w.donor && !mascotOnly && fun > 0.94) {
            if (fun > 0.97) planCookies(w, s);
            else planFirework(w);
            return;
        }
        if (!mascotOnly && !w.npc) {
            loadDex();
            if (fun < 0.025 && planSnack(w)) return;
            if (fun < 0.05 && fun >= 0.025 && planGacha(w)) return;
            if (fun < 0.075 && fun >= 0.05 && planFurniture(w, s)) return;
            if (fun < 0.1 && fun >= 0.075 && clock > (w.cd || 0) && planTrade(w, s)) return;
            if (fun < 0.12 && fun >= 0.1 && planMine(w, s)) return;
        }
        if (night() && roll < 0.5) {
            w.plan.push({ type: 'idle', t: rand(20, 60), cls: 'sleep' });
            return;
        }
        if (portal && (s.el === portal.a.s || s.el === portal.b.s) && roll < 0.35 && planPortal(w, s)) return;
        if (roll < 0.7 && all().some(function (p) { return p.s === w.el; })) {
            w.plan.push({ type: 'idle', t: rand(1, 3) });
            return;
        }
        if (roll < 0.2 && !mascotOnly && planPick(w, s)) return;
        if (roll < 0.5 && planMove(w, s)) return;
        if (roll < 0.58 && planStack(w, s)) return;
        if (roll < 0.66 && planSnatch(w, s)) return;
        if (roll < 0.72 && planEdge(w, s, false)) return;
        if (roll < 0.78 && planEdge(w, s, true)) return;
        if (roll < 0.81) {
            w.plan.push({ type: 'idle', t: rand(1.5, 3), cls: 'spin' }, { type: 'idle', t: rand(0.5, 1.5) });
            return;
        }
        if (roll < 0.84) {
            w.plan.push({ type: 'walk', x: rand(s.left + 4, s.right - 4), crawl: true }, { type: 'idle', t: rand(1, 3) });
            return;
        }
        if (roll < 0.9 && planGuild(w, s)) return;
        if (roll > 0.92) {
            w.plan.push({ type: 'jump', to: s.el, x: w.x });
            w.plan.push({ type: 'jump', to: s.el, x: w.x });
        }
        w.plan.push({ type: 'walk', x: rand(s.left + 4, s.right - 4) });
        w.plan.push({ type: 'idle', t: rand(1, 5) });
    }

    function ghost(prop, r, x, y, small) {
        var clone = prop.cloneNode(true);
        clone.removeAttribute('id');
        clone.setAttribute('class', (clone.getAttribute('class') || '') + ' walker-prop');
        clone.setAttribute('aria-hidden', 'true');
        clone.style.cssText = 'position:absolute;margin:0;left:' + Math.round(x) + 'px;top:' + Math.round(y) + 'px;width:' + r.width + 'px;height:' + r.height + 'px;' +
            (small ? 'transform:scale(0.1);opacity:0;' : '');
        layer.appendChild(clone);
        return clone;
    }
    function lift(w, prop) {
        var r = relRect(prop);
        var clone = ghost(prop, r, r.left, r.top);
        prop.style.visibility = 'hidden';
        var box = document.createElement('span');
        box.className = 'walker-box';
        box.setAttribute('aria-hidden', 'true');
        var kind = pick(BOXES);
        box.style.backgroundImage = icon('custom/brilliant/movingbox/movingbox_' + kind + '_close');
        layer.appendChild(box);
        w.carry = { prop: prop, clone: box, width: 22, height: 22, kind: kind };
        w.el.classList.add('carry');
        place(w);
        requestAnimationFrame(function () {
            requestAnimationFrame(function () {
                clone.style.left = Math.round(w.x - r.width / 2) + 'px';
                clone.style.top = Math.round(w.y - HEIGHT - 25 - r.height / 2) + 'px';
                clone.style.transform = 'scale(0.1)';
                clone.style.opacity = '0';
            });
        });
        setTimeout(function () { clone.remove(); }, 450);
    }
    function putBack(w, now) {
        var c = w.carry;
        if (!c) return;
        w.carry = null;
        w.el.classList.remove('carry');
        var out = null;
        var done = function () {
            c.clone.remove();
            if (out) out.remove();
            c.prop.style.visibility = '';
            taken.splice(taken.indexOf(c.prop), 1);
        };
        if (now || !document.contains(c.prop)) return done();
        var r = relRect(c.prop);
        out = ghost(c.prop, r, parseFloat(c.clone.style.left) + 11 - r.width / 2, parseFloat(c.clone.style.top) + 11 - r.height / 2, true);
        if (c.kind) c.clone.style.backgroundImage = icon('custom/brilliant/movingbox/movingbox_' + c.kind + '_open');
        c.clone.classList.add('opening');
        requestAnimationFrame(function () {
            requestAnimationFrame(function () {
                out.style.left = r.left + 'px';
                out.style.top = r.top + 'px';
                out.style.transform = 'scale(1)';
                out.style.opacity = '1';
            });
        });
        setTimeout(done, 450);
    }

    function step(w, dt) {
        w.drop = 0;
        if (w.land > 0 && (w.land -= dt) <= 0) w.el.classList.remove('land');
        var s = surfaceOf(w.s);
        if (w.el.classList.contains('pressed')) {
            pose(w, 'idle');
            return place(w);
        }
        if (w.plan[0] && w.plan[0].type === 'held') {
            pose(w, 'air');
            return place(w);
        }
        if (s && s.walker && (w.x < s.left - 6 || w.x > s.right + 6)) s = null;
        if (!s && (!w.plan.length || ['walk', 'idle', 'look', 'pick', 'putback', 'eat', 'tug', 'follow'].indexOf(w.plan[0].type) >= 0)) {
            putBack(w);
            drop(w);
            w.s = null;
            w.plan.push({ type: 'fall', to: below(w.x, w.y).el });
        }
        if (s && !s.view && !w.carry && !w.waiting) {
            w.away = (w.away || 0) + dt;
            var near = surfaces.filter(function (t) { return t.view; });
            if (w.away > w.patience && near.length) {
                var t = pick(near);
                drop(w);
                w.s = null;
                w.x = rand(t.left + 4, t.right - 4);
                w.y = viewTop;
                w.plan.push({ type: 'fall', to: t.el });
                s = null;
            }
        } else w.away = 0;
        if (s && !s.walker && clock > (w.cd || 0) && (w.scan = (w.scan || 0) - dt) <= 0) {
            w.scan = 0.4;
            if (calm(w.plan[0]) && !w.carry) {
                for (var k in walkers) {
                    var o = walkers[k];
                    if (o !== w && o.s === w.s && !o.el.hidden && !o.carry && clock > (o.cd || 0) && calm(o.plan[0]) && Math.abs(o.x - w.x) < 30) {
                        greet(w, o);
                        break;
                    }
                }
            }
        }
        if (!w.plan.length) plan(w, s);
        var a = w.plan[0];
        aura(w, dt, a.type === 'walk');
        var target = a.to && surfaceOf(a.to);
        if (a.to && !target) { drop(w); return; }

        if (a.type === 'idle' || a.type === 'look') {
            pose(w, 'idle');
            w.drop = a.sit || 0;
            if (s) w.y = s.top + w.drop;
            if (a.watch !== undefined && (!mouse || mouse.since !== a.watch)) a.t = 0;
            if (a.type === 'idle' && !a.cls && Math.random() < dt * 0.015) bubble(w);
            if (mouse && a.type === 'idle' && !a.cls && Math.abs(mouse.x - w.x) < 160 && Math.abs(mouse.y - w.y + 32) < 160) w.el.classList.toggle('left', mouse.x < w.x);
            if ((a.cls === 'sleep' || a.cls === 'doze') && (a.z = (a.z || 0) + dt) > 1.4) {
                a.z = 0;
                burst(w.x + (w.el.classList.contains('left') ? 14 : -14), w.y - 24, 'zzz', 1);
            }
            if (!a.on) {
                a.on = true;
                if (a.cls) w.el.classList.add(a.cls);
                if (a.cls === 'wish') {
                    var held = document.createElement('span');
                    held.className = 'walker-held';
                    held.style.backgroundImage = icon('custom/brilliant/currency/watercress');
                    w.el.appendChild(held);
                }
                if (a.left !== undefined) w.el.classList.toggle('left', a.left);
            }
            if (a.type === 'look' && Math.random() < dt * 0.8) w.el.classList.toggle('left');
            if (a.cls === 'munch' && (a.chew = (a.chew || 0) + dt) >= 0.4) {
                a.chew = 0;
                bite(w, a.img);
            }
            if (a.mine) mineTick(a);
            if ((a.t -= dt) <= 0) {
                w.plan.shift();
                if (a.cls) w.el.classList.remove(a.cls);
                if (a.done) a.done();
            }
        } else if (a.type === 'rain') {
            w.s = null;
            w.x = a.x !== undefined ? a.x : rand(target.left + 4, target.right - 4);
            w.y = Math.min(viewTop, target.top - 120);
            burst(w.x, w.y - 30, 'portal', 6);
            w.plan[0] = { type: 'fall', to: a.to };
        } else if (a.type === 'call') {
            w.plan.shift();
            a.fn();
        } else if (a.type === 'follow') {
            var lead = a.who;
            if (walkers[lead.name] !== lead || lead.s !== w.s || (a.t -= dt) <= 0) {
                w.plan.shift();
                return place(w);
            }
            if (s) w.y = s.top;
            var goal = lead.x + (lead.el.classList.contains('left') ? 1 : -1) * (lead.half + w.half + 4);
            var off = goal - w.x;
            if (Math.abs(off) < 6) {
                pose(w, 'idle');
                w.el.classList.toggle('left', lead.x < w.x);
            } else {
                w.x += (off < 0 ? -1 : 1) * Math.min(Math.abs(off), w.speed * 1.4 * dt);
                w.el.classList.toggle('left', off < 0);
                pose(w, 'walk');
            }
        } else if (a.type === 'walk') {
            var gap = a.x - w.x;
            var speed = w.speed * (a.fast ? 2.5 : a.crawl ? 0.35 : 1);
            if (s) w.y = s.top;
            if (Math.abs(gap) <= speed * dt) {
                w.x = a.x;
                w.plan.shift();
                pose(w, 'idle');
            } else {
                w.x += (gap < 0 ? -1 : 1) * speed * dt;
                w.el.classList.toggle('left', gap < 0);
                pose(w, a.crawl ? 'crawl' : 'walk');
            }
        } else if (a.type === 'climb') {
            w.el.classList.toggle('left', a.face);
            pose(w, 'climb');
            w.y -= 40 * dt;
            if (w.y <= target.top) { w.y = target.top; land(w, a.to); }
        } else if (a.type === 'fall') {
            pose(w, 'air');
            a.vy = (a.vy || 0) + 900 * dt;
            w.y += a.vy * dt;
            if (w.y >= target.top) { w.y = target.top; land(w, a.to); }
        } else if (a.type === 'jump') {
            if (a.wind === undefined) {
                if (a.ride) a.x = a.ride.x;
                a.wind = 0.15;
                a.t = 0;
                a.x0 = w.x;
                a.y0 = w.y;
                a.dur = 0.45 + Math.abs(a.x - w.x) / 400;
                a.h = Math.max(20, (a.y0 - target.top) / 2 + 24);
                if (a.x !== w.x) w.el.classList.toggle('left', a.x < w.x);
            }
            if (a.wind > 0) {
                a.wind -= dt;
                pose(w, 'crouch');
                return place(w);
            }
            pose(w, 'air');
            a.t = Math.min(1, a.t + dt / a.dur);
            w.x = a.x0 + (a.x - a.x0) * a.t;
            w.y = a.y0 + (target.top - a.y0) * a.t - a.h * 4 * a.t * (1 - a.t);
            if (a.t >= 1) {
                w.y = target.top;
                land(w, a.to);
                if (a.after && a.after.length) {
                    w.el.classList.toggle('left', a.x0 < w.x);
                    w.plan.unshift({ type: 'idle', t: 0.22 * a.after.length + 0.3 });
                    crumble(a.after);
                }
            }
        } else if (a.type === 'pillar') {
            if (!a.blocks) {
                a.blocks = [];
                a.base = w.y;
                a.t = 0;
            }
            var k = a.blocks.length;
            if (a.base - k * BLOCK - target.top <= JUMP_UP || k >= 6) {
                w.plan[0] = { type: 'jump', to: a.to, x: a.x, after: a.blocks };
                return place(w);
            }
            a.t += dt;
            var q = Math.min(1, a.t / 0.38);
            pose(w, q < 1 ? 'air' : 'crouch');
            w.y = a.base - k * BLOCK - BLOCK * q - 8 * Math.sin(Math.PI * q);
            if (q >= 0.7 && !a.placed) {
                a.placed = true;
                var b = document.createElement('span');
                b.className = 'walker-block';
                b.setAttribute('aria-hidden', 'true');
                b.style.left = Math.round(w.x - BLOCK / 2) + 'px';
                b.style.top = Math.round(a.base - (k + 1) * BLOCK) + 'px';
                layer.appendChild(b);
                a.blocks.push(b);
            }
            if (q >= 1) {
                a.t = 0;
                a.placed = false;
            }
        } else if (a.type === 'fish') {
            pose(w, 'idle');
            w.drop = SIT_EDGE;
            if (s) w.y = s.top + SIT_EDGE;
            if (!a.rod) castRod(w, a);
            placeRod(w, a);
            a.t += dt;
            if (!a.bite && a.t > a.wait) {
                a.bite = true;
                a.rod.classList.add('bite');
                burst(a.bx, a.by, 'splash', 3);
            }
            if (a.bite && a.t > a.wait + 0.9) {
                w.plan.shift();
                reel(w, a);
            }
        } else if (a.type === 'warp') {
            pose(w, 'idle');
            if (!a.t) {
                w.el.classList.add('warp-in');
                burst(w.x, w.y - 30, 'portal', 8);
            }
            a.t += dt;
            if (!a.out && a.t >= 0.45) {
                var to = portal && spot(a.other);
                if (!to) {
                    drop(w);
                    return place(w);
                }
                a.out = to;
                w.x = to.x;
                w.y = to.y;
                w.s = to.s;
                w.el.classList.remove('warp-in');
                w.el.classList.add('warp-out');
                burst(w.x, w.y - 30, 'portal', 8);
            }
            if (a.t >= 0.9) {
                w.el.classList.remove('warp-out');
                w.plan.shift();
                w.plan.unshift({ type: 'walk', x: a.out.x - a.other.side * 44 });
            }
        } else if (a.type === 'fly') {
            pose(w, 'air');
            var y0 = w.y;
            a.vy += 900 * dt;
            a.spin = (a.spin || 0) + a.vx * dt * 0.4;
            w.body.style.rotate = Math.round(a.spin) + 'deg';
            w.x += a.vx * dt;
            w.y += a.vy * dt;
            if (Math.hypot(a.vx, a.vy) > 300) knock(w, a);
            if (a.vy > 0) {
                var hit = null;
                surfaces.forEach(function (t) {
                    if (w.x >= t.left && w.x <= t.right && y0 <= t.top && w.y >= t.top && (!hit || t.top < hit.top)) hit = t;
                });
                var floor = surfaceOf(ground);
                if (!hit && floor && w.y >= floor.top) {
                    hit = floor;
                    w.x = clamp(w.x, floor.left + 4, floor.right - 4);
                }
                if (hit) {
                    var impact = Math.hypot(a.vx, a.vy);
                    w.y = hit.top;
                    land(w, hit.el);
                    if (impact > 800 && Math.random() < 0.4) totem(w);
                    else if (impact > 800) w.plan.unshift({ type: 'idle', t: 2.5, cls: 'dizzy' });
                }
            }
        } else if (a.type === 'pick') {
            var close = s && document.contains(a.prop) && relRect(a.prop);
            w.plan.shift();
            if (close && beside(s, close)) {
                w.el.classList.toggle('left', close.left + close.width / 2 < w.x);
                if (a.mine) startMine(w, a.prop);
                else lift(w, a.prop);
            } else {
                taken.splice(taken.indexOf(a.prop), 1);
                drop(w);
            }
        } else if (a.type === 'putback') {
            putBack(w);
            w.plan.shift();
        } else if (a.type === 'eat') {
            var c = a.cake;
            pose(w, 'idle');
            if (s) w.y = s.top;
            w.plan.shift();
            if (cakes.indexOf(c) < 0 || c.eater || Math.abs(c.x - w.x) > w.half + 24) burst(w.x, w.y - HEIGHT, 'angry', 3);
            else if (c.coin) {
                removeCake(c);
                gain(w, COINS[c.coin][1]);
                burst(c.x, c.y - 8, 'happy', 4);
                w.plan.unshift({ type: 'jump', to: w.s, x: w.x });
            } else {
                c.eater = w;
                c.eatAt = clock;
                c.el.classList.add('eaten');
                w.el.classList.toggle('left', c.x < w.x);
                w.plan.unshift({
                    type: 'idle', t: 1.6, cls: 'munch', done: function () {
                        removeCake(c);
                        bite(w, c.food && icon(c.food));
                        burst(w.x, w.y - HEIGHT - 6, 'heart', 2);
                        w.plan.unshift({ type: 'jump', to: w.s, x: w.x });
                    }
                });
                bite(w, c.food && icon(c.food));
            }
        } else if (a.type === 'tug') {
            var other = a.other;
            w.plan.shift();
            if (walkers[other.name] === other && other.carry && other.s === w.s && other.plan[0] && other.plan[0].type === 'look' && Math.abs(other.x - w.x) < other.half + w.half + 12) {
                w.el.classList.toggle('left', other.x < w.x);
                other.el.classList.toggle('left', w.x < other.x);
                w.plan.unshift({ type: 'idle', t: 1.3, cls: 'tug', done: function () { burst(w.x, w.y - HEIGHT, 'angry', 4); } });
                other.plan = [{
                    type: 'idle', t: 1.3, cls: 'tug', done: function () {
                        if (other.carry) other.carry.clone.classList.add('flung');
                        putBack(other);
                    }
                }, { type: 'idle', t: 0.6 }];
            }
        }
        var floor = surfaceOf(ground);
        if (floor && w.y > floor.top + w.drop) w.y = floor.top + w.drop;
        if (!floor && w.y > layer.offsetHeight + 300 && surfaces.length) {
            drop(w);
            w.plan.push({ type: 'rain', to: pick(seen(surfaces)).el });
        }
        place(w);
    }

    function frame(now) {
        requestAnimationFrame(frame);
        var dt = Math.min(0.1, (now - (last || now)) / 1000);
        last = now;
        clock = now / 1000;
        if ((spawnT += dt) >= 0.15) {
            spawnT = 0;
            spawn();
        }
        if (!disabled && !mascot && surfaces.length && !document.documentElement.classList.contains('loading') && !sceneShown()) spawnMascot();
        stepExtras(dt);
        cakes.slice().forEach(function (c) { stepCake(c, dt); });
        stepQueue();
        if (!mascotOnly && !disabled) {
            stepCoins();
            stepPortal();
            stepGame();
            stepMeteor(dt);
            stepMusic(dt);
        }
        if (mouse && !mouse.fan && clock - mouse.since > 3) fan();
        Object.keys(walkers).forEach(function (name) { if (walkers[name] && !walkers[name].el.hidden) step(walkers[name], dt); });
    }

    try { disabled = localStorage.getItem('walkers') === 'off'; } catch (e) { }
    syncButton();
    measure();
    sync(JSON.parse(ground.getAttribute('data-players') || '[]'), JSON.parse(ground.getAttribute('data-guilds') || '{}'), JSON.parse(ground.getAttribute('data-info') || '{}'));
    window.syncWalkers = sync;
    window.walkerMe().then(function (r) { return r.ok ? r.json() : {}; }).then(function (d) {
        if (!d || !d.name) return;
        meName = d.name;
        adopt();
    }).catch(function () { });
    if (still) {
        while (queue.length) add(queue.shift());
        return;
    }
    document.addEventListener('click', function (e) {
        if (e.button || !e.target.closest) return;
        if (e.target.closest('.ip-copy-btn')) return cheer();
        if (mascotOnly) return;
        if (!layer.parentElement.contains(e.target) || e.target.closest('a, button, input, textarea, select, summary, label, img, video, audio, iframe, svg, [onclick], [role="button"], .walker')) return;
        if (String(window.getSelection ? getSelection() : '')) return;
        var p = pointer(e);
        dropCake(p.x, p.y);
    });
    document.addEventListener('pointermove', function (e) {
        if (e.pointerType !== 'mouse') return;
        var p = pointer(e);
        mouse = { x: p.x, y: p.y, since: clock };
    }, { passive: true });
    document.documentElement.addEventListener('mouseleave', function () { mouse = null; });
    setInterval(measure, 1000);
    window.addEventListener('resize', measure);
    requestAnimationFrame(frame);
})();
