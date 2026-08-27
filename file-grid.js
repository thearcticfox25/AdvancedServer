'use strict';

class FileGrid {
    constructor(container, opts = {}) {
        this._el      = container;
        this._onOpen  = opts.onOpen    || null;
        this._layout  = opts.layout    || 'panel';
        this._fixed   = opts.fixedPath !== undefined ? opts.fixedPath : null;
        this._path    = this._fixed !== null ? this._fixed : '';
        this._selTile = null;

        this._el.classList.add('fg-root', 'fg-' + this._layout);
    }

    get currentPath() { return this._path; }

    navigate(path) {
        if (this._fixed !== null) return;
        this._path = path;
        this.refresh();
    }

    refresh() {
        this._selTile = null;
        this._render(VFS.listDir(this._path));
    }

    _render(items) {
        this._el.innerHTML = '';

        if (!items || items.length === 0) {
            const msg = document.createElement('div');
            msg.className   = 'fg-empty';
            msg.textContent = 'Empty folder';
            this._el.appendChild(msg);
            return;
        }

        items.forEach(item => {
            const tile = document.createElement('div');
            tile.className = 'fg-tile';
            tile.title     = item.label || item.name;

            const icon = document.createElement('div');
            icon.className = 'fg-icon';
            const fallbackEmoji = item.linkUrl ? '🔗' : (item.hasIndex ? '🌐' : '📁');
            if (item.iconPath) {
                const img   = document.createElement('img');
                img.src     = item.iconPath;
                img.alt     = '';
                img.onerror = () => { icon.removeChild(img); icon.textContent = fallbackEmoji; };
                icon.appendChild(img);
            } else {
                icon.textContent = fallbackEmoji;
            }

            const lbl = document.createElement('span');
            lbl.className   = 'fg-label';
            lbl.textContent = item.label || item.name;

            tile.appendChild(icon);
            tile.appendChild(lbl);

            tile.addEventListener('click', e => {
                e.stopPropagation();
                this._selectTile(tile);
            });

            tile.addEventListener('dblclick', e => {
                e.stopPropagation();
                this._open(item);
            });

            this._el.appendChild(tile);
        });

        this._el.addEventListener('click', () => this._selectTile(null));
    }

    _selectTile(tile) {
        if (this._selTile) this._selTile.classList.remove('sel');
        this._selTile = tile;
        if (tile) tile.classList.add('sel');
    }

    _open(item) {
        if (this._onOpen) { this._onOpen(item); return; }
        if (item.linkUrl) {
            window.open(item.linkUrl, '_blank');
        } else if (item.hasIndex && item.path) {
            WindowManager.open(BrowserWindow, {
                url:         VFS.srcToVfsUrl(item.path),
                window_name: item.label || item.name,
            });
        } else {
            const childPath = (this._path ? this._path + '/' : '') + item.name;
            WindowManager.open(ExplorerWindow, { startPath: childPath });
        }
    }
}
