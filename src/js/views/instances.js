import { api, FILE_TYPES, fileSrc, isType, openUrl, pickFile, pickFiles, watchFolders } from '../api.js';
import { $, $$, acceptFiles, clear, confirm, dialog, fail, fmtBytes, fmtDuration, h, rangeFill, toast, guard, RECYCLE_NOTE } from '../ui.js';
import { accountFor, emit, isRunning, loadInstances, on, saveSettings, selectedInstance, store } from '../store.js';
import { icon, iconOf, INSTANCE_ICONS, instanceIconName, instanceVisual, svgFromFile } from '../icons.js';

const isSvg = (path) => /\.svg$/i.test(path || '');
import { importPicture } from '../crop.js';
import { importPack, openStore } from './mods.js';
import { isPreparing, repairInstance } from './home.js';
import { jvmPresetButtons, SHARE_INFO, SHARE_LABELS } from './settings.js';
import { perfCheck } from '../perf.js';

export async function versionData() {
    if (!store.versions) store.versions = await api.list_versions();
    return store.versions;
}

function segmented(options, value) {
    const box = h('div', { class: 'segmented', role: 'radiogroup' });
    box.value = value;
    const buttons = options.map(([v, label]) =>
        h('button', {
            type: 'button',
            role: 'radio',
            'aria-selected': String(v === value),
            'aria-checked': String(v === value),
            onclick: () => {
                box.value = v;
                buttons.forEach((b, i) => {
                    b.setAttribute('aria-selected', String(options[i][0] === v));
                    b.setAttribute('aria-checked', String(options[i][0] === v));
                });
                box.dispatchEvent(new Event('change'));
            }
        }, label)
    );
    box.append(...buttons);
    return box;
}

async function editorDialog(title, source) {
    const data = await versionData();
    const modSupported = new Set(data.modVersions);

    const name = h('input', { type: 'text', value: source.name ?? '', maxlength: '40' });
    const group = groupInput(source.group);
    const iconPick = iconChooser(source.icon?.startsWith('img:') ? source.icon : instanceIconName(source.icon), INSTANCE_ICONS);
    const iconPicker = iconPick.el;
    let pickedImage;
    const preview = h('div', { class: 'image-preview' });
    const imageClear = h('button', { type: 'button', class: 'mc', onclick: () => { pickedImage = ''; renderPreview(); } }, '移除圖片');
    const renderPreview = () => {
        const current = pickedImage === undefined ? source.image : pickedImage;
        const vector = isSvg(current);
        preview.style.backgroundImage = current && !vector ? `url("${fileSrc(current)}")` : '';
        preview.replaceChildren(...(vector ? [svgFromFile(current, 'cover-svg')] : []));
        preview.classList.toggle('empty', !current);
        imageClear.disabled = !current;
    };
    const chooseCover = async (path) => {
        if (!path) return;
        const saved = await importPicture(path, { width: 1200, height: 300, title: '調整封面', hint: '封面會以 4:1 顯示在實例卡片上，建議 1200×300。', vectorSvg: true }).catch(fail);
        if (saved) { pickedImage = saved; renderPreview(); }
    };
    const imageField = h('div', { class: 'field' }, h('label', {}, `封面圖片（選填，會取代圖示；建議 1200×300，比例 4:1；${FILE_TYPES.cover.extensions.map((e) => e.toUpperCase()).join('、')}，可直接拖進來）`),
        acceptFiles(h('div', { class: 'image-pick' }, preview,
            h('div', { class: 'toolbar' },
                h('button', { type: 'button', class: 'mc', onclick: async () => chooseCover(await pickFile('cover').catch(fail)) }, icon('image'), '選擇圖片'),
                imageClear)), 'cover', chooseCover));
    renderPreview();
    const wanted = source.mcVersion || (source.general ? data.latestRelease : data.recommended) || '';
    let showAll = !!source.general || !modSupported.size || (wanted !== '' && !modSupported.has(wanted));
    const kind = segmented([['brilliant', '輝煌實例'], ['general', '一般實例']], source.general ? 'general' : 'brilliant');
    const kindHint = h('div', { class: 'hint' });
    const mcVersion = h('select', {});
    const renderVersions = () => {
        const current = mcVersion.value || wanted;
        clear(mcVersion).append(
            ...data.versions
                .filter((v) => showAll || modSupported.has(v.id))
                .map((v) =>
                    h(
                        'option',
                        { value: v.id, selected: v.id === current },
                        `${v.id}${v.type === 'release' ? '' : `（${v.type}）`}${v.id === data.recommended ? (kind.value === 'general' ? '（輝煌推薦）' : '（推薦）') : showAll && modSupported.has(v.id) ? '（輝煌支援）' : ''}`
                    )
                )
        );
    };
    renderVersions();
    const showAllBox = h('label', { style: 'margin-top:8px' },
        h('input', { type: 'checkbox', checked: showAll, onchange: (e) => { showAll = e.target.checked; renderVersions(); refreshLoaders(); } }),
        '顯示所有 Minecraft 版本（輝煌不支援的版本無法連上伺服器）');
    const loader = segmented([['fabric', 'Fabric'], ['vanilla', '原版']], source.loader === 'fabric' ? 'fabric' : 'vanilla');
    const loaderVersion = h('select', {}, h('option', { value: '' }, '最新穩定版'),
        source.loaderVersion ? h('option', { value: source.loaderVersion, selected: true }, source.loaderVersion) : '');
    let loaderSeq = 0;
    const memoryLabel = h('b', {});
    const memory = h('input', { type: 'range', min: '0', max: '32768', step: '512', value: String(source.memoryMb ?? 0) });
    const syncMemory = () => {
        memoryLabel.textContent = Number(memory.value) ? `${memory.value} MB` : `沿用全域設定（${store.settings?.memoryMb ?? 4096} MB）`;
        rangeFill(memory);
    };
    memory.addEventListener('input', syncMemory);
    syncMemory();
    const rpc = segmented([['', '沿用全域'], ['on', '啟用'], ['off', '停用']],
        source.injectRichPresence === true ? 'on' : source.injectRichPresence === false ? 'off' : '');

    const loaderField = h('div', { class: 'field' }, h('label', {}, 'Fabric Loader 版本'), loaderVersion);
    const account = h('select', {}, h('option', { value: '' }, '跟隨目前選擇的帳號'),
        ...store.accounts.map((a) => h('option', { value: a.id, selected: a.id === source.account }, `${a.name}${a.kind === 'msa' ? '' : '（離線）'}`)));
    const accountField = h('div', { class: 'field' }, h('label', {}, '使用帳號'), account,
        h('div', { class: 'hint' }, '指定後，這個實例一律用該帳號啟動，不受上方切換的帳號影響，適合同時開兩個帳號遊玩。'));
    const rpcField = h('div', { class: 'field' }, h('label', {}, '自動安裝 Brilliant-Server-Mod'), rpc,
        h('div', { class: 'hint' }, '啟用時，每次啟動都會自動安裝最新的 Brilliant-Server-Mod 與 Fabric API。'));

    async function refreshLoaders() {
        const isFabric = loader.value === 'fabric';
        const general = kind.value === 'general';
        loaderField.style.display = isFabric ? '' : 'none';
        rpcField.style.display = isFabric && !general ? '' : 'none';
        showAllBox.style.display = general ? 'none' : '';
        kindHint.textContent = general
            ? '給合作伺服器或單人遊戲用：任何版本都能選，不會自動安裝輝煌模組、不會預載伺服器資源包，也不會跳出輝煌的升版提醒。'
            : '玩輝煌用：自動安裝 Brilliant-Server-Mod、預載伺服器資源包，伺服器升版時會提醒你一鍵跟上。';
        if (!isFabric) return;
        const seq = ++loaderSeq;
        const keep = loaderVersion.value;
        loaderVersion.disabled = true;
        try {
            const list = await api.list_fabric_loaders({ gameVersion: mcVersion.value });
            if (seq !== loaderSeq) return;
            clear(loaderVersion).append(
                h('option', { value: '' }, '最新穩定版'),
                ...list.map((l) =>
                    h('option', { value: l.version, selected: l.version === keep },
                        `${l.version}${l.stable ? '' : '（測試版）'}`)
                )
            );
        } catch (error) {
            if (seq === loaderSeq) fail(error);
        } finally {
            if (seq === loaderSeq) loaderVersion.disabled = false;
        }
    }
    loader.addEventListener('change', refreshLoaders);
    kind.addEventListener('change', () => {
        showAll = kind.value === 'general' || !modSupported.size;
        showAllBox.querySelector('input').checked = showAll;
        renderVersions();
        refreshLoaders();
    });
    mcVersion.addEventListener('change', () => {
        loaderVersion.value = '';
        refreshLoaders();
    });

    const javaPath = h('select', {}, h('option', { value: '' }, '沿用全域設定'),
        source.javaPath ? h('option', { value: source.javaPath, selected: true }, source.javaPath) : '');
    api.list_java().then((list) => {
        const keep = javaPath.value;
        clear(javaPath).append(h('option', { value: '' }, '沿用全域設定'),
            ...list.map((j) => h('option', { value: j.path, selected: j.path === keep }, `Java ${j.version} — ${j.path}`)));
        if (keep && !list.some((j) => j.path === keep)) javaPath.append(h('option', { value: keep, selected: true }, keep));
    }).catch(() => {});
    const jvmArgs = h('textarea', { spellcheck: 'false', placeholder: '留空沿用全域設定' }, source.jvmArgs ?? '');
    const width = h('input', { type: 'number', min: '0', max: '7680', value: source.width || '', placeholder: '沿用全域' });
    const height = h('input', { type: 'number', min: '0', max: '4320', value: source.height || '', placeholder: '沿用全域' });
    const fullscreen = h('input', { type: 'checkbox', checked: !!source.fullscreen });
    const javaMajor = h('select', {},
        ...[[0, '自動（依 Minecraft 版本）'], [8, 'Java 8'], [17, 'Java 17'], [21, 'Java 21'], [25, 'Java 25']].map(([value, label]) =>
            h('option', { value: String(value), selected: Number(source.javaMajor || 0) === value }, label)));
    const wrapper = h('input', { type: 'text', spellcheck: 'false', value: source.wrapper ?? '', placeholder: '留空沿用全域設定' });
    const envVars = h('textarea', { spellcheck: 'false', placeholder: '每行一個 KEY=VALUE，會覆蓋全域的同名變數' }, source.envVars ?? '');
    const modChannel = h('select', {},
        ...[['', '沿用全域設定'], ['release', '只用正式版'], ['beta', '允許 Beta 版'], ['alpha', '允許所有版本']].map(([value, label]) =>
            h('option', { value, selected: (source.modChannel ?? '') === value }, label)));
    const freezeMods = h('input', { type: 'checkbox', checked: !!source.freezeMods });
    const advanced = h('details', { class: 'advanced', open: !!(source.javaPath || source.jvmArgs || source.width || source.javaMajor || source.wrapper || source.envVars || source.modChannel || source.freezeMods) },
        h('summary', {}, '進階設定（覆寫全域）'),
        h('div', { class: 'field-row' },
            h('div', { class: 'field' }, h('label', {}, 'Java 版本'), javaMajor),
            h('div', { class: 'field' }, h('label', {}, 'Java 執行檔'), javaPath)),
        h('div', { class: 'hint' }, '指定版本時，啟動器會自動下載並管理對應的 Java；選了「Java 執行檔」則以執行檔為準。'),
        h('div', { class: 'field' }, h('label', {}, 'JVM 參數'), jvmArgs, h('div', { class: 'jvm-presets' }, ...jvmPresetButtons(jvmArgs, () => {}))),
        h('div', { class: 'field' }, h('label', {}, '啟動包裝指令'), wrapper),
        h('div', { class: 'field' }, h('label', {}, '環境變數'), envVars),
        h('div', { class: 'field-row' },
            h('div', { class: 'field' }, h('label', {}, '模組更新管道'), modChannel),
            h('div', { class: 'field' }, h('label', {}, freezeMods, '凍結模組'), h('div', { class: 'hint' }, '不檢查也不自動更新這個實例的模組，適合穩定的整合包。'))),
        h('div', { class: 'field-row' },
            h('div', { class: 'field' }, h('label', {}, '視窗寬度'), width),
            h('div', { class: 'field' }, h('label', {}, '視窗高度'), height)),
        h('label', {}, fullscreen, '全螢幕啟動（需同時填寫寬高才會覆寫）'));

    const content = h(
        'div',
        { class: 'inst-editor' },
        h('div', { class: 'field' }, h('label', {}, '類型'), kind, kindHint),
        h('div', { class: 'field' }, h('label', {}, '名稱'), name),
        h('div', { class: 'field' }, h('label', {}, '分組'), group, h('div', { class: 'hint' }, '同一組的實例會放在一起，可一鍵全部啟動，也能在分組設定裡共用存檔、資源包、光影或截圖。留空表示不分組。')),
        h('div', { class: 'field' }, h('label', {}, '圖示'), iconPicker),
        imageField,
        h('div', { class: 'field' }, h('label', {}, 'Minecraft 版本'), mcVersion, modSupported.size ? showAllBox : ''),
        h('div', { class: 'field' }, h('label', {}, '模組載入器'), loader),
        loaderField,
        h('div', { class: 'field' }, h('label', {}, '記憶體上限：', memoryLabel), memory),
        accountField,
        rpcField,
        advanced
    );

    refreshLoaders();

    const ok = await dialog({
        title,
        content,
        buttons: [
            { label: '取消', value: false },
            { label: '確定', value: true }
        ]
    });
    if (ok !== true) return null;
    if (!name.value.trim()) {
        toast('請輸入實例名稱', 'error');
        return null;
    }
    if (nameTaken(name.value, source.id)) {
        toast(`已經有叫「${name.value.trim()}」的實例了，請換一個名稱`, 'error');
        return null;
    }
    return {
        ...source,
        name: name.value.trim(),
        icon: iconPick.value,
        mcVersion: mcVersion.value,
        loader: loader.value,
        loaderVersion: loader.value === 'fabric' ? loaderVersion.value : '',
        memoryMb: Math.max(0, Number(memory.value) || 0),
        injectRichPresence: rpc.value === '' ? null : rpc.value === 'on',
        general: kind.value === 'general',
        javaPath: javaPath.value,
        jvmArgs: jvmArgs.value.trim(),
        width: Math.max(0, Number(width.value) || 0),
        height: Math.max(0, Number(height.value) || 0),
        fullscreen: fullscreen.checked,
        javaMajor: Number(javaMajor.value) || 0,
        wrapper: wrapper.value.trim(),
        envVars: envVars.value.trim(),
        modChannel: modChannel.value,
        freezeMods: freezeMods.checked,
        account: account.value,
        group: group.input.value.trim().slice(0, 24),
        pickedImage
    };
}

