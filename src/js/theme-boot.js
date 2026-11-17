{
    let mode = null;
    try { mode = localStorage.getItem('brilliant.mode'); } catch {}
    document.documentElement.dataset.theme = mode === 'light' || mode === 'dark' ? mode : matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    if (/Android/i.test(navigator.userAgent)) document.documentElement.dataset.platform = 'android';
    if (/Android/i.test(navigator.userAgent) && !('showPopover' in HTMLElement.prototype)) {
        const version = (/Chrome\/(\d+)/.exec(navigator.userAgent) || [])[1] || '?';
        addEventListener('DOMContentLoaded', () => {
            document.body.innerHTML = '';
            const box = document.createElement('div');
            box.setAttribute('role', 'alert');
            box.style.cssText = 'position:fixed;inset:0;display:flex;flex-direction:column;justify-content:center;gap:12px;padding:24px 48px;background:#16181c;color:#f2f2f2;font:16px/1.6 sans-serif;';
            box.innerHTML = '<b style="font-size:20px">需要更新 Android System WebView</b>'
                + '<span>這支手機的 WebView 版本是 ' + version + '，輝煌啟動器需要 114 以上。請到 Play 商店搜尋「Android System WebView」並更新，再重新開啟啟動器。</span>'
                + '<span style="color:#b9bdc3">Android System WebView ' + version + ' is too old. Update it to version 114 or newer from the Play Store, then reopen the launcher.</span>';
            document.body.append(box);
        });
    }
}
