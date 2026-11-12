{
    let mode = null;
    try { mode = localStorage.getItem('brilliant.mode'); } catch {}
    document.documentElement.dataset.theme = mode === 'light' || mode === 'dark' ? mode : matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}