const BLANK = {
    id: '', name: '', icon: 'zap', mcVersion: '', loader: 'vanilla', loaderVersion: '',
    versionId: '', memoryMb: 0, jvmArgs: '', width: 0, height: 0, fullscreen: false,
    injectRichPresence: null, general: false, join: '', createdAt: 0, lastPlayed: 0, playSeconds: 0,
    javaMajor: 0, wrapper: '', envVars: '', useShared: false, modChannel: '', freezeMods: false
};

const nameTaken = (name, exceptId = '') => store.instances.some((i) => i.id !== exceptId && i.name.trim() === name.trim());

async function applyImage(id, pickedImage) {
    if (pickedImage === undefined) return;
    await api.set_instance_image({ id, path: pickedImage }).catch(fail);
}

async function createFromDraft({ pickedImage, ...draft }) {
    toast('正在準備版本資訊⋯⋯');
    const created = await api.create_instance({ draft });
    await applyImage(created.id, pickedImage);
    await api.select_instance({ id: created.id });
    store.settings.selectedInstance = created.id;
    await loadInstances();
    emit('settings');
    toast(`已建立「${created.name}」`);
    return created;
}

export async function quickCreate() {
    const data = await versionData();
    const version = data.recommended || data.modVersions[0];
    if (!version) throw new Error('暫時連不上官網，無法取得輝煌支援的版本，請稍後再試，或到「實例」頁手動建立。');
    return createFromDraft({ ...BLANK, name: `輝煌 ${version}`, icon: 'zap', mcVersion: version, loader: 'fabric' });
}

async function createInstance(general = false) {
    const draft = await editorDialog('建立實例', { ...BLANK, loader: 'fabric', general });
    if (!draft) return;
    await createFromDraft(draft).catch(fail);
}

export const createGeneralInstance = () => createInstance(true);

export async function editInstance(instance) {
    const result = await editorDialog('編輯實例', { ...instance });
    if (!result) return;
    const { pickedImage, ...draft } = result;
    try {
        await api.update_instance({ updated: draft });
        await applyImage(draft.id, pickedImage);
        await loadInstances();
        toast('已儲存');
    } catch (error) {
        fail(error);
    }
}

async function duplicateInstance(instance) {
    const input = h('input', { type: 'text', value: `${instance.name} 的複本`, maxlength: '40' });
    const ok = await dialog({
        title: '複製實例',
        content: h('div', { class: 'field' },
            h('label', {}, '新實例名稱'), input,
            h('div', { class: 'hint' }, '存檔、設定與模組都會一併複製。')),
        buttons: [{ label: '取消', value: false }, { label: '複製', value: true }]
    });
    if (ok !== true) return;
    const name = input.value.trim();
    if (!name) return toast('請輸入新實例的名稱', 'warn');
    if (nameTaken(name)) return toast(`已經有叫「${name}」的實例了，複本請取一個不同的名稱`, 'warn');
    toast(`正在複製「${instance.name}」，檔案多時需要一些時間⋯`);
    try {
        await api.duplicate_instance({ id: instance.id, name });
        await loadInstances();
        toast('已複製');
    } catch (error) {
        fail(error);
    }
}

async function removeInstance(instance) {
    if (!(await confirm(`確定要刪除「${instance.name}」嗎？\n整個實例（存檔、模組、設定）${RECYCLE_NOTE}`, '刪除'))) return;
    try {
        await api.delete_instance({ id: instance.id });
        await loadInstances();
        toast('已刪除');
    } catch (error) {
        fail(error);
    }
}

async function openFolder(instance, sub = '') {
    try {
        const path = await api.open_instance_folder({ id: instance.id, sub });
        await api.open_path({ path });
    } catch (error) {
        fail(error);
    }
}

export async function selectInstance(instance) {
    if (selectedInstance()?.id === instance.id) return;
    await api.select_instance({ id: instance.id });
    store.settings.selectedInstance = instance.id;
    emit('instances');
    emit('settings');
}

const PLAN_LABEL = {
    managed: ['自動處理', ''],
    update: ['會更新', 'accent'],
    keep: ['可直接使用', ''],
    disable: ['新版尚未支援，先停用', 'warn'],
    unknown: ['無法確認，保留不動', 'warn']
};

function planOf(entry) {
    if (entry.managed) return 'managed';
    if (!entry.enabled) return 'keep';
    if (entry.update) return 'update';
    if (entry.compatible === true) return 'keep';
    if (entry.compatible === false) return 'disable';
    return 'unknown';
}

