'use strict';

class ExplorerWindow extends AppWindow {
    constructor(opts = {}) {
        super({
            width: 820, height: 560,
            window_name: 'File Manager',
            app_class:   'explorer',
            icon_name:   '<img class="ico-img" src="graphical-berries/folder.png" alt="">',
            ...opts,
        });
        this._path = opts.startPath || '';
        this._grid = null;
    }

    getCurrentAddress() {
        return VFS.toVfsUrl(this._path);
    }

    renderContent() {
        this.contentEl.innerHTML = `
<div class="ex-root">
  <div class="ex-bar">
    <div class="ex-bc" id="ex-bc-${this.id}"></div>
  </div>
  <div class="ex-body">
    <div class="ex-pane-grid" id="ex-grid-${this.id}"></div>
    <div class="ex-sizer"     id="ex-sz-${this.id}"></div>
    <div class="ex-pane-tree" id="ex-tree-${this.id}"></div>
  </div>
</div>`;

        const gridEl = document.getElementById(`ex-grid-${this.id}`);
        this._grid = new FileGrid(gridEl, {
            layout: 'panel',
            onOpen: item => this._onGridOpen(item),
        });

        this._buildTree();
        this._go(this._path, false);
        this._initSizer();
    }

    _go(path, syncTree = true) {
        path = path || '';
        this._path = path;

        const lastName = path.split('/').filter(Boolean).pop() || Config.vfs.root;
        this.setTitle('Files — ' + lastName);

        this._renderBreadcrumb(path);
        this._grid.navigate(path);

        if (syncTree) this._syncTree(path);
    }

    _onGridOpen(item) {
        if (item.linkUrl) {
            window.open(item.linkUrl, '_blank');
            return;
        }
        const childPath = (this._path ? this._path + '/' : '') + item.name;
        if (item.hasIndex && item.path) {
            WindowManager.open(BrowserWindow, {
                url:         VFS.srcToVfsUrl(item.path),
                window_name: item.label || item.name,
            });
        } else {
            this._go(childPath);
        }
    }

    _renderBreadcrumb(path) {
        const bc    = document.getElementById(`ex-bc-${this.id}`);
        bc.innerHTML = '';
        const parts  = path.split('/').filter(Boolean);

        if (!parts.length) {
            bc.appendChild(this._makeCrumb('/', ''));
            return;
        }

        // Render deepest→shallowest, root last — with justify-content:flex-end
        // this pins / to the right wall and expands the path leftward:
        //   current ‹ parent ‹ … ‹ /
        const crumbs = [];
        let built = '';
        parts.forEach(part => {
            built = built ? built + '/' + part : part;
            crumbs.push({ label: part, path: built });
        });

        for (let i = crumbs.length - 1; i >= 0; i--) {
            bc.appendChild(this._makeCrumb(crumbs[i].label, crumbs[i].path));
            const sep = document.createElement('span');
            sep.className   = 'bc-sep';
            sep.textContent = '‹';
            bc.appendChild(sep);
        }

        bc.appendChild(this._makeCrumb('/', ''));
    }

    _makeCrumb(label, path) {
        const c = document.createElement('span');
        c.className   = 'bc-crumb';
        c.textContent = label;
        c.addEventListener('click', () => this._go(path));
        return c;
    }

    _buildTree() {
        const tree = document.getElementById(`ex-tree-${this.id}`);
        tree.innerHTML = '';
        const rootNode = VFS.getNode('');
        if (rootNode) this._treeChildren(tree, rootNode, '', 0);
    }

    _treeChildren(container, node, path, depth) {
        const dirs = Object.entries(node.children || {})
            .filter(([, c]) => c.type === 'dir' && !c.hasIndex);

        dirs.forEach(([name, child]) => {
            const childPath = path ? path + '/' + name : name;
            const hasSubs   = Object.values(child.children || {}).some(c => c.type === 'dir' && !c.hasIndex);

            const row = document.createElement('div');
            row.className = 'ex-tree-row';
            row.style.paddingRight = (10 + depth * 14) + 'px';
            row.dataset.path = childPath;

            const iconSrc = child.iconPath || 'graphical-berries/folder.png';
            row.innerHTML =
                `<span class="tt-name">${child.label || name}</span>` +
                `<img class="tt-icon" src="${iconSrc}" alt="" onerror="this.style.display='none'">` +
                `<span class="tt-toggle">${hasSubs ? '◀' : ' '}</span>`;

            row.addEventListener('click', () => {
                document.querySelectorAll(`#ex-tree-${this.id} .ex-tree-row`)
                    .forEach(r => r.classList.remove('sel'));
                row.classList.add('sel');
                this._go(childPath, false);

                if (!hasSubs) return;
                const isExp = row.dataset.exp === '1';
                const subId = `ex-sub-${this.id}-${childPath.replace(/[^a-z0-9]/gi, '_')}`;
                if (isExp) {
                    row.dataset.exp = '0';
                    row.querySelector('.tt-toggle').textContent = '◀';
                    document.getElementById(subId)?.remove();
                } else {
                    row.dataset.exp = '1';
                    row.querySelector('.tt-toggle').textContent = '▼';
                    const sub = document.createElement('div');
                    sub.id = subId;
                    this._treeChildren(sub, child, childPath, depth + 1);
                    row.insertAdjacentElement('afterend', sub);
                }
            });

            container.appendChild(row);
        });
    }

    _syncTree(path) {
        const rows = document.querySelectorAll(`#ex-tree-${this.id} .ex-tree-row`);
        rows.forEach(r => r.classList.toggle('sel', r.dataset.path === path));
    }

    _initSizer() {
        const sz   = document.getElementById(`ex-sz-${this.id}`);
        const tree = document.getElementById(`ex-tree-${this.id}`);
        if (!sz || !tree) return;
        let on = false, sx, sw;
        sz.addEventListener('mousedown', e => {
            on = true; sx = e.clientX; sw = tree.offsetWidth;
            e.preventDefault();
        });
        document.addEventListener('mousemove', e => {
            if (!on) return;
            tree.style.width = Math.max(80, Math.min(400, sw + (sx - e.clientX))) + 'px';
        });
        document.addEventListener('mouseup', () => { on = false; });
    }
}
