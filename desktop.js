'use strict';

const Desktop = {
    _startOpen:   false,
    _desktopGrid: null,
    _ctxMenu:     null,

    init() {
        const gridEl = document.getElementById('desktop-grid');
        this._desktopGrid = new FileGrid(gridEl, {
            layout:    'desktop',
            fixedPath: Config.desktop.gridPath,
        });
        this._desktopGrid.refresh();

        this._buildStartMenu();
        this._bindGlobal();
        this.updateTaskbar();
    },

    _bindGlobal() {
        document.getElementById('start-btn').addEventListener('click', e => {
            e.stopPropagation();
            this.toggleStartMenu();
        });
        document.getElementById('show-desktop-btn').addEventListener('click', () => {
            WindowManager.minimizeAll();
        });
        document.addEventListener('click', () => {
            this.closeStartMenu();
            this._closeCtx();
            document.querySelectorAll('.wl-popup').forEach(p => p.remove());
        });
        document.addEventListener('contextmenu', e => {
            if (e.target.closest('.tb-btn')) e.preventDefault();
        });
    },

    updateTaskbar() {
        const bar = document.getElementById('taskbar-apps');
        bar.innerHTML = '';

        const groups = new Map();
        WindowManager.windows.forEach(win => {
            if (!groups.has(win.app_class))
                groups.set(win.app_class, { icon: win.icon_name, wins: [] });
            groups.get(win.app_class).wins.push(win);
        });

        [...groups.values()].reverse().forEach(group => {
            const n          = group.wins.length;
            const anyVisible = group.wins.some(w => !w.minimized);
            const anyFocused = group.wins.some(w => w.element.classList.contains('focused'));

            const btn = document.createElement('div');
            btn.className = 'tb-btn';
            if (anyVisible)  btn.classList.add('active');
            if (anyFocused)  btn.classList.add('focused-g');
            btn.classList.add(n > 1 ? 'multi' : 'single');

            btn.innerHTML =
                `<span class="tb-icon">${group.icon}</span>` +
                (n > 1 ? `<span class="tb-badge">${n}</span>` : '') +
                `<span class="tb-bar"></span>`;

            let hoverTimer, leaveTimer;
            btn.addEventListener('mouseenter', () => {
                clearTimeout(leaveTimer);
                hoverTimer = setTimeout(() => this._showWinList(btn, group.wins), 320);
            });
            btn.addEventListener('mouseleave', () => {
                clearTimeout(hoverTimer);
                leaveTimer = setTimeout(() => {
                    document.querySelectorAll('.wl-popup').forEach(p => p.remove());
                }, 250);
            });

            if (n === 1) {
                const win = group.wins[0];
                btn.title = win.window_name;
                btn.addEventListener('click', () => {
                    if (win.minimized) win.restore();
                    else if (win.element.classList.contains('focused')) win.minimize();
                    else WindowManager.focus(win);
                });
            } else {
                btn.title = `${group.wins[0].app_class} (${n})`;
                btn.addEventListener('click', e => {
                    e.stopPropagation();
                    const vis = group.wins.some(w => !w.minimized);
                    group.wins.forEach(w => vis ? w.minimize() : w.restore());
                });
            }

            btn.addEventListener('contextmenu', e => {
                e.preventDefault();
                e.stopPropagation();
                this._showTaskbarCtx(e.clientX, e.clientY, group.wins);
            });

            bar.appendChild(btn);
        });
    },

    _showWinList(anchor, wins) {
        document.querySelectorAll('.wl-popup').forEach(p => p.remove());
        const popup = document.createElement('div');
        popup.className = 'wl-popup';

        wins.forEach(win => {
            const row = document.createElement('div');
            row.className = 'wl-row';
            const addr    = win.getCurrentAddress?.() ?? win.window_name;
            row.innerHTML =
                `<span class="wl-icon">${win.icon_name}</span>` +
                `<span class="wl-text">${addr}</span>`;
            row.addEventListener('click', e => {
                e.stopPropagation();
                WindowManager.focus(win);
                popup.remove();
            });
            popup.appendChild(row);
        });

        const r = anchor.getBoundingClientRect();
        popup.style.left   = r.left + 'px';
        popup.style.bottom = (window.innerHeight - r.top + 4) + 'px';
        document.body.appendChild(popup);
        const pr = popup.getBoundingClientRect();
        if (pr.right > window.innerWidth) {
            popup.style.left = Math.max(0, r.right - pr.width) + 'px';
        }

        let closeTimer;
        const schedClose = () => { closeTimer = setTimeout(() => popup.remove(), 200); };
        const cancelClose = () => clearTimeout(closeTimer);

        anchor.addEventListener('mouseleave', schedClose);
        popup.addEventListener('mouseenter', cancelClose);
        popup.addEventListener('mouseleave', schedClose);
    },

    _showTaskbarCtx(x, y, wins) {
        this._closeCtx();
        const menu = document.createElement('div');
        menu.className = 'ctx-menu';
        menu.style.bottom = (window.innerHeight - y + 2) + 'px';
        menu.style.left   = x + 'px';

        const mkItem = (label, fn) => {
            const el = document.createElement('div');
            el.className   = 'ctx-item';
            el.textContent = label;
            el.addEventListener('click', e => { e.stopPropagation(); fn(); this._closeCtx(); });
            return el;
        };

        menu.appendChild(mkItem('Maximize all', () => wins.forEach(w => { if (!w.fullscreen) w.toggleMaximize(); })));
        menu.appendChild(mkItem('Close all',    () => [...wins].forEach(w => w.close())));

        document.body.appendChild(menu);
        this._ctxMenu = menu;
        const rect = menu.getBoundingClientRect();
        if (rect.right > window.innerWidth) {
            menu.style.left = Math.max(0, x - rect.width) + 'px';
        }
    },

    _closeCtx() {
        this._ctxMenu?.remove();
        this._ctxMenu = null;
    },

    _buildStartMenu() {
        const container = document.getElementById('start-buttons');
        const entries = [
            { icon: 'ℹ️',  label: 'About',   fn: () => { this.closeStartMenu(); WindowManager.open(InfoWindow,     {}); } },
            { icon: '<img class="ico-img" src="graphical-berries/folder.png" alt="">',  label: 'Files',   fn: () => { this.closeStartMenu(); WindowManager.open(ExplorerWindow, {}); } },
            { icon: '<img class="ico-img" src="graphical-berries/default.ico" alt="">', label: 'Browser', fn: () => { this.closeStartMenu(); WindowManager.open(BrowserWindow,  {}); } },
            { icon: '📹',  label: 'Video Player (Disabled)',  fn: null, disabled: true },
            { icon: '⏻',   label: 'Reboot',  fn: () => { this.closeStartMenu(); setTimeout(() => location.reload(), 300); } },
        ];
        entries.forEach(e => {
            const btn = document.createElement('div');
            btn.className = 'sm-btn' + (e.disabled ? ' sm-btn--disabled' : '');
            btn.innerHTML =
                `<span class="sm-lbl">${e.label}</span>` +
                `<span class="sm-icon">${e.icon}</span>`;
            if (!e.disabled) btn.addEventListener('click', e.fn);
            container.appendChild(btn);
        });
    },

    toggleStartMenu() {
        this._startOpen = !this._startOpen;
        document.getElementById('start-menu').classList.toggle('hidden', !this._startOpen);
        document.getElementById('start-btn').classList.toggle('active',   this._startOpen);
    },

    closeStartMenu() {
        if (!this._startOpen) return;
        this._startOpen = false;
        document.getElementById('start-menu').classList.add('hidden');
        document.getElementById('start-btn').classList.remove('active');
    },
};