export async function upgradeFlow(instance, target) {
    const list = h('div', { class: 'rows upgrade-list' }, h('div', { class: 'empty-state' }, '正在比對模組相容性⋯'));
    const mode = segmented([['inplace', '直接升級（先自動備份存檔）'], ['copy', '複製一份再升級']], 'inplace');
    let plan = [];
    api.content_list({ id: instance.id, kind: 'mod', mc: target }).then((entries) => {
        plan = entries.map((entry) => ({ entry, action: planOf(entry) }));
        const count = (a) => plan.filter((p) => p.action === a).length;
        const summary = [['update', '個會更新'], ['keep', '個可直接使用'], ['disable', '個新版尚未支援（先停用）'], ['unknown', '個無法確認']]
            .filter(([a]) => count(a)).map(([a, label]) => `${count(a)} ${label}`).join('、');
        clear(list).append(...(summary ? [h('div', { class: 'hint' }, summary)] : []), ...(plan.length ? plan.map(({ entry, action }) => h('div', { class: 'row' },
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, entry.name),
                h('div', { class: 'meta' }, action === 'update' ? `${entry.version ?? ''} → ${entry.update.version}` : entry.file)),
            h('span', { class: `tag ${PLAN_LABEL[action][1]}` }, PLAN_LABEL[action][0])))
            : [h('div', { class: 'empty-state' }, '這個實例沒有安裝其他模組。')]));
    }).catch((error) => {
        clear(list).append(h('div', { class: 'empty-state' }, '暫時無法連上 Modrinth，模組會保持原樣，升級後可到「模組與資源」檢查更新。'));
        fail(error);
    });
    const go = await dialog({
        title: `升級到 Minecraft ${target}`,
        wide: true,
        content: h('div', {},
            h('p', { class: 'hint' }, instance.loader === 'fabric'
                ? `「${instance.name}」目前是 ${instance.mcVersion}。升級後 Fabric Loader 會換成最新穩定版，Brilliant-Server-Mod 與 Fabric API 會在下次啟動自動換成對應版本。`
                : `「${instance.name}」目前是 ${instance.mcVersion}，升級後下次開始遊戲會自動下載新版本。`),
            !instance.general && store.versions?.modVersions?.length && !store.versions.modVersions.includes(target)
                ? h('p', { class: 'hint warn' }, `輝煌伺服器目前還不支援 ${target}，升級後可能無法連線輝煌。`)
                : '',
            mode,
            list),
        buttons: [{ label: '取消', value: false }, { label: '開始升級', value: true }]
    });
    if (go !== true) return;
    try {
        let targetInstance = instance;
        if (mode.value === 'copy') {
            toast('正在複製實例⋯');
            const base = `${instance.name}（${target}）`;
            const taken = new Set(store.instances.map((i) => i.name.trim()));
            let copyName = base;
            for (let n = 2; taken.has(copyName); n++) copyName = `${base} (${n})`;
            targetInstance = await api.duplicate_instance({ id: instance.id, name: copyName });
        } else {
            await api.backup_now({ id: instance.id }).catch(() => {});
        }
        toast(`正在升級到 ${target}⋯`);
        await api.update_instance({ updated: { ...targetInstance, mcVersion: target, loaderVersion: '' } });
        for (const { entry, action } of plan) {
            if (action === 'update') await api.content_update({ id: targetInstance.id, kind: 'mod', file: entry.file, update: entry.update }).catch(fail);
            if (action === 'disable') await api.set_mod_enabled({ id: targetInstance.id, file: entry.file, enabled: false }).catch(fail);
        }
        const dropped = new Set(plan.filter((p) => p.action === 'disable').map(({ entry }) => entry.project_id || entry.file.replace(/\.disabled$/, '')));
        if (dropped.size && targetInstance.modProfiles?.length) {
            const profiles = targetInstance.modProfiles.map((p) => ({ ...p, enabled: p.enabled.filter((key) => !dropped.has(key)) }));
            await api.set_mod_profiles({ id: targetInstance.id, profiles, active: targetInstance.activeProfile || '' }).catch(fail);
        }
        await api.select_instance({ id: targetInstance.id });
        store.settings.selectedInstance = targetInstance.id;
        await loadInstances();
        emit('settings');
        toast(`已升級到 Minecraft ${target}`);
        api.prefetch({ id: targetInstance.id }).catch(() => {});
    } catch (error) {
        fail(error);
    }
}

async function pickUpgrade(instance) {
    const data = await versionData().catch(() => null);
    if (!data) return toast('暫時無法取得版本清單', 'warn');
    const index = data.versions.findIndex((v) => v.id === instance.mcVersion);
    const newer = data.versions.slice(0, index < 0 ? 0 : index).filter((v) => v.type === 'release');
    if (!newer.length) return toast(`「${instance.name}」已經是最新的正式版 ${instance.mcVersion}`);
    const supported = new Set(data.modVersions);
    const select = h('select', {}, ...newer.map((v) => h('option', { value: v.id, selected: v.id === data.recommended },
        `${v.id}${v.id === data.recommended ? '（輝煌推薦）' : supported.has(v.id) ? '（輝煌支援）' : ''}`)));
    const go = await dialog({
        title: `升級「${instance.name}」`,
        content: h('div', { class: 'field' },
            h('label', {}, `從 ${instance.mcVersion} 升級到`),
            select,
            h('div', { class: 'hint' }, '下一步會逐一檢查每個模組在新版本有沒有對應版本，再決定更新、保留或先停用。')),
        buttons: [{ label: '取消', value: false }, { label: '檢查相容性', value: true }]
    });
    if (go === true) upgradeFlow(instance, select.value);
}

const PART_LABEL = {
    saves: '存檔',
    'options.txt': '遊戲設定與按鍵',
    'servers.dat': '伺服器清單',
    resourcepacks: '資源包',
    shaderpacks: '光影',
    screenshots: '截圖',
    mods: '模組（版本相同時）',
    config: '模組設定檔'
};

export async function importFlow() {
    const box = h('div', { class: 'rows import-list' }, h('div', { class: 'empty-state' }, '正在尋找電腦上的其他啟動器⋯'));
    let chosen = null;
    const LOADER = { fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' };
    const show = (sources, empty) => clear(box).append(...(sources.length ? sources.map((src) => h('button', {
        type: 'button',
        class: 'mc import-source',
        onclick: () => { chosen = src; $('#dlg-foot button').click(); }
    },
    h('span', { class: 'grow' },
        h('b', {}, src.name),
        h('small', {}, `${src.launcher}｜${src.mcVersion || '版本未知'}${LOADER[src.loader] ? `・${LOADER[src.loader]} ${src.loaderVersion}` : ''}｜存檔 ${src.saves}・模組 ${src.mods}`)),
    icon('arrow-right')))
        : [h('div', { class: 'empty-state' }, empty)]));
    const pickFolder = h('button', {
        type: 'button',
        class: 'mc',
        onclick: async () => {
            const path = await pickFiles({ directory: true, multiple: false }).catch(fail);
            if (!path) return;
            show(await api.import_inspect({ path }).catch((error) => (fail(error), [])), '這個資料夾裡找不到可以匯入的實例。可以選啟動器的資料夾、instances 資料夾，或單一實例資料夾。');
        }
    }, icon('folder'), '手動選擇資料夾');
    const data = await versionData().catch(() => null);
    api.import_sources().then((sources) => show(sources, '沒有自動找到其他啟動器。MultiMC 這類免安裝版可以用「手動選擇資料夾」。')).catch(fail);
    await dialog({
        title: '從其他啟動器匯入',
        wide: true,
        content: h('div', {},
            h('div', { class: 'toolbar' },
                h('span', { class: 'toolbar-label' }, '支援官方啟動器、Prism、MultiMC、PolyMC、CurseForge、Modrinth App、X Minecraft Launcher。原本的資料不會被移動或刪除。'),
                h('span', { class: 'spacer' }),
                h('button', { type: 'button', class: 'mc', onclick: () => { $('#dlg').close(); importPack(null); } }, icon('briefcase'), '匯入 .mrpack 模組包'),
                pickFolder),
            box),
        buttons: [{ label: '取消', value: null }]
    });
    if (!chosen) return;

    const known = data?.versions?.some((v) => v.id === chosen.mcVersion);
    const mc = known ? chosen.mcVersion : (data?.recommended || '');
    const name = h('input', { type: 'text', value: chosen.name.slice(0, 40), maxlength: '40' });
    const target = segmented([['new', '建立新實例'], ['current', '匯入到目前的實例']], 'new');
    const checks = Object.entries(PART_LABEL).map(([part, label]) => {
        const sameGame = chosen.mcVersion === mc && chosen.loader === 'fabric';
        const input = h('input', { type: 'checkbox', checked: part === 'mods' ? sameGame : part !== 'screenshots', value: part });
        return h('label', {}, input, label);
    });
    const ok = await dialog({
        title: `匯入「${chosen.name}」`,
        content: h('div', {},
            h('div', { class: 'field' }, target),
            h('div', { class: 'field' }, h('label', {}, '新實例名稱'), name,
                h('div', { class: 'hint' }, `會建立 Minecraft ${mc || '（請先選版本）'}${chosen.loader === 'fabric' ? ' + Fabric' : ''} 的實例${known ? '' : '（原版本不在清單上，改用推薦版本）'}。${['forge', 'neoforge', 'quilt'].includes(chosen.loader) ? `輝煌啟動器只支援 Fabric，${LOADER[chosen.loader]} 模組不會帶過來。` : ''}`)),
            h('div', { class: 'field import-parts' }, h('label', {}, '要帶過來的內容'), ...checks)),
        buttons: [{ label: '取消', value: false }, { label: '開始匯入', value: true }]
    });
    if (ok !== true) return;
    if (target.value === 'new' && name.value.trim() && nameTaken(name.value)) return toast(`已經有叫「${name.value.trim()}」的實例了，請換一個名稱`, 'error');
    const parts = checks.map((label) => label.querySelector('input')).filter((i) => i.checked).map((i) => i.value);
    try {
        let instance = selectedInstance();
        if (target.value === 'new' || !instance) {
            if (!mc) throw new Error('無法判斷要用的 Minecraft 版本');
            instance = await createFromDraft({ ...BLANK, name: name.value.trim() || chosen.name, icon: 'archive', mcVersion: mc, loader: chosen.loader === 'fabric' ? 'fabric' : 'vanilla', loaderVersion: known ? chosen.loaderVersion : '' });
        }
        toast('正在複製檔案⋯');
        const bytes = await api.import_into({ request: { path: chosen.path, instanceId: instance.id, parts } });
        toast(`匯入完成，共 ${fmtBytes(bytes)}`);
    } catch (error) {
        fail(error);
    }
}

const GAME_MODE = ['生存', '創造', '冒險', '旁觀'];

async function showWorlds(instance) {
    const list = h('div', { class: 'rows world-list' }, h('div', { class: 'empty-state' }, '讀取世界清單⋯'));
    const notice = h('div', {});
    shareNotice(instance, 'saves').then((el) => el && notice.replaceWith(el));
    const refresh = async () => {
        const worlds = await api.list_worlds({ id: instance.id }).catch((error) => (fail(error), []));
        clear(list).append(...(worlds.length ? worlds.map((w) => h('div', { class: 'row world-row' },
            w.icon ? h('img', { class: 'world-icon', src: fileSrc(w.icon), alt: '' }) : h('span', { class: 'world-icon' }, icon('map')),
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, w.name, w.hardcore ? h('span', { class: 'badge warn' }, '極限') : null),
                h('div', { class: 'meta' }, [
                    GAME_MODE[w.gameMode] ?? '生存',
                    fmtBytes(w.size),
                    w.lastPlayed ? `上次遊玩 ${new Date(w.lastPlayed).toLocaleString('zh-TW', { hour12: false })}` : '',
                    w.folder !== w.name ? `資料夾：${w.folder}` : ''
                ].filter(Boolean).join('｜'))),
            h('button', { type: 'button', class: 'mc', title: '複製到其他實例', onclick: () => copyWorld(instance, w).then(refresh) }, icon('copy'), '複製到⋯'),
            h('button', {
                type: 'button',
                class: 'mc',
                title: '匯出成 zip',
                onclick: async () => {
                    toast(`正在匯出「${w.name}」⋯`);
                    try {
                        const path = await api.world_export({ id: instance.id, folder: w.folder });
                        toast(`已匯出到「下載」資料夾：${path.split(/[\\/]/).pop()}`);
                        api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
                    } catch (error) {
                        fail(error);
                    }
                }
            }, icon('download'), '匯出'),
            h('button', {
                type: 'button',
                class: 'mc icon-btn danger',
                title: '刪除',
                'aria-label': '刪除',
                onclick: async () => {
                    if (!(await confirm(`要刪除世界「${w.name}」嗎？\n${RECYCLE_NOTE}`, '刪除'))) return showWorlds(instance);
                    await api.world_delete({ id: instance.id, folder: w.folder }).then(() => toast(`已刪除「${w.name}」`)).catch(fail);
                    showWorlds(instance);
                }
            }, icon('trash'))))
            : [h('div', { class: 'empty-state' }, '這個實例還沒有任何單人世界。')]));
    };
    const importWorld = async (directory, dropped = null) => {
        const path = dropped ?? await (directory ? pickFiles({ directory: true, multiple: false }) : pickFile('world')).catch(fail);
        if (!path) return;
        toast('正在匯入世界⋯');
        try {
            const name = await api.world_import({ id: instance.id, path });
            toast(`已匯入世界「${name}」`);
        } catch (error) {
            fail(error);
        }
        refresh();
    };
    refresh();
    await dialog({
        title: `世界管理｜${instance.name}`,
        wide: true,
        content: acceptFiles(h('div', {},
            h('div', { class: 'toolbar' },
                h('button', { type: 'button', class: 'mc primary', onclick: () => importWorld(false) }, icon('plus'), '匯入 zip'),
                h('button', { type: 'button', class: 'mc', onclick: () => importWorld(true) }, icon('folder'), '匯入資料夾'),
                h('span', { class: 'spacer' }),
                h('button', { type: 'button', class: 'mc', onclick: () => openFolder(instance, 'saves') }, icon('folder'), '開啟存檔資料夾')),
            notice,
            list,
            h('div', { class: 'hint' }, '可以把世界 zip 直接拖進這個視窗匯入。遊戲執行中無法匯入、複製或刪除世界。')), 'world', (path) => importWorld(false, path))
    });
}

