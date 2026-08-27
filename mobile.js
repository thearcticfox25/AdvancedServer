'use strict';

const MobileBrowser = {
    init() {
        history.replaceState({ vfsPath: Config.desktop.gridPath, crumbs: [] }, '');
        window.addEventListener('popstate', e => {
            if (e.state) this._render(e.state.vfsPath, e.state.crumbs);
        });
        this._checkDomain();
        this._render(Config.desktop.gridPath, []);
    },

    _navigate(vfsPath, crumbs) {
        history.pushState({ vfsPath, crumbs }, '');
        this._render(vfsPath, crumbs);
    },

    _render(vfsPath, crumbs) {
        this._buildBreadcrumb(crumbs);
        this._buildList(vfsPath, crumbs);
    },

    // Breadcrumb is RTL: current dir on LEFT, root on RIGHT, ‹ separators.
    _buildBreadcrumb(crumbs) {
        const bc = document.getElementById('mob-bc');
        bc.innerHTML = '';

        if (crumbs.length === 0) {
            const el = document.createElement('span');
            el.className   = 'mob-bc-crumb current';
            el.textContent = '/';
            bc.appendChild(el);
            return;
        }

        // Current dir — not clickable
        const cur = document.createElement('span');
        cur.className   = 'mob-bc-crumb current';
        cur.textContent = crumbs[crumbs.length - 1].label;
        bc.appendChild(cur);

        // Parent dirs from second-to-last up to first
        for (let i = crumbs.length - 2; i >= 0; i--) {
            bc.appendChild(this._sep());
            const el  = document.createElement('span');
            el.className   = 'mob-bc-crumb';
            el.textContent = crumbs[i].label;
            const snap = i;
            el.addEventListener('click', () =>
                this._navigate(crumbs[snap].path, crumbs.slice(0, snap + 1)));
            bc.appendChild(el);
        }

        // Root link
        bc.appendChild(this._sep());
        const root = document.createElement('span');
        root.className   = 'mob-bc-crumb';
        root.textContent = '/';
        root.addEventListener('click', () =>
            this._navigate(Config.desktop.gridPath, []));
        bc.appendChild(root);
    },

    _sep() {
        const s = document.createElement('span');
        s.className   = 'mob-bc-sep';
        s.textContent = '‹';
        return s;
    },

    _buildList(vfsPath, crumbs) {
        const list = document.getElementById('mob-list');
        list.innerHTML = '';

        const items = VFS.listDir(vfsPath);
        if (!items.length) {
            const el = document.createElement('div');
            el.className   = 'mob-empty';
            el.textContent = 'This folder is empty.';
            list.appendChild(el);
            return;
        }

        items.forEach(item => {
            const row = document.createElement('div');
            row.className = 'mob-item';
            if (item.linkUrl)  row.classList.add('is-link');
            if (item.hasIndex) row.classList.add('is-page');

            // ‹ arrow on the left
            const arrow = document.createElement('span');
            arrow.className   = 'mob-item-arrow';
            arrow.textContent = '‹';

            // Label right-aligned in the middle
            const label = document.createElement('span');
            label.className   = 'mob-item-label';
            label.textContent = item.label || item.name;

            // Icon from graphical-berries on the right
            const icon = document.createElement('img');
            icon.className = 'mob-item-icon';
            icon.alt       = '';
            icon.src       = item.iconPath || 'graphical-berries/folder.png';
            icon.onerror   = () => { icon.style.display = 'none'; };

            row.appendChild(arrow);
            row.appendChild(label);
            row.appendChild(icon);

            row.addEventListener('click', () => {
                if (item.linkUrl) {
                    window.open(item.linkUrl, '_blank');
                } else if (item.hasIndex && item.path) {
                    window.location.href = item.path;
                } else {
                    const childPath = vfsPath
                        ? `${vfsPath}/${item.name}`
                        : item.name;
                    this._navigate(childPath,
                        [...crumbs, { path: childPath, label: item.label || item.name }]);
                }
            });

            list.appendChild(row);
        });
    },

    _checkDomain() {
        const expected = Config.site?.expectedUrl;
        if (!expected) return;
        try {
            const exp = new URL(expected);
            const ok  = location.origin === exp.origin
                     && location.pathname.startsWith(exp.pathname);
            if (!ok) {
                const dw  = document.getElementById('mob-domain-warn');
                const url = document.getElementById('mob-dw-url');
                if (!dw || !url) return;
                url.textContent = expected;
                url.addEventListener('click', () => {
                    navigator.clipboard?.writeText(expected).catch(() => {});
                });
                dw.style.display = 'flex';
                document.getElementById('mob-dw-close')
                    ?.addEventListener('click', () => { dw.style.display = 'none'; });
            }
        } catch (e) { console.warn('[mob domain]', e); }
    },
};
