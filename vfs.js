'use strict';

const VFS = {
    _root: { type: 'dir', children: {} },

    async init() {
        try {
            if (Config.vfs.mode === 'json') {
                await this._loadJson();
            } else {
                await this._scanServer();
            }
        } catch (err) {
            console.error('[VFS] init failed:', err);
        }
    },

    async _loadJson() {
        let raw;
        if (window.__vfsManifest) {
            raw = window.__vfsManifest;
        } else {
            const r = await fetch(Config.vfs.jsonPath);
            if (!r.ok) throw new Error(`HTTP ${r.status} — ${Config.vfs.jsonPath}`);
            raw = await r.json();
        }
        this._root = this._normalise(raw, '');
    },

    _normalise(node, relPath) {
        if (!node || node.type !== 'dir') return { type: 'dir', children: {} };
        const out = { type: 'dir', label: node.label, children: {} };

        for (const [name, child] of Object.entries(node.children || {})) {
            if (!child) continue;
            if (child.type === 'link') {
                out.children[name] = {
                    type:     'link',
                    label:    child.label    || name,
                    url:      child.url      || null,
                    iconPath: child.iconPath || null,
                };
                continue;
            }
            if (child.type !== 'dir') continue;
            const childRel = relPath ? `${relPath}/${name}` : name;
            const c        = this._normalise(child, childRel);
            c.label    = child.label    || name;
            c.hasIndex = !!child.hasIndex;
            c.iconPath = child.iconPath || null;
            c.path     = c.hasIndex
                ? `${Config.vfs.root}/${childRel}/index.html`
                : null;
            out.children[name] = c;
        }
        return out;
    },

    async _scanServer() {
        this._root = await this._fetchDir('', 0);
        this._pruneFiles(this._root);
    },

    async _fetchDir(relPath, depth) {
        const node = { type: 'dir', children: {} };
        if (depth > (Config.vfs.maxDepth || 8)) return node;

        const url = Config.vfs.root + (relPath ? `/${relPath}` : '') + '/';
        let html;
        try {
            const r = await fetch(url);
            if (!r.ok) return node;
            html = await r.text();
        } catch { return node; }

        const seen = new Set();
        const re   = /href="([^"#?]+)"/gi;
        let m;
        while ((m = re.exec(html)) !== null) {
            const href = decodeURIComponent(m[1]);
            if (!href || href === '../' || href === './' ||
                href.startsWith('/') || href.startsWith('http') ||
                href.startsWith('?')) continue;
            if (seen.has(href)) continue;
            seen.add(href);

            const isDir = href.endsWith('/');
            const name  = href.replace(/\/$/, '');
            if (!name || name.startsWith('.')) continue;

            const childRel = relPath ? `${relPath}/${name}` : name;

            if (isDir) {
                const child  = await this._fetchDir(childRel, depth + 1);
                child.label  = name;
                const fileKeys = Object.keys(child.children)
                    .filter(k => child.children[k].type === 'file')
                    .map(k => k.toLowerCase());
                child.hasIndex = fileKeys.some(k => k === 'index.html' || k === 'index.htm');
                child.path     = child.hasIndex
                    ? `${Config.vfs.root}/${childRel}/index.html`
                    : null;
                child.iconPath = null;
                node.children[name] = child;
            } else {
                node.children[name] = { type: 'file', label: name };
            }
        }
        return node;
    },

    _pruneFiles(node) {
        if (!node?.children) return;
        for (const [k, c] of Object.entries(node.children)) {
            if (c.type === 'file') { delete node.children[k]; continue; }
            this._pruneFiles(c);
        }
    },

    getNode(path) {
        const parts = (path || '').split('/').filter(Boolean);
        let   cur   = this._root;
        for (const p of parts) {
            if (!cur?.children) return null;
            cur = cur.children[p];
            if (!cur) return null;
        }
        return cur;
    },

    listDir(path) {
        const node = this.getNode(path);
        if (!node || node.type !== 'dir') return [];
        return Object.entries(node.children || {})
            .filter(([, c]) => c.type === 'dir' || c.type === 'link')
            .map(([name, c]) => {
                if (c.type === 'link') return {
                    name,
                    label:    c.label || name,
                    hasIndex: false,
                    linkUrl:  c.url   || null,
                    path:     null,
                    iconPath: c.iconPath || 'graphical-berries/file.bmp',
                };
                return {
                    name,
                    label:    c.label    || name,
                    hasIndex: !!c.hasIndex,
                    linkUrl:  null,
                    path:     c.path     || null,
                    iconPath: c.iconPath || (c.hasIndex
                        ? 'graphical-berries/file.bmp'
                        : 'graphical-berries/folder.png'),
                };
            });
    },

    toVfsUrl(relPath) {
        return 'vfs://' + (relPath || '');
    },

    fromVfsUrl(url) {
        return (typeof url === 'string' && url.startsWith('vfs://'))
            ? url.slice(6)
            : url;
    },

    resolveToSrc(vfsUrl) {
        const rel  = this.fromVfsUrl(vfsUrl);
        const node = this.getNode(rel);
        if (node?.path)     return node.path;
        if (node?.hasIndex) return `${Config.vfs.root}/${rel}/index.html`;
        return null;
    },

    srcToVfsUrl(src) {
        const prefix = Config.vfs.root + '/';
        let   rel    = src.startsWith(prefix) ? src.slice(prefix.length) : src;
        rel = rel.replace(/\/index\.html?$/i, '');
        return 'vfs://' + rel;
    },
};