async function copyWorld(instance, world) {
    const targets = store.instances.filter((i) => i.id !== instance.id);
    if (!targets.length) return toast('沒有其他實例可以複製過去', 'warn');
    const select = h('select', {}, ...targets.map((t) => h('option', { value: t.id }, `${t.name}（${t.mcVersion}）`)));
    const ok = await dialog({
        title: `複製「${world.name}」`,
        content: h('div', { class: 'field' }, h('label', {}, '複製到哪個實例'), select,
            h('div', { class: 'hint' }, '用比較舊的版本開啟新版本的世界可能會損壞，建議複製到相同或更新的版本。')),
        buttons: [{ label: '取消', value: false }, { label: '複製', value: true }]
    });
    if (ok === true) {
        const target = targets.find((t) => t.id === select.value);
        toast(`正在把「${world.name}」複製到「${target.name}」⋯`);
        try {
            const name = await api.world_copy({ id: instance.id, folder: world.folder, target: target.id });
            toast(`已複製到「${target.name}」（${name}）`);
        } catch (error) {
            fail(error);
        }
    }
    return showWorlds(instance);
}

const BACKUP_KIND = { auto: '自動', manual: '手動', 'before-restore': '還原前' };

async function manageBackups(instance) {
    const list = h('div', { class: 'rows' }, h('div', { class: 'empty-state' }, '讀取中⋯'));
    const refresh = async () => {
        const backups = await api.list_backups({ id: instance.id }).catch((error) => (fail(error), []));
        clear(list).append(...(backups.length ? backups.map((b) => h('div', { class: 'row' },
            h('span', { class: 'row-icon' }, icon('archive')),
            h('div', { class: 'grow' },
                h('div', { class: 'name' }, new Date(b.time * 1000).toLocaleString('zh-TW', { hour12: false })),
                h('div', { class: 'meta' }, `${BACKUP_KIND[b.kind] ?? b.kind}備份｜${fmtBytes(b.size)}`)),
            h('button', {
                type: 'button',
                class: 'mc',
                onclick: async () => {
                    if (!(await confirm(`要把存檔還原到 ${new Date(b.time * 1000).toLocaleString('zh-TW', { hour12: false })} 的狀態嗎？\n目前的存檔會先自動備份一份「還原前」備份，再移到回收資料夾。${groupList().find((g) => g.name === instance.group)?.shared?.includes('saves') ? `\n存檔由分組「${instance.group}」共用，分組裡的其他實例也會一起還原。` : ''}`, '還原'))) return manageBackups(instance);
                    toast('正在還原存檔⋯');
                    try {
                        await api.restore_backup({ id: instance.id, file: b.file });
                        toast('存檔已還原');
                    } catch (error) {
                        fail(error);
                    }
                    manageBackups(instance);
                }
            }, '還原'),
            h('button', {
                type: 'button',
                class: 'mc icon-btn danger',
                title: '刪除',
                'aria-label': '刪除',
                onclick: async () => {
                    if (!(await confirm(`要刪除這份備份嗎？\n${b.file}\n${RECYCLE_NOTE}`, '刪除'))) return manageBackups(instance);
                    await api.delete_backup({ id: instance.id, file: b.file }).then(() => toast('已刪除備份'), fail);
                    manageBackups(instance);
                }
            }, icon('trash'))))
            : [h('div', { class: 'empty-state' }, '還沒有任何備份。')]));
    };
    const content = h('div', {},
        h('div', { class: 'toolbar' },
            h('button', {
                type: 'button',
                class: 'mc primary',
                onclick: async (e) => {
                    e.currentTarget.disabled = true;
                    try {
                        await api.backup_now({ id: instance.id });
                        toast('已備份存檔');
                    } catch (error) {
                        fail(error);
                    }
                    e.target.closest('button').disabled = false;
                    refresh();
                }
            }, icon('save'), '立即備份'),
            h('span', { class: 'spacer' }),
            h('button', { type: 'button', class: 'mc', onclick: () => openFolder(instance, '../backups') }, icon('folder'), '開啟備份資料夾')),
        list,
        h('div', { class: 'hint' }, '可在「設定 → 啟動流程」開啟每次啟動前自動備份。'));
    refresh();
    await dialog({ title: `存檔備份｜${instance.name}`, content, wide: true });
}

export async function shareScreenshot(instance, shot) {
    const input = h('input', { type: 'text', maxlength: 60, placeholder: '介紹一下這張截圖（選填，最多 60 字）' });
    const go = await dialog({
        title: '投稿到玩家足跡',
        content: h('div', { class: 'share-shot' },
            h('img', { src: fileSrc(shot.path), alt: shot.name }),
            input,
            h('p', { class: 'hint' }, '投稿會用目前登入的正版帳號驗證身分，你的 Discord 需要先在遊戲內用 /discord link 綁定。管理員審核通過後，截圖會出現在官網的玩家足跡與首頁輪播。')),
        buttons: [{ label: '取消', value: false }, { label: '送出投稿', value: true }]
    });
    if (go === true) {
        toast('正在上傳截圖⋯');
        await api.share_screenshot({ id: instance.id, file: shot.name, description: input.value }).then((message) => toast(message), async (error) => {
            fail(error);
            if (/discord link/.test(String(error?.message ?? error)) && await confirm('你的帳號還沒綁定 Discord，要打開官網的綁定教學嗎？', '打開教學')) {
                openUrl(`https://www.brilliantw.net/${encodeURIComponent('連結帳號')}/`);
            }
        });
    }
}

async function showScreenshots(instance) {
    const grid = h('div', { class: 'shots' }, h('div', { class: 'empty-state' }, '讀取中⋯'));
    const fill = () => api.list_screenshots({ id: instance.id }).then((shots) => {
        clear(grid).append(...(shots.length ? shots.map((shot) => h('div', { class: 'shot' },
            h('button', { type: 'button', class: 'shot-open', title: shot.name, onclick: () => api.open_path({ path: shot.path }).catch(fail) },
                h('img', { src: fileSrc(shot.path), alt: shot.name, loading: 'lazy' })),
            h('div', { class: 'shot-foot' },
                h('span', {}, new Date(shot.time * 1000).toLocaleString('zh-TW', { hour12: false })),
                h('button', { type: 'button', class: 'mc', onclick: () => shareScreenshot(instance, shot).then(() => showScreenshots(instance)) }, icon('heart'), '投稿'))))
            : [h('div', { class: 'empty-state' }, '還沒有截圖。遊戲中按 F2 就能截圖。')]));
    }).catch(fail);
    fill();
    const stop = watchFolders(['screenshots'], () => instance.id, fill);
    try {
        await dialog({
            title: `截圖｜${instance.name}`,
            wide: true,
            content: h('div', {},
                h('div', { class: 'toolbar' },
                    h('span', { class: 'toolbar-label' }, '點一下用系統預設程式開啟'),
                    h('span', { class: 'spacer' }),
                    h('button', { type: 'button', class: 'mc', onclick: () => openFolder(instance, 'screenshots') }, icon('folder'), '開啟截圖資料夾')),
                grid)
        });
    } finally {
        stop();
    }
}

async function createShortcut(instance) {
    const target = h('select', {},
        h('option', { value: '' }, '只開啟遊戲（進主畫面）'),
        ...(instance.general ? [] : [['s1', '炒飯服'], ['s2', '書碼服'], ['s3', '羽森服'], ['s4', '恐龍服']].map(([value, label]) =>
            h('option', { value }, `直接連線 ${label}`))));
    const ok = await dialog({
        title: `建立「${instance.name}」的桌面捷徑`,
        content: h('div', {},
            h('p', { class: 'hint' }, '在桌面放一個捷徑，點兩下就用這個實例直接開始遊戲，不必先操作啟動器。'),
            h('div', { class: 'field' }, h('label', {}, '開啟後'), target)),
        buttons: [{ label: '取消', value: false }, { label: '建立', value: true }]
    });
    if (ok !== true) return;
    try {
        const path = await api.create_shortcut({ id: instance.id, join: target.value || null });
        toast(`已建立捷徑：${path.split(/[\\/]/).pop()}`);
    } catch (error) {
        fail(error);
    }
}

async function exportDiagnostics(instance) {
    toast('正在打包診斷資料⋯');
    try {
        const path = await api.export_diagnostics({ id: instance.id });
        toast(`已匯出到「下載」資料夾：${path.split(/[\\/]/).pop()}\n裡面的登入憑證已自動隱藏，可以放心傳給管理員。`);
        api.open_path({ path: path.replace(/[\\/][^\\/]+$/, '') }).catch(() => {});
    } catch (error) {
        fail(error);
    }
}

export function moreMenu(instance) {
    const item = (name, label, handler, extra = '') =>
        h('button', { type: 'button', role: 'menuitem', class: extra, onclick: (e) => { e.stopPropagation(); closeMenus(); handler(); } }, icon(name), label);
    const menu = h('div', { class: 'inst-menu', role: 'menu', hidden: true },
        item('map', '世界管理', () => showWorlds(instance)),
        item('save', '存檔備份', () => manageBackups(instance)),
        item('image', '截圖', () => showScreenshots(instance)),
        item('zap', '效能健檢', () => perfCheck(instance)),
        item('arrow-right', '升級版本⋯', () => pickUpgrade(instance)),
        item('arrow-up', pinnedKey() === `i:${instance.id}` ? '取消置頂' : '置頂實例', () => togglePin(`i:${instance.id}`)),
        item('folder', '移到分組⋯', () => moveToGroup(instance).catch(fail)),
        item('folder', '開啟資料夾', () => openFolder(instance)),
        item('reload', '驗證並修復檔案', () => repairInstance(instance)),
        navigator.userAgent.includes('Mac') ? null : item('external-link', '建立桌面捷徑', () => createShortcut(instance)),
        item('bug', '匯出診斷包', () => exportDiagnostics(instance)),
        item('copy', '複製實例', () => duplicateInstance(instance)),
        item('trash', '刪除實例', () => removeInstance(instance), 'danger'));
    const toggle = h('button', {
        type: 'button',
        class: 'mc icon-btn',
        title: '更多',
        'aria-label': '更多',
        'aria-haspopup': 'menu',
        onclick: (e) => {
            e.stopPropagation();
            const open = menu.hidden;
            closeMenus();
            menu.hidden = !open;
            if (open) placeMenu(menu, toggle);
        }
    }, icon('more-horizontal'));
    return h('div', { class: 'inst-more' }, toggle, menu);
}

function placeMenu(menu, anchor) {
    const bounds = (anchor.closest('.page-wrap') ?? document.body).getBoundingClientRect();
    const rect = anchor.getBoundingClientRect();
    const above = rect.top - bounds.top - 12;
    const below = bounds.bottom - rect.bottom - 12;
    menu.style.maxHeight = '';
    const down = menu.scrollHeight > above && below > above;
    menu.classList.toggle('down', down);
    menu.style.maxHeight = `${Math.max(160, down ? below : above)}px`;
}

function closeMenus() {
    document.querySelectorAll('.inst-menu').forEach((m) => { m.hidden = true; });
    document.querySelectorAll('.inst-menu.floating').forEach((m) => m.remove());
}

export async function playInstance(instance) {
    if (isPreparing()) return toast('正在準備遊戲，請等這次完成後再啟動', 'warn');
    await selectInstance(instance).catch(fail);
    location.hash = '#home';
    emit('launch');
}

async function manageMods(instance) {
    await selectInstance(instance).catch(fail);
    openStore(instance.loader === 'fabric' ? 'mod' : 'resourcepack', 'installed');
    location.hash = '#mods';
}

let clickTimer = 0;

function card(instance) {
    const active = selectedInstance()?.id === instance.id;
    const running = isRunning(instance.id);
    const fabric = instance.loader === 'fabric';
    const inject = fabric && !instance.general && (instance.injectRichPresence ?? store.settings?.injectRichPresence ?? true);
    const owner = instance.account ? store.accounts.find((acc) => acc.id === instance.account) : null;
    const supported = store.versions?.modVersions;
    const unsupported = !instance.general && supported?.length && !supported.includes(instance.mcVersion);
    const tool = (name, label, handler, extra = '') =>
        h('button', {
            type: 'button',
            class: `mc icon-btn ${extra}`,
            title: label,
            'aria-label': label,
            onclick: (e) => { e.stopPropagation(); handler(); }
        }, icon(name));

    return h(
        'div',
        {
            class: `inst-card${active ? ' selected' : ''}`,
            'data-id': instance.id,
            title: '點一下選擇，點兩下開啟模組與資源',
            onclick: (e) => {
                clearTimeout(clickTimer);
                if (dragged() || e.detail > 1) return;
                clickTimer = setTimeout(() => selectInstance(instance).catch(fail), 250);
            },
            ondblclick: () => {
                clearTimeout(clickTimer);
                if (!dragged()) manageMods(instance);
            },
            onpointerdown: (e) => startDrag(e, e.currentTarget, { kind: 'instance', id: instance.id, label: instance.name })
        },
        h('div', { class: `inst-banner${instance.image && !isSvg(instance.image) ? ' has-image' : ''}`, style: instance.image && !isSvg(instance.image) ? `--cover:url("${fileSrc(instance.image)}")` : '' },
            isSvg(instance.image) ? svgFromFile(instance.image, 'cover-svg') : instance.image ? null : iconOf(instance.icon),
            h('div', { class: 'inst-flags' },
                pinnedKey() === `i:${instance.id}` ? h('span', { class: 'badge' }, '置頂') : null,
                running ? h('span', { class: 'badge warn' }, '執行中') : null,
                active ? h('span', { class: 'badge' }, '已選擇') : null)),
        h('div', { class: 'inst-body' },
            h('div', { class: 'inst-title', title: instance.name }, instance.name),
            h('div', { class: 'inst-tags' },
                h('span', { class: 'tag' }, `Minecraft ${instance.mcVersion}`),
                h('span', { class: 'tag' }, fabric ? `Fabric${instance.loaderVersion ? ` ${instance.loaderVersion}` : ''}` : '原版'),
                instance.general ? h('span', { class: 'tag' }, '一般實例') : null,
                inject ? h('span', { class: 'tag accent' }, 'Brilliant-Server-Mod') : null,
                owner ? h('span', { class: 'tag', title: '這個實例固定用這個帳號啟動' }, icon('user'), owner.name) : null,
                ...shareTags(instance)),
            h('div', { class: 'inst-meta' },
                icon('clock'),
                [fmtDuration(instance.playSeconds), instance.lastPlayed ? `上次 ${new Date(instance.lastPlayed * 1000).toLocaleDateString('zh-TW')}` : ''].filter(Boolean).join('｜')),
            unsupported
                ? h('div', { class: 'inst-warn' }, icon('warning-box'), '輝煌不支援這個版本',
                    store.versions?.recommended && fabric
                        ? h('button', { type: 'button', class: 'link', onclick: (e) => { e.stopPropagation(); upgradeFlow(instance, store.versions.recommended); } }, `升級到 ${store.versions.recommended}`)
                        : null)
                : null),
        h('div', { class: 'inst-actions' },
            h('button', {
                type: 'button',
                class: 'mc primary inst-play',
                onclick: (e) => { e.stopPropagation(); playInstance(instance); }
            }, icon(running ? 'close' : 'play'), running ? '結束遊戲' : '開始遊戲'),
            tool('edit', '設定', () => editInstance(instance)),
            tool('zap', fabric ? '模組與資源' : '資源包與光影', () => manageMods(instance)),
            moreMenu(instance))
    );
}

function iconChooser(initial, names) {
    let chosen = initial;
    const box = h('div', { class: 'icon-picker' });
    const paint = () => { for (const b of box.querySelectorAll('[data-value]')) b.setAttribute('aria-pressed', String(b.dataset.value === chosen)); };
    const choice = (value, title) => h('button', { type: 'button', class: 'mc', title, 'data-value': value, onclick: () => { chosen = value; paint(); } }, iconOf(value));
    const custom = h('button', {
        type: 'button',
        class: 'mc',
        title: `自訂圖片（${FILE_TYPES.icon.extensions.map((e) => e.toUpperCase()).join('、')}），也可以直接把圖片拖到這一區`,
        'aria-label': '自訂圖片',
        onclick: async () => useCustom(await pickFile('icon').catch(fail))
    }, icon('plus'));
    async function useCustom(path) {
        if (!path) return;
        const saved = await importPicture(path, { width: 256, height: 256, title: '裁切圖示', hint: '圖示會顯示成正方形，建議 256×256 以上。' }).catch(fail);
        if (!saved) return;
        chosen = `img:${saved}`;
        box.querySelector('[data-custom]')?.remove();
        const button = choice(chosen, '自訂圖片');
        button.dataset.custom = '';
        box.insertBefore(button, custom);
        paint();
    }
    acceptFiles(box, 'icon', useCustom);
    box.append(...names.map((name) => choice(name, name)));
    if (initial?.startsWith('img:')) {
        const button = choice(initial, '自訂圖片');
        button.dataset.custom = '';
        box.append(button);
    }
    box.append(custom);
    paint();
    return { el: box, get value() { return chosen; } };
}

function groupList() {
    const saved = (store.settings?.instanceGroups ?? []).filter((g) => g.name);
    const known = new Set(saved.map((g) => g.name));
    const extra = [...new Set(store.instances.map((i) => i.group).filter((name) => name && !known.has(name)))].map((name) => ({ name, icon: '', note: '', key: '', shared: [] }));
    return [...saved, ...extra];
}

const newKey = () => [...crypto.getRandomValues(new Uint8Array(8))].map((b) => b.toString(16).padStart(2, '0')).join('');
const cleanGroups = (groups) => groups.map(({ name, icon: ico, note, key, shared }) => ({ name, icon: ico || '', note: note || '', key: key || newKey(), shared: shared || [] }));
const saveGroups = (groups) => saveSettings({ instanceGroups: cleanGroups(groups) });
const sharedNames = (group) => (group?.shared ?? []).map((f) => SHARE_LABELS.find(([v]) => v === f)?.[1]).filter(Boolean);

let shareState = new Map();
let shareSeq = 0;

async function refreshShare() {
    const seq = ++shareSeq;
    const results = await api.shared_status({ ids: store.instances.map((i) => i.id) }).catch(() => null);
    if (!results || seq !== shareSeq) return;
    const next = new Map(results.map((r) => [r.id, r]));
    if (JSON.stringify([...next]) === JSON.stringify([...shareState])) return;
    shareState = next;
    render();
}

const folderLabel = (f) => SHARE_LABELS.find(([v]) => v === f)?.[1] ?? f;

function shareTags(instance) {
    const st = shareState.get(instance.id);
    if (!st) return [];
    return [
        st.linked.length ? h('span', { class: 'tag share-tag', title: `這些資料夾連到分組「${instance.group}」的共用資料夾` }, icon('folder'), `共用中：${st.linked.map(folderLabel).join('、')}`) : null,
        st.pending.length ? h('span', { class: 'tag share-pending', title: `${st.pending.map(folderLabel).join('、')}的共用設定還沒套用到這個實例，會在下次啟動遊戲時套用` }, icon('warning-box'), '下次啟動套用共用') : null
    ].filter(Boolean);
}

export async function shareNotice(instance, folder) {
    const [st] = await api.shared_status({ ids: [instance.id] }).catch(() => []);
    if (!st?.linked.includes(folder)) return null;
    const members = store.instances.filter((i) => i.group === instance.group);
    return h('div', { class: 'share-notice' }, icon('folder'),
        `這個${folderLabel(folder)}資料夾與分組「${instance.group}」的 ${members.length} 個實例共用，新增或刪除會影響所有參與的實例。`);
}

async function sharedResources(group) {
    const folders = SHARE_LABELS.filter(([v]) => group.shared.includes(v));
    let folder = folders[0][0];
    const tabs = h('div', { class: 'segmented share-tabs' });
    const body = h('div', { class: 'share-body' });
    const status = h('span', { class: 'hint' });
    const addButton = h('button', { type: 'button', class: 'mc primary', onclick: async () => add(await pickFile('pack', { multiple: true }).catch(fail)) }, icon('plus'), '加入檔案');
    const add = async (picked) => {
        const files = (Array.isArray(picked) ? picked : picked ? [picked] : []).filter((p) => isType(p, 'pack'));
        if (!files.length) return;
        try {
            const added = await api.shared_add({ key: group.key, folder, files });
            toast(`已加入 ${added} 個${folderLabel(folder)}，分組裡的實例都看得到`);
        } catch (error) {
            fail(error);
        }
        load();
    };
    const remove = async (item, button) => {
        if (!button.classList.contains('confirming')) {
            button.classList.add('confirming', 'danger');
            button.title = `再按一次即從共用資料夾移除「${item.title}」，分組「${group.name}」的所有實例都會少掉它（會移到回收資料夾）`;
            const label = [...button.childNodes].find((n) => n.nodeType === 3);
            const before = label?.textContent;
            if (label) label.textContent = '再按一次移除';
            setTimeout(() => {
                button.classList.remove('confirming', 'danger');
                if (label) label.textContent = before;
            }, 3000);
            return;
        }
        try {
            await api.shared_remove({ key: group.key, folder, names: [item.name] });
            toast(`已從共用資料夾移除「${item.title}」，可以在回收資料夾找回`);
        } catch (error) {
            fail(error);
        }
        load();
    };
    const row = (item) => h('div', { class: 'row shared-row' },
        item.icon ? h('img', { class: 'row-icon', src: item.icon.startsWith('data:') ? item.icon : fileSrc(item.icon), alt: '' }) : h('div', { class: 'row-icon' }, icon(folder === 'saves' ? 'map' : 'image')),
        h('div', { class: 'grow' },
            h('div', { class: 'name' }, item.title),
            h('div', { class: 'meta' }, [fmtBytes(item.size), new Date(item.modified * 1000).toLocaleString('zh-TW', { hour12: false }), item.title !== item.name ? item.name : ''].filter(Boolean).join('｜'))),
        h('button', { type: 'button', class: 'mc', onclick: (e) => remove(item, e.currentTarget) }, icon('trash'), '移除'));
    const shot = (item) => h('figure', { class: 'shared-shot' },
        h('img', { src: fileSrc(item.path), alt: item.name, loading: 'lazy' }),
        h('figcaption', {}, item.name),
        h('button', { type: 'button', class: 'mc icon-btn', title: '移除', 'aria-label': `移除 ${item.name}`, onclick: (e) => remove(item, e.currentTarget) }, icon('trash')));
    async function load() {
        for (const b of tabs.children) b.setAttribute('aria-selected', String(b.dataset.folder === folder));
        addButton.hidden = !['resourcepacks', 'shaderpacks'].includes(folder);
        clear(body).append(h('div', { class: 'empty-state' }, '讀取中⋯'));
        const items = await api.shared_items({ key: group.key, folder }).catch((error) => (fail(error), []));
        status.textContent = `${items.length} 個項目・${fmtBytes(items.reduce((sum, i) => sum + i.size, 0))}`;
        const hint = {
            resourcepacks: '可以在這裡加入或移除，也可以從「模組與資源」的商店安裝到這個分組的任一實例，都會出現在這裡。',
            shaderpacks: '可以在這裡加入或移除，也可以從「模組與資源」的商店安裝到這個分組的任一實例，都會出現在這裡。',
            saves: '在分組裡任一實例建立或匯入的世界都會出現在這裡。',
            screenshots: '分組裡所有實例拍的截圖都集中在這裡，相簿也會一起顯示。'
        }[folder];
        const list = !items.length
            ? h('div', { class: 'empty-state' }, `共用的${folderLabel(folder)}還是空的。`)
            : folder === 'screenshots' ? h('div', { class: 'shared-shots' }, ...items.map(shot)) : h('div', { class: 'rows' }, ...items.map(row));
        const zone = h('div', {}, h('p', { class: 'hint' }, hint), list);
        if (!addButton.hidden) acceptFiles(zone, 'pack', add, { multiple: true });
        clear(body).append(zone);
    }
    tabs.append(...folders.map(([value, label]) => h('button', { type: 'button', 'data-folder': value, onclick: () => { folder = value; load(); } }, label)));
    load();
    await dialog({
        title: `共用資源｜${group.name}`,
        wide: true,
        content: h('div', { class: 'shared-resources' },
            tabs,
            h('div', { class: 'toolbar' },
                addButton,
                h('button', { type: 'button', class: 'mc', onclick: async () => api.open_path({ path: await api.shared_folder_path({ key: group.key }) + (navigator.userAgent.includes('Windows') ? '\\' : '/') + folder }).catch(fail) }, icon('folder'), '開啟資料夾'),
                h('button', { type: 'button', class: 'mc', onclick: () => load() }, icon('reload'), '重新整理'),
                h('span', { class: 'spacer' }),
                status),
            body),
        buttons: [{ label: '關閉', value: null }]
    });
}

async function applySharing(ids, quiet = false) {
    if (!ids.length) return;
    try {
        const results = await api.apply_shared({ ids: [...new Set(ids)] });
        refreshShare();
        const lines = results.flatMap(([name, notes]) => notes.map((note) => `「${name}」${note}`));
        if (!lines.length) return;
        if (quiet && lines.length <= 2) return lines.forEach((line) => toast(line));
        await dialog({ title: '共用資料夾已更新', content: h('ul', { class: 'share-result' }, ...lines.map((line) => h('li', {}, line))), buttons: [{ label: '好', value: null }] });
    } catch (error) {
        fail(error);
    }
}

let migrating = false;
async function migrateLegacyShare() {
    const folders = store.settings?.sharedFolders ?? [];
    if (migrating || !folders.length || !store.instances.length) return;
    migrating = true;
    const users = store.instances.filter((i) => i.useShared);
    const groups = groupList();
    const patch = { sharedFolders: [] };
    if (users.length) {
        let name = '共用資料夾';
        for (let n = 2; groups.some((g) => g.name === name); n++) name = `共用資料夾 ${n}`;
        patch.instanceGroups = cleanGroups([...groups, { name, icon: 'folder', note: '從舊版共用設定轉移', key: 'legacy', shared: folders }]);
        await saveSettings(patch).catch(fail);
        for (const instance of users) {
            await api.update_instance({ updated: { ...instance, useShared: false, group: instance.group || name } }).catch(fail);
        }
        await loadInstances().catch(() => {});
        toast(`共用資料夾改由分組設定，原本的共用設定已轉成分組「${name}」`);
        await applySharing(users.map((i) => i.id), true);
    } else {
        await saveSettings(patch).catch(fail);
    }
    migrating = false;
}
const pinnedKey = () => store.settings?.pinned ?? '';

async function togglePin(key) {
    const next = pinnedKey() === key ? '' : key;
    await saveSettings({ pinned: next }).catch(fail);
    toast(next ? '已置頂，原本置頂的項目已取消置頂' : '已取消置頂');
}

function sortMembers(members) {
    const pinned = pinnedKey();
    return [...members].sort((a, b) => (pinned === `i:${b.id}`) - (pinned === `i:${a.id}`));
}

function render() {
    const box = clear($('#instance-list'));
    if (!store.instances.length && !groupList().length) {
        box.append(h('div', { class: 'empty-state inst-empty' },
            icon('zap'),
            h('div', {}, '還沒有任何實例'),
            h('div', { class: 'hint' }, '實例是一套獨立的遊戲版本、模組與存檔。建議先一鍵建立輝煌實例。'),
            h('button', { type: 'button', class: 'mc primary', onclick: () => quickCreate().catch(fail) }, icon('plus'), '一鍵建立輝煌實例')));
        return;
    }
    const groups = groupList();
    if (!groups.length) {
        box.append(...sortMembers(store.instances).map(card));
        return;
    }
    const pinned = pinnedKey();
    const ordered = [...groups.filter((g) => pinned === `g:${g.name}`), ...groups.filter((g) => pinned !== `g:${g.name}`)];
    const loose = store.instances.filter((i) => !i.group || !groups.some((g) => g.name === i.group));
    const sections = [...ordered.filter((g) => pinned === `g:${g.name}`), ...(loose.length ? [{ name: '', icon: '', note: '' }] : []), ...ordered.filter((g) => pinned !== `g:${g.name}`)];
    const folded = foldedGroups();
    for (const group of sections) {
        const members = sortMembers(group.name ? store.instances.filter((i) => i.group === group.name) : loose);
        const closed = folded.has(group.name);
        box.append(groupHeader(group, members, closed));
        if (closed) continue;
        if (members.length) box.append(...members.map(card));
        else box.append(h('div', { class: 'inst-group-empty', 'data-group': group.name }, '把實例拖到這裡加入這個分組'));
    }
}

const FOLD_KEY = 'instance-groups-folded';

function foldedGroups() {
    try {
        return new Set(JSON.parse(localStorage.getItem(FOLD_KEY) || '[]'));
    } catch {
        return new Set();
    }
}

function setFolded(folded) {
    try {
        localStorage.setItem(FOLD_KEY, JSON.stringify([...folded]));
    } catch {}
}

function toggleFold(name) {
    const folded = foldedGroups();
    if (!folded.delete(name)) folded.add(name);
    setFolded(folded);
    render();
}

function groupHeader(group, members, closed) {
    const running = members.filter((i) => isRunning(i.id)).length;
    const pinned = group.name && pinnedKey() === `g:${group.name}`;
    const header = h('div', {
        class: `inst-group${group.name ? ' movable' : ''}`,
        'data-group': group.name,
        oncontextmenu: (e) => {
            if (!group.name) return;
            e.preventDefault();
            groupMenu(group, members, e.clientX, e.clientY);
        },
        onpointerdown: (e) => group.name && startDrag(e, header, { kind: 'group', name: group.name, label: group.name })
    },
    h('button', { type: 'button', class: 'inst-group-name', 'aria-expanded': String(!closed), title: group.name ? '點一下收合；右鍵可編輯、置頂；拖曳可調整順序' : '', onclick: () => { if (!dragged()) toggleFold(group.name); } },
        icon('chevron-down', 'ico fold'), iconOf(group.name ? group.icon || 'folder' : 'folder'), h('b', {}, group.name || '未分組'),
        pinned ? h('span', { class: 'badge' }, '置頂') : null,
        sharedNames(group).length ? h('span', { class: 'badge share-badge', title: '這個分組的實例共用這些資料夾；後面的數字是已經套用的實例數' }, `共用 ${sharedNames(group).join('、')}${members.length ? `・${members.filter((m) => !shareState.get(m.id)?.pending.length).length}/${members.length} 已套用` : ''}`) : null,
        h('span', { class: 'hint' }, `${members.length} 個實例${running ? `・${running} 個執行中` : ''}`),
        group.note ? h('span', { class: 'inst-group-note', title: group.note }, group.note) : null),
    group.name && group.shared?.length && group.key ? h('button', {
        type: 'button',
        class: 'mc',
        title: '管理這個分組共用的資源包、光影、存檔或截圖',
        onclick: () => sharedResources(group).catch(fail)
    }, icon('folder'), '共用資源') : null,
    group.name ? h('button', {
        type: 'button',
        class: 'mc',
        disabled: !members.length || running === members.length,
        title: '依序啟動這一組所有還沒在執行的實例',
        onclick: () => launchGroup(group.name, members).catch(fail)
    }, icon('play'), '全部啟動') : null);
    return header;
}

function groupMenu(group, members, x, y) {
    closeMenus();
    const item = (name, label, handler, extra = '') =>
        h('button', { type: 'button', role: 'menuitem', class: extra, onclick: (e) => { e.stopPropagation(); closeMenus(); handler(); } }, icon(name), label);
    const menu = h('div', { class: 'inst-menu floating', role: 'menu' },
        item('edit', '編輯分組⋯', () => groupDialog(group).catch(fail)),
        item('folder', '共用資料夾設定⋯', () => groupDialog(group).then(() => {}).catch(fail)),
        group.shared?.length && group.key ? item('archive', '管理共用資源⋯', () => sharedResources(group).catch(fail)) : null,
        item('arrow-up', pinnedKey() === `g:${group.name}` ? '取消置頂' : '置頂分組', () => togglePin(`g:${group.name}`)),
        members.length ? item('play', '全部啟動', () => launchGroup(group.name, members).catch(fail)) : null,
        item('trash', '刪除分組（實例保留）', () => removeGroup(group).catch(fail), 'danger'));
    document.body.append(menu);
    const rect = menu.getBoundingClientRect();
    menu.style.left = `${Math.min(x, innerWidth - rect.width - 8)}px`;
    menu.style.top = `${Math.min(y, innerHeight - rect.height - 8)}px`;
}

async function setGroupOf(list, name) {
    const moved = [];
    for (const instance of list) {
        if ((instance.group || '') === name) continue;
        await api.update_instance({ updated: { ...instance, group: name } }).catch(fail);
        moved.push(instance.id);
    }
    await loadInstances().catch(fail);
    await applySharing(moved, true);
}

async function groupDialog(existing = null) {
    const iconPick = iconChooser(existing?.icon || 'folder', ['folder', ...INSTANCE_ICONS]);
    const icons = iconPick.el;
    const name = h('input', { type: 'text', value: existing?.name ?? '', maxlength: '24', placeholder: '例如：輝煌、建築、朋友的伺服器' });
    const note = h('input', { type: 'text', value: existing?.note ?? '', maxlength: '120', placeholder: '選填，會顯示在分組標題旁' });
    const picks = store.instances.map((instance) => ({ instance, input: h('input', { type: 'checkbox', checked: !!existing && instance.group === existing.name, onchange: () => warnShare() }) }));
    const key = existing?.key || newKey();
    const sharing = new Set(existing?.shared ?? []);
    const shareCards = SHARE_LABELS.map(([value, label]) => {
        const input = h('input', { type: 'checkbox', checked: sharing.has(value), onchange: () => { input.checked ? sharing.add(value) : sharing.delete(value); warnShare(); } });
        return h('label', { class: 'share-card' }, h('span', { class: 'share-head' }, input, icon(SHARE_INFO[value][0]), h('b', {}, label)), h('small', {}, SHARE_INFO[value][1]));
    });
    const shareWarn = h('div', { class: 'share-warn', hidden: true });
    const warnShare = () => {
        const versions = new Set(picks.filter((p) => p.input.checked).map((p) => p.instance.mcVersion));
        shareWarn.hidden = !(sharing.has('saves') && versions.size > 1);
        shareWarn.textContent = `注意：這個分組有不同 Minecraft 版本的實例（${[...versions].join('、')}），共用存檔可能讓舊版本打不開被升級過的世界。`;
    };
    warnShare();
    const location = h('code', {}, existing?.key ? '讀取中⋯' : '建立分組後才會產生');
    if (existing?.key) api.shared_folder_path({ key }).then((path) => { location.textContent = path; }).catch(() => {});
    const ok = await dialog({
        title: existing ? `編輯分組「${existing.name}」` : '建立分組',
        content: h('div', { class: 'inst-editor' },
            h('div', { class: 'field' }, h('label', {}, '名稱'), name),
            h('div', { class: 'field' }, h('label', {}, '圖示'), icons),
            h('div', { class: 'field' }, h('label', {}, '備註'), note),
            picks.length ? h('div', { class: 'field' }, h('label', {}, '加入的實例'),
                h('div', { class: 'group-picks' }, ...picks.map(({ instance, input }) => h('label', { class: 'group-pick' }, input, instanceVisual(instance), h('span', {}, instance.name),
                    instance.group && instance.group !== existing?.name ? h('small', {}, `目前在「${instance.group}」`) : null)))) : null,
            h('div', { class: 'field share-manager' }, h('label', {}, '共用資料夾'),
                h('div', { class: 'hint' }, '勾選的資料夾由這個分組的實例共用一份，其他分組不受影響。加入共用時，實例原本的內容會搬進分組的共用資料夾（同名會自動改名，不會覆蓋）；實例離開分組或取消共用時，會把共用內容複製一份回實例。'),
                h('div', { class: 'share-cards' }, ...shareCards),
                shareWarn,
                h('div', { class: 'share-location' }, location, existing?.key ? h('button', { type: 'button', class: 'mc', onclick: async () => api.open_path({ path: await api.shared_folder_path({ key }) }).catch(fail) }, icon('folder'), '開啟') : null))),
        buttons: [{ label: '取消', value: false }, { label: existing ? '儲存並套用' : '建立', value: true }]
    });
    if (ok !== true) return;
    const title = name.value.trim().slice(0, 24);
    if (!title) return toast('請輸入分組名稱', 'error');
    const groups = groupList();
    if (groups.some((g) => g.name === title && g.name !== existing?.name)) return toast(`已經有叫「${title}」的分組`, 'error');
    const entry = { name: title, icon: iconPick.value, note: note.value.trim().slice(0, 120), key, shared: SHARE_LABELS.map(([v]) => v).filter((v) => sharing.has(v)) };
    const shareChanged = (existing?.shared ?? []).join() !== entry.shared.join();
    const next = existing ? groups.map((g) => (g.name === existing.name ? entry : g)) : [...groups, entry];
    const patch = { instanceGroups: cleanGroups(next) };
    if (existing && existing.name !== title && pinnedKey() === `g:${existing.name}`) patch.pinned = `g:${title}`;
    await saveSettings(patch).catch(fail);
    if (existing && existing.name !== title) {
        const folded = foldedGroups();
        if (folded.delete(existing.name)) folded.add(title);
        setFolded(folded);
    }
    const join = picks.filter((p) => p.input.checked).map((p) => p.instance);
    const leave = existing ? picks.filter((p) => !p.input.checked && p.instance.group === existing.name).map((p) => p.instance) : [];
    const renamed = existing && existing.name !== title ? store.instances.filter((i) => i.group === existing.name && !leave.includes(i)) : [];
    await setGroupOf([...join, ...renamed], title);
    if (leave.length) await setGroupOf(leave, '');
    if (shareChanged) await applySharing(store.instances.filter((i) => i.group === title).map((i) => i.id));
    toast(existing ? `已更新分組「${title}」` : `已建立分組「${title}」`);
}

async function removeGroup(group) {
    const members = store.instances.filter((i) => i.group === group.name);
    if (!(await confirm(`確定要刪除分組「${group.name}」嗎？${members.length ? `\n裡面的 ${members.length} 個實例會移到「未分組」，不會被刪除。` : ''}`, '刪除分組'))) return;
    const patch = { instanceGroups: cleanGroups(groupList().filter((g) => g.name !== group.name)) };
    if (pinnedKey() === `g:${group.name}`) patch.pinned = '';
    await saveSettings(patch).catch(fail);
    await setGroupOf(members, '');
}

let dragState = null;
let dragEndedAt = 0;
const dragged = () => Date.now() - dragEndedAt < 300;

function startDrag(event, source, item) {
    if (event.button !== 0 || event.target.closest('.inst-actions, .inst-group > .mc, input, select, textarea, .inst-menu')) return;
    dragState = { item, source, x: event.clientX, y: event.clientY, ghost: null };
    addEventListener('pointermove', moveDrag);
    addEventListener('pointerup', endDrag, { once: true });
    addEventListener('pointercancel', cancelDrag, { once: true });
}

function dropTargetAt(x, y) {
    const el = document.elementFromPoint(x, y);
    if (!dragState || !el) return null;
    if (dragState.item.kind === 'group') {
        const header = el.closest('.inst-group.movable');
        return header && header.dataset.group !== dragState.item.name ? header : null;
    }
    const spot = el.closest('#instance-list [data-group], #instance-list .inst-card');
    if (!spot) return null;
    const group = spot.dataset.group ?? store.instances.find((i) => i.id === spot.dataset.id)?.group ?? '';
    return { el: spot, group: groupList().some((g) => g.name === group) ? group : '' };
}

function moveDrag(event) {
    if (!dragState) return;
    if (!dragState.ghost) {
        if (Math.hypot(event.clientX - dragState.x, event.clientY - dragState.y) < 6) return;
        dragState.ghost = h('div', { class: 'drag-ghost' }, icon(dragState.item.kind === 'group' ? 'folder' : 'zap'), dragState.item.label);
        document.body.append(dragState.ghost);
        dragState.source.classList.add('dragging');
        document.body.classList.add('inst-dragging');
    }
    dragState.ghost.style.transform = `translate(${event.clientX + 12}px, ${event.clientY + 12}px)`;
    const wrap = $('#view-instances').closest('.page-wrap') ?? $('.page-wrap');
    const edge = wrap?.getBoundingClientRect();
    if (edge && event.clientY > edge.bottom - 48) wrap.scrollBy(0, 16);
    else if (edge && event.clientY < edge.top + 48) wrap.scrollBy(0, -16);
    for (const el of $$('#instance-list .drop-over')) el.classList.remove('drop-over');
    const target = dropTargetAt(event.clientX, event.clientY);
    const mark = target?.el ?? target;
    if (dragState.item.kind === 'instance' && target) {
        const header = $(`#instance-list .inst-group[data-group="${CSS.escape(target.group)}"]`);
        header?.classList.add('drop-over');
    } else mark?.classList?.add('drop-over');
}

function cancelDrag() {
    removeEventListener('pointermove', moveDrag);
    if (dragState?.ghost) {
        dragState.ghost.remove();
        dragState.source.classList.remove('dragging');
        document.body.classList.remove('inst-dragging');
        dragEndedAt = Date.now();
    }
    for (const el of $$('#instance-list .drop-over')) el.classList.remove('drop-over');
    dragState = null;
}

async function endDrag(event) {
    const state = dragState;
    const moved = !!state?.ghost;
    const target = moved ? dropTargetAt(event.clientX, event.clientY) : null;
    removeEventListener('pointercancel', cancelDrag);
    cancelDrag();
    if (!moved || !target) return;
    if (state.item.kind === 'group') {
        const groups = groupList();
        const from = groups.findIndex((g) => g.name === state.item.name);
        const [moving] = groups.splice(from, 1);
        const rect = target.getBoundingClientRect();
        let to = groups.findIndex((g) => g.name === target.dataset.group);
        if (event.clientY > rect.top + rect.height / 2 || from <= to) to++;
        groups.splice(Math.min(to, groups.length), 0, moving);
        await saveGroups(groups).catch(fail);
        return;
    }
    const instance = store.instances.find((i) => i.id === state.item.id);
    if (!instance || (instance.group || '') === target.group) return;
    await setGroupOf([instance], target.group);
    toast(target.group ? `已把「${instance.name}」移到「${target.group}」` : `已把「${instance.name}」移出分組`);
}

async function launchGroup(name, members) {
    if (isPreparing()) return toast('正在準備遊戲，請等這次完成後再啟動', 'warn');
    const pending = members.filter((i) => !isRunning(i.id));
    if (!pending.length) return toast(`「${name}」的實例都已經在執行中`);
    const byAccount = new Map();
    for (const instance of members) {
        const account = accountFor(instance);
        if (!account) continue;
        byAccount.set(account.id, [...(byAccount.get(account.id) ?? []), instance]);
    }
    const shared = [...byAccount.values()].filter((list) => list.length > 1);
    if (shared.length && !(await confirm(`${shared.map((list) => `${list.map((i) => `「${i.name}」`).join('、')}都使用 ${accountFor(list[0]).name}`).join('\n')}。\n同一個帳號同時連進伺服器會互相踢下線，可以在實例設定的「使用帳號」各自指定帳號。\n仍要全部啟動嗎？`, '仍要啟動'))) return;
    let started = 0;
    for (const [index, instance] of pending.entries()) {
        toast(`正在啟動「${instance.name}」（${index + 1}/${pending.length}）`);
        try {
            const notes = await api.launch_game({ id: instance.id, join: null });
            for (const note of notes) toast(note, 'warn');
            started++;
            await loadInstances();
        } catch (error) {
            fail(error);
        }
    }
    toast(`「${name}」已啟動 ${started} 個實例`);
}

function groupInput(value = '') {
    const list = h('datalist', { id: `groups-${Math.random().toString(36).slice(2)}` },
        ...groupList().map((g) => h('option', { value: g.name })));
    const input = h('input', { type: 'text', value, maxlength: '24', placeholder: '例如：輝煌、建築、朋友的伺服器', list: list.id });
    const box = h('span', { class: 'group-input' }, input, list);
    box.input = input;
    return box;
}

async function moveToGroup(instance) {
    const group = groupInput(instance.group);
    const ok = await dialog({
        title: `把「${instance.name}」移到分組`,
        content: h('div', { class: 'field' }, h('label', {}, '分組名稱'), group, h('div', { class: 'hint' }, '輸入新名稱即建立新分組；留空表示不分組。也可以直接把實例卡片拖到分組上。')),
        buttons: [{ label: '取消', value: false }, { label: '確定', value: true }]
    });
    if (ok !== true) return;
    await assignGroup(instance, group.input.value.trim().slice(0, 24));
}

async function assignGroup(instance, name) {
    if (name && !groupList().some((g) => g.name === name)) await saveGroups([...groupList(), { name, icon: 'folder', note: '' }]).catch(fail);
    await setGroupOf([instance], name);
}

export function initInstances() {
    $('#btn-new-instance').addEventListener('click', guard(() => createInstance()));
    $('#btn-quick-instance').addEventListener('click', guard(() => quickCreate().catch(fail)));
    $('#btn-import').addEventListener('click', guard(() => importFlow().catch(fail)));
    $('#btn-new-group').addEventListener('click', guard(() => groupDialog().catch(fail)));
    $('#btn-refresh-instances').addEventListener('click', guard(async () => {
        store.versions = null;
        await loadInstances().catch(fail);
        await versionData().catch(() => {});
        render();
        toast('已重新整理');
    }));
    on('instances', render);
    document.addEventListener('click', closeMenus);
    on('settings', render);
    on('accounts', render);
    on('instances', () => { migrateLegacyShare(); refreshShare(); });
    on('settings', () => { migrateLegacyShare(); refreshShare(); });
    render();
    versionData().then(render, () => {});
}
