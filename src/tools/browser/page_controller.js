/* OpenZ Native In-Page Browser Controller (flutterff-rs + page-agent unified engine) */
(() => {
  if (window.__openzBrowser && window.__openzBrowser.__version === 1) {
    return;
  }

  const MAX_LOGS = 200;
  const MAX_NETWORK = 150;
  const OVERLAY_ATTR = 'data-openz-internal';

  const state = {
    elements: new Map(),
    elementMeta: new Map(),
    scrollables: new Map(),
    prevSnapshotMap: new Map(),
    logs: [],
    network: [],
    overlayEnabled: false,
    pausedByHuman: false,
    resumeWaiters: [],
  };

  function safeString(val, maxLen = 500) {
    try {
      const s = typeof val === 'string' ? val : JSON.stringify(val);
      if (!s) return '';
      return s.length > maxLen ? s.slice(0, maxLen) + '…' : s;
    } catch (_) {
      const fallback = String(val);
      return fallback.length > maxLen ? fallback.slice(0, maxLen) + '…' : fallback;
    }
  }

  function pushLog(level, args) {
    state.logs.push({
      ts: Date.now(),
      level,
      message: Array.from(args).map((a) => safeString(a, 400)).join(' '),
    });
    if (state.logs.length > MAX_LOGS) {
      state.logs.splice(0, state.logs.length - MAX_LOGS);
    }
  }

  // Hook console + uncaught errors once
  if (!window.__openzConsoleHooked) {
    window.__openzConsoleHooked = true;
    ['log', 'info', 'warn', 'error'].forEach((level) => {
      const orig = console[level];
      if (typeof orig === 'function') {
        console[level] = function (...args) {
          try {
            pushLog(level, args);
          } catch (_) {}
          return orig.apply(this, args);
        };
      }
    });
    window.addEventListener('error', (ev) => {
      pushLog('pageerror', [ev.message || 'Error', ev.filename || '', ev.lineno || 0]);
    });
    window.addEventListener('unhandledrejection', (ev) => {
      pushLog('unhandledrejection', [ev.reason || 'Promise rejected']);
    });
  }

  function pushNetwork(entry) {
    state.network.push(entry);
    if (state.network.length > MAX_NETWORK) {
      state.network.splice(0, state.network.length - MAX_NETWORK);
    }
  }

  // Hook fetch + XHR once
  if (!window.__openzNetworkHooked) {
    window.__openzNetworkHooked = true;
    if (typeof window.fetch === 'function') {
      const origFetch = window.fetch;
      window.fetch = async function (input, init) {
        const started = performance.now();
        const method = (init && init.method) || (input && input.method) || 'GET';
        const url = typeof input === 'string' ? input : (input && input.url) || String(input);
        try {
          const resp = await origFetch.apply(this, arguments);
          pushNetwork({
            kind: 'fetch',
            method: String(method).toUpperCase(),
            url: safeString(url, 300),
            status: resp.status,
            ok: resp.ok,
            duration_ms: Math.round(performance.now() - started),
          });
          return resp;
        } catch (err) {
          pushNetwork({
            kind: 'fetch',
            method: String(method).toUpperCase(),
            url: safeString(url, 300),
            status: 0,
            ok: false,
            error: safeString(err && err.message ? err.message : err, 160),
            duration_ms: Math.round(performance.now() - started),
          });
          throw err;
        }
      };
    }

    if (typeof XMLHttpRequest !== 'undefined') {
      const origOpen = XMLHttpRequest.prototype.open;
      const origSend = XMLHttpRequest.prototype.send;
      XMLHttpRequest.prototype.open = function (method, url) {
        this.__openzMeta = {
          method: String(method || 'GET').toUpperCase(),
          url: safeString(url, 300),
        };
        return origOpen.apply(this, arguments);
      };
      XMLHttpRequest.prototype.send = function () {
        const meta = this.__openzMeta;
        const started = performance.now();
        if (meta) {
          this.addEventListener('loadend', () => {
            pushNetwork({
              kind: 'xhr',
              method: meta.method,
              url: meta.url,
              status: this.status || 0,
              ok: this.status >= 200 && this.status < 400,
              duration_ms: Math.round(performance.now() - started),
            });
          });
        }
        return origSend.apply(this, arguments);
      };
    }
  }

  function isInternalNode(node) {
    if (!node || node.nodeType !== 1) return false;
    return !!(node.hasAttribute(OVERLAY_ATTR) || (node.closest && node.closest(`[${OVERLAY_ATTR}]`)));
  }

  function isSensitiveInput(el) {
    if (!el || !el.tagName) return false;
    const type = String(el.getAttribute('type') || '').toLowerCase();
    if (type === 'password') return true;
    const auto = String(el.getAttribute('autocomplete') || '').toLowerCase();
    return (
      auto.includes('password') ||
      auto.includes('cc-number') ||
      auto.includes('cc-csc')
    );
  }

  function collapseWhitespace(input) {
    const str = String(input || '');
    let out = '';
    let prevSpace = false;
    for (let i = 0; i < str.length; i++) {
      const ch = str[i];
      const isSpace = ch === ' ' || ch === '\n' || ch === '\r' || ch === '\t' || ch === '\f';
      if (isSpace) {
        if (!prevSpace) {
          out += ' ';
          prevSpace = true;
        }
      } else {
        out += ch;
        prevSpace = false;
      }
    }
    return out.trim();
  }

  function cleanText(raw, maxLen = 140) {
    const collapsed = collapseWhitespace(raw);
    if (collapsed.length <= maxLen) return collapsed;
    return collapsed.slice(0, maxLen) + '…';
  }

  function inferRole(el) {
    const explicit = el.getAttribute && el.getAttribute('role');
    if (explicit) return explicit.toLowerCase();
    const tag = el.tagName ? el.tagName.toLowerCase() : '';
    if (tag === 'a' && el.hasAttribute('href')) return 'link';
    if (tag === 'button' || tag === 'summary') return 'button';
    if (tag === 'select') return 'combobox';
    if (tag === 'textarea') return 'textbox';
    if (tag === 'flt-semantics') return 'semantics';
    if (tag === 'input') {
      const t = String(el.getAttribute('type') || 'text').toLowerCase();
      if (t === 'submit' || t === 'button' || t === 'reset') return 'button';
      if (t === 'checkbox') return 'checkbox';
      if (t === 'radio') return 'radio';
      if (t === 'range') return 'slider';
      if (t === 'search') return 'searchbox';
      return 'textbox';
    }
    if (el.isContentEditable) return 'textbox';
    return tag || 'element';
  }

  function isInteractiveElement(el, style) {
    const tag = el.tagName ? el.tagName.toLowerCase() : '';
    if (!tag || tag === 'html' || tag === 'body' || tag === 'head' || tag === 'script' || tag === 'style') {
      return false;
    }
    if (
      tag === 'button' ||
      tag === 'select' ||
      tag === 'textarea' ||
      tag === 'summary' ||
      tag === 'flt-semantics'
    ) {
      return true;
    }
    if (tag === 'a' && (el.hasAttribute('href') || el.hasAttribute('onclick'))) {
      return true;
    }
    if (tag === 'input') {
      const t = String(el.getAttribute('type') || 'text').toLowerCase();
      return t !== 'hidden';
    }
    if (el.isContentEditable) {
      const ce = el.getAttribute('contenteditable');
      if (ce === '' || ce === 'true' || ce === 'plaintext-only') return true;
    }
    const role = String(el.getAttribute('role') || '').toLowerCase();
    if (
      [
        'button',
        'link',
        'textbox',
        'searchbox',
        'checkbox',
        'radio',
        'switch',
        'combobox',
        'listbox',
        'menuitem',
        'tab',
        'option',
        'slider',
        'spinbutton',
        'treeitem',
      ].includes(role)
    ) {
      return true;
    }
    if (el.hasAttribute('onclick') || el.hasAttribute('data-testid') && style.cursor === 'pointer') {
      return true;
    }
    const tabIdx = el.getAttribute('tabindex');
    if (tabIdx !== null && Number(tabIdx) >= 0) {
      return true;
    }
    if (style && style.cursor === 'pointer' && el.children.length <= 4) {
      const parentEl = el.parentElement;
      const win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
      const parentPointer =
        parentEl && win.getComputedStyle(parentEl).cursor === 'pointer';
      if (!parentPointer) {
        const txt = cleanText(el.innerText || el.textContent || '', 80);
        if (txt.length > 0 && txt.length <= 80) {
          return true;
        }
      }
    }
    return false;
  }

  function elementLabel(el) {
    const aria = el.getAttribute('aria-label');
    if (aria && aria.trim()) return cleanText(aria, 120);
    const labelledBy = el.getAttribute('aria-labelledby');
    if (labelledBy && el.ownerDocument) {
      const parts = collapseWhitespace(labelledBy)
        .split(' ')
        .map((id) => {
          const ref = el.ownerDocument.getElementById(id);
          return ref ? cleanText(ref.innerText || ref.textContent || '', 80) : '';
        })
        .filter(Boolean);
      if (parts.length) return parts.join(' ');
    }
    if (el.id && el.ownerDocument) {
      try {
        const lbl = el.ownerDocument.querySelector(`label[for="${CSS.escape(el.id)}"]`);
        if (lbl) {
          const t = cleanText(lbl.innerText || lbl.textContent || '', 100);
          if (t) return t;
        }
      } catch (_) {}
    }
    const closestLabel = el.closest && el.closest('label');
    if (closestLabel) {
      const t = cleanText(closestLabel.innerText || closestLabel.textContent || '', 100);
      if (t) return t;
    }
    const placeholder = el.getAttribute('placeholder');
    if (placeholder && placeholder.trim()) return cleanText(placeholder, 100);
    const title = el.getAttribute('title');
    if (title && title.trim()) return cleanText(title, 100);
    const alt = el.getAttribute('alt');
    if (alt && alt.trim()) return cleanText(alt, 100);
    const inner = cleanText(el.innerText || el.textContent || '', 120);
    if (inner) return inner;
    const nameAttr = el.getAttribute('name');
    if (nameAttr && nameAttr.trim()) return cleanText(nameAttr, 80);
    return '';
  }

  function collectSceneNodes(rootDoc, offsetX, offsetY, outInteractive, outScrollables, outOverflow) {
    if (!rootDoc || !rootDoc.documentElement) return;
    const vw = window.innerWidth || document.documentElement.clientWidth || 1280;
    const vh = window.innerHeight || document.documentElement.clientHeight || 800;

    function visit(root) {
      const walker = (root.ownerDocument || rootDoc).createTreeWalker(root, NodeFilter.SHOW_ELEMENT);
      let el = root.nodeType === 1 ? root : walker.nextNode();
      while (el) {
        if (!isInternalNode(el)) {
          const win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
          const style = win.getComputedStyle(el);
          if (style.display !== 'none' && style.visibility !== 'hidden' && Number(style.opacity || '1') > 0.01) {
            const rect = el.getBoundingClientRect();
            const absX = Math.round(rect.left + offsetX);
            const absY = Math.round(rect.top + offsetY);
            const w = Math.round(rect.width);
            const h = Math.round(rect.height);

            if (w > 1 && h > 1) {
              // Scrollable inner container check
              const oy = style.overflowY;
              if (
                el !== rootDoc.body &&
                el !== rootDoc.documentElement &&
                (oy === 'auto' || oy === 'scroll' || oy === 'overlay') &&
                el.scrollHeight > el.clientHeight + 24
              ) {
                outScrollables.push({
                  el,
                  tag: el.tagName.toLowerCase(),
                  id: el.id || null,
                  px_above: Math.round(el.scrollTop),
                  px_below: Math.max(0, Math.round(el.scrollHeight - el.clientHeight - el.scrollTop)),
                  rect: { x: absX, y: absY, w, h },
                });
              }

              // Horizontal overflow warning (flutterff-rs RenderFlex overflow insight)
              if (el.scrollWidth > el.clientWidth + 16 && style.overflowX === 'visible' && absX + w > vw + 16) {
                if (outOverflow.length < 10) {
                  outOverflow.push({
                    tag: el.tagName.toLowerCase(),
                    id: el.id || null,
                    overflow_px: Math.round(absX + w - vw),
                  });
                }
              }

              if (isInteractiveElement(el, style)) {
                outInteractive.push({ el, style, absX, absY, w, h, vw, vh });
              }
            }

            if (el.shadowRoot) {
              visit(el.shadowRoot);
            }
            if (el.tagName && el.tagName.toLowerCase() === 'iframe') {
              try {
                if (el.contentDocument) {
                  collectSceneNodes(
                    el.contentDocument,
                    absX,
                    absY,
                    outInteractive,
                    outScrollables,
                    outOverflow
                  );
                }
              } catch (_) {}
            }
          }
        }
        el = walker.nextNode();
      }
    }

    visit(rootDoc.documentElement);
  }

  function buildElementRecord(index, raw) {
    const { el, absX, absY, w, h, vw, vh } = raw;
    const tag = el.tagName.toLowerCase();
    const role = inferRole(el);
    const label = elementLabel(el);
    const disabled = !!(el.disabled || el.getAttribute('aria-disabled') === 'true');
    const inViewport = absX + w > 0 && absY + h > 0 && absX < vw && absY < vh;

    let value = null;
    if (isSensitiveInput(el)) {
      value = el.value ? '••••••••' : '';
    } else if ('value' in el && typeof el.value === 'string' && tag !== 'button') {
      value = cleanText(el.value, 120);
    } else if (el.isContentEditable) {
      value = cleanText(el.innerText || el.textContent || '', 120);
    }

    let checked = null;
    if (tag === 'input' && (el.type === 'checkbox' || el.type === 'radio')) {
      checked = !!el.checked;
    } else if (el.hasAttribute('aria-checked')) {
      checked = el.getAttribute('aria-checked') === 'true';
    }

    let options = null;
    if (tag === 'select' && el.options) {
      options = Array.from(el.options)
        .slice(0, 25)
        .map((opt) => ({
          value: cleanText(opt.value, 60),
          label: cleanText(opt.text, 60),
          selected: !!opt.selected,
        }));
    }

    const href = tag === 'a' && el.href ? safeString(el.href, 200) : null;
    const rawHrefAttr = tag === 'a' ? cleanText(el.getAttribute('href') || '', 64) : '';
    const inputType = tag === 'input' ? String(el.getAttribute('type') || 'text').toLowerCase() : null;

    const parts = [`[${index}] <${tag}>`];
    if (role && role !== tag) parts.push(`role=${role}`);
    if (inputType) parts.push(`type=${inputType}`);
    if (label) parts.push(`"${label}"`);
    if (
      rawHrefAttr &&
      !rawHrefAttr.startsWith('#') &&
      !rawHrefAttr.toLowerCase().startsWith('javascript:') &&
      rawHrefAttr !== label
    ) {
      parts.push(`href="${rawHrefAttr}"`);
    }
    if (value !== null && value !== '' && value !== label) parts.push(`val="${value}"`);
    if (checked !== null) parts.push(checked ? '[checked]' : '[unchecked]');
    if (disabled) parts.push('[disabled]');
    if (options && options.length) {
      const optSummary = options
        .slice(0, 10)
        .map((o) => `${o.value}:"${o.label}"${o.selected ? '*' : ''}`)
        .join(', ');
      parts.push(`options=[${optSummary}]`);
    }
    if (!inViewport) parts.push('@offscreen');
    const line = parts.join(' ');

    const signature = `${tag}|${role}|${label}|${value || ''}|${checked}|${disabled}`;
    return {
      index,
      tag,
      role,
      type: inputType,
      label,
      value,
      checked,
      disabled,
      href,
      options,
      in_viewport: inViewport,
      rect: { x: absX, y: absY, w, h },
      line,
      signature,
    };
  }

  function computeSnapshot(opts = {}) {
    const query = opts.query ? String(opts.query).toLowerCase().trim() : '';
    const roleFilter = opts.role ? String(opts.role).toLowerCase().trim() : '';
    const maxElements = Number(opts.max_elements || 180);
    const viewportOnly = !!opts.viewport_only;

    const rawInteractive = [];
    const rawScrollables = [];
    const overflows = [];
    collectSceneNodes(document, 0, 0, rawInteractive, rawScrollables, overflows);

    state.elements.clear();
    state.elementMeta.clear();
    state.scrollables.clear();

    const scrollSummary = rawScrollables.slice(0, 15).map((sc, i) => {
      const id = `scroll#${i}`;
      state.scrollables.set(id, sc.el);
      state.scrollables.set(String(i), sc.el);
      return {
        scroll_id: id,
        tag: sc.tag,
        dom_id: sc.id,
        px_above: sc.px_above,
        px_below: sc.px_below,
        rect: sc.rect,
      };
    });

    const items = [];
    const nextMap = new Map();

    for (let i = 0; i < rawInteractive.length; i++) {
      const raw = rawInteractive[i];
      if (viewportOnly) {
        const inVp =
          raw.absX + raw.w > 0 &&
          raw.absY + raw.h > 0 &&
          raw.absX < raw.vw &&
          raw.absY < raw.vh;
        if (!inVp) continue;
      }
      const probe = buildElementRecord(items.length, raw);

      if (roleFilter && probe.role !== roleFilter && probe.tag !== roleFilter) {
        continue;
      }
      if (query) {
        const hay = `${probe.tag} ${probe.role} ${probe.label} ${probe.value || ''} ${probe.href || ''}`.toLowerCase();
        if (!hay.includes(query)) continue;
      }

      const idx = items.length;
      state.elements.set(idx, raw.el);
      state.elementMeta.set(idx, {
        tag: probe.tag,
        role: probe.role,
        label: probe.label,
        query,
        roleFilter,
      });
      nextMap.set(idx, probe.line);
      items.push(probe);
      if (items.length >= maxElements) break;
    }

    // Compute compact differential ui_diff against previous snapshot
    const diffLines = [];
    const prevValues = new Set(state.prevSnapshotMap.values());
    const nextValues = new Set(nextMap.values());

    for (const rec of items) {
      if (!prevValues.has(rec.line)) {
        const prevAtIdx = state.prevSnapshotMap.get(rec.index);
        if (prevAtIdx && prevAtIdx !== rec.line) {
          diffLines.push(`[~] ${rec.line}`);
        } else {
          diffLines.push(`[+] ${rec.line}`);
        }
      }
    }
    let removedCount = 0;
    for (const oldLine of prevValues) {
      if (!nextValues.has(oldLine)) {
        removedCount++;
        if (removedCount <= 15) {
          diffLines.push(`[-] ${oldLine}`);
        }
      }
    }
    if (removedCount > 15) {
      diffLines.push(`[-] …and ${removedCount - 15} more removed elements`);
    }
    state.prevSnapshotMap = nextMap;

    const docEl = document.documentElement || document.body;
    const scrollY = Math.round(window.scrollY || window.pageYOffset || 0);
    const viewportH = window.innerHeight || (docEl && docEl.clientHeight) || 800;
    const viewportW = window.innerWidth || (docEl && docEl.clientWidth) || 1280;
    const totalH = (docEl && docEl.scrollHeight) || viewportH;
    const pxBelow = Math.max(0, Math.round(totalH - viewportH - scrollY));

    const mediaSummary = Array.from(document.querySelectorAll('video, audio'))
      .slice(0, 4)
      .map((m) => ({
        tag: m.tagName.toLowerCase(),
        paused: !!m.paused,
        muted: !!m.muted,
        ended: !!m.ended,
        current_time: Number.isFinite(m.currentTime) ? Math.round(m.currentTime * 10) / 10 : 0,
        duration: Number.isFinite(m.duration) ? Math.round(m.duration * 10) / 10 : null,
        ready_state: m.readyState || 0,
      }));

    if (state.overlayEnabled) {
      renderInspectOverlay(items);
    }

    return {
      url: String(location.href || ''),
      title: String(document.title || ''),
      ready_state: String(document.readyState || ''),
      viewport: {
        width: viewportW,
        height: viewportH,
        px_above: scrollY,
        px_below: pxBelow,
      },
      element_count: items.length,
      tree_text: items.map((r) => r.line).join('\n'),
      ui_diff: diffLines.length ? diffLines.join('\n') : '(no interactive DOM changes)',
      elements: opts.include_records ? items : undefined,
      scrollable_containers: scrollSummary,
      media: mediaSummary.length ? mediaSummary : undefined,
      overflow_warnings: overflows,
    };
  }

  function showActionPulse(x, y, labelText) {
    try {
      const ring = document.createElement('div');
      ring.setAttribute(OVERLAY_ATTR, 'pulse');
      ring.style.cssText = [
        'position:fixed',
        `left:${Math.round(x - 14)}px`,
        `top:${Math.round(y - 14)}px`,
        'width:28px',
        'height:28px',
        'border:3px solid #FF4500',
        'background:rgba(255,69,0,0.22)',
        'border-radius:999px',
        'pointer-events:none',
        'z-index:2147483646',
        'transition:transform 0.35s ease-out, opacity 0.35s ease-out',
        'transform:scale(0.65)',
        'opacity:1',
      ].join(';');
      if (labelText) {
        const badge = document.createElement('span');
        badge.setAttribute(OVERLAY_ATTR, 'pulse-label');
        badge.textContent = cleanText(labelText, 28);
        badge.style.cssText =
          'position:absolute;left:32px;top:2px;background:#111827;color:#FFEDD5;border:1px solid #FF4500;font:600 11px system-ui,sans-serif;padding:2px 6px;border-radius:4px;white-space:nowrap;';
        ring.appendChild(badge);
      }
      document.documentElement.appendChild(ring);
      requestAnimationFrame(() => {
        ring.style.transform = 'scale(1.35)';
        ring.style.opacity = '0';
      });
      setTimeout(() => {
        if (ring.parentNode) ring.parentNode.removeChild(ring);
      }, 400);
    } catch (_) {}
  }

  function clearInspectOverlay() {
    const existing = document.querySelectorAll(`[${OVERLAY_ATTR}="som-root"]`);
    existing.forEach((n) => n.parentNode && n.parentNode.removeChild(n));
  }

  function renderInspectOverlay(items) {
    clearInspectOverlay();
    if (!items || !items.length) return;
    const root = document.createElement('div');
    root.setAttribute(OVERLAY_ATTR, 'som-root');
    root.style.cssText =
      'position:fixed;inset:0;pointer-events:none;z-index:2147483645;overflow:hidden;';
    for (const item of items) {
      if (!item.in_viewport) continue;
      const box = document.createElement('div');
      box.setAttribute(OVERLAY_ATTR, 'som-box');
      box.style.cssText = [
        'position:fixed',
        `left:${item.rect.x}px`,
        `top:${item.rect.y}px`,
        `width:${Math.max(item.rect.w, 8)}px`,
        `height:${Math.max(item.rect.h, 8)}px`,
        'border:1.5px solid #FF4500',
        'background:rgba(255,69,0,0.06)',
        'box-sizing:border-box',
        'pointer-events:none',
      ].join(';');
      const tag = document.createElement('span');
      tag.setAttribute(OVERLAY_ATTR, 'som-tag');
      tag.textContent = `[${item.index}]`;
      tag.style.cssText =
        'position:absolute;left:-1px;top:-16px;background:#FF4500;color:#fff;font:700 10px monospace;padding:1px 4px;border-radius:3px;line-height:13px;';
      box.appendChild(tag);
      root.appendChild(box);
    }
    document.documentElement.appendChild(root);
  }

  function updateStatusBanner(statusText, showBanner) {
    let bar = document.querySelector(`[${OVERLAY_ATTR}="status-bar"]`);
    if (!showBanner) {
      if (bar && bar.parentNode) bar.parentNode.removeChild(bar);
      return { paused: state.pausedByHuman };
    }
    if (!bar) {
      bar = document.createElement('div');
      bar.setAttribute(OVERLAY_ATTR, 'status-bar');
      bar.style.cssText = [
        'position:fixed',
        'right:16px',
        'bottom:16px',
        'display:flex',
        'align-items:center',
        'gap:10px',
        'background:rgba(15,23,42,0.92)',
        'color:#F8FAFC',
        'border:1px solid #FF4500',
        'border-radius:999px',
        'padding:6px 14px',
        'font:500 12px system-ui,sans-serif',
        'box-shadow:0 8px 24px rgba(0,0,0,0.35)',
        'z-index:2147483647',
      ].join(';');
      const dot = document.createElement('span');
      dot.setAttribute(OVERLAY_ATTR, 'status-dot');
      dot.style.cssText = 'width:8px;height:8px;border-radius:999px;background:#FF4500;display:inline-block;';
      const lbl = document.createElement('span');
      lbl.setAttribute(OVERLAY_ATTR, 'status-label');
      const btn = document.createElement('button');
      btn.setAttribute(OVERLAY_ATTR, 'takeover-btn');
      btn.style.cssText =
        'background:#FF4500;color:#fff;border:none;border-radius:999px;padding:3px 10px;font:600 11px system-ui,sans-serif;cursor:pointer;';
      btn.addEventListener('click', (ev) => {
        ev.preventDefault();
        ev.stopPropagation();
        state.pausedByHuman = !state.pausedByHuman;
        btn.textContent = state.pausedByHuman ? 'Resume Agent' : 'Human Takeover';
        dot.style.background = state.pausedByHuman ? '#F59E0B' : '#FF4500';
        lbl.textContent = state.pausedByHuman
          ? 'Paused — Human in control'
          : 'OpenZ Browser Active';
        if (!state.pausedByHuman) {
          const waiters = state.resumeWaiters.splice(0, state.resumeWaiters.length);
          waiters.forEach((fn) => fn());
        }
      });
      bar.appendChild(dot);
      bar.appendChild(lbl);
      bar.appendChild(btn);
      document.documentElement.appendChild(bar);
    }
    const lbl = bar.querySelector(`[${OVERLAY_ATTR}="status-label"]`);
    const btn = bar.querySelector(`[${OVERLAY_ATTR}="takeover-btn"]`);
    if (lbl) {
      lbl.textContent = state.pausedByHuman
        ? 'Paused — Human in control'
        : cleanText(statusText || 'OpenZ Browser Active', 60);
    }
    if (btn) {
      btn.textContent = state.pausedByHuman ? 'Resume Agent' : 'Human Takeover';
    }
    return { paused: state.pausedByHuman };
  }

  async function waitIfPausedByHuman() {
    if (!state.pausedByHuman) return;
    await new Promise((resolve) => {
      state.resumeWaiters.push(resolve);
    });
  }

  function parseIndexRef(val) {
    if (typeof val === 'number' && Number.isFinite(val)) return Math.trunc(val);
    if (typeof val === 'string') {
      let trimmed = val.trim();
      if (trimmed.startsWith('[') && trimmed.endsWith(']')) {
        trimmed = trimmed.slice(1, -1).trim();
      } else if (trimmed.toLowerCase().startsWith('@e')) {
        trimmed = trimmed.slice(2).trim();
      }
      if (trimmed.length > 0) {
        let allDigits = true;
        for (let i = 0; i < trimmed.length; i++) {
          const code = trimmed.charCodeAt(i);
          if (code < 48 || code > 57) {
            allDigits = false;
            break;
          }
        }
        if (allDigits) return Number(trimmed);
      }
    }
    return null;
  }

  function resolveTargetElement(cmd) {
    if (!state.elements.size) {
      computeSnapshot({});
    }
    const idx =
      parseIndexRef(cmd.index) ??
      parseIndexRef(cmd.ref_id) ??
      parseIndexRef(cmd.selector);
    if (idx !== null && state.elements.has(idx)) {
      const candidate = state.elements.get(idx);
      if (candidate && candidate.isConnected) return candidate;
      const prevMeta = state.elementMeta.get(idx);
      computeSnapshot({
        query: prevMeta ? prevMeta.query : '',
        role: prevMeta ? prevMeta.roleFilter : '',
      });
      if (prevMeta && prevMeta.label) {
        for (const [, el] of state.elements) {
          if (!el || !el.isConnected) continue;
          if (
            el.tagName.toLowerCase() === prevMeta.tag &&
            elementLabel(el) === prevMeta.label
          ) {
            return el;
          }
        }
        for (const [, el] of state.elements) {
          if (!el || !el.isConnected) continue;
          if (elementLabel(el) === prevMeta.label) return el;
        }
      }
      if (state.elements.has(idx)) return state.elements.get(idx);
    }

    if (cmd.selector && typeof cmd.selector === 'string') {
      const sel = cmd.selector.trim();
      try {
        const direct = document.querySelector(sel);
        if (direct) return direct;
      } catch (_) {}
    }

    const needle = String(cmd.label || cmd.target_text || cmd.text_match || (cmd.action === 'click' ? cmd.text || '' : '')).trim().toLowerCase();
    if (needle) {
      let allConnected = true;
      for (const [, el] of state.elements) {
        if (!el || !el.isConnected) {
          allConnected = false;
          break;
        }
      }
      if (!allConnected) computeSnapshot({});
      for (const [, el] of state.elements) {
        if (!el || !el.isConnected) continue;
        if (elementLabel(el).toLowerCase() === needle) return el;
      }
      for (const [, el] of state.elements) {
        if (!el || !el.isConnected) continue;
        if (elementLabel(el).toLowerCase().includes(needle)) return el;
      }
      for (const [, el] of state.elements) {
        if (!el || !el.isConnected) continue;
        const anchor =
          el.tagName && el.tagName.toLowerCase() === 'a'
            ? el
            : el.closest
              ? el.closest('a[href]')
              : null;
        if (!anchor) continue;
        const rawHref = String(anchor.getAttribute('href') || '').trim().toLowerCase();
        const cleanHref = rawHref.startsWith('/') ? rawHref.slice(1) : rawHref;
        if (rawHref === needle || cleanHref === needle || rawHref.endsWith('/' + needle)) {
          return anchor;
        }
      }
    }
    return null;
  }

  function setNativeControlValue(el, text) {
    const tag = el.tagName ? el.tagName.toLowerCase() : '';
    const win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
    if (el.isContentEditable && tag !== 'input' && tag !== 'textarea') {
      el.focus();
      el.textContent = text;
      el.dispatchEvent(new win.InputEvent('input', { bubbles: true, data: text }));
      el.dispatchEvent(new win.Event('change', { bubbles: true }));
      return;
    }
    const proto =
      tag === 'textarea'
        ? win.HTMLTextAreaElement && win.HTMLTextAreaElement.prototype
        : win.HTMLInputElement && win.HTMLInputElement.prototype;
    const desc = proto ? Object.getOwnPropertyDescriptor(proto, 'value') : null;
    if (desc && typeof desc.set === 'function') {
      desc.set.call(el, text);
    } else {
      el.value = text;
    }
    el.dispatchEvent(new win.Event('input', { bubbles: true }));
    el.dispatchEvent(new win.Event('change', { bubbles: true }));
  }

  async function settleFrames(ms = 70) {
    await new Promise((resolve) => requestAnimationFrame(() => setTimeout(resolve, ms)));
  }

  function domToMarkdown(root) {
    if (!root) return '';
    const lines = [];

    function inlineText(node) {
      if (!node) return '';
      if (node.nodeType === 3) {
        const rawTxt = String(node.nodeValue || '');
        const collapsed = collapseWhitespace(rawTxt);
        return collapsed ? ` ${collapsed} ` : '';
      }
      if (node.nodeType !== 1 || isInternalNode(node)) return '';
      const tag = node.tagName.toLowerCase();
      if (tag === 'script' || tag === 'style' || tag === 'noscript' || tag === 'template') return '';
      if (tag === 'br') return '\n';
      if (tag === 'code') {
        const t = cleanText(node.textContent || '', 400);
        return t ? `\`${t}\`` : '';
      }
      if (tag === 'strong' || tag === 'b') {
        const t = Array.from(node.childNodes).map(inlineText).join('').trim();
        return t ? `**${t}**` : '';
      }
      if (tag === 'em' || tag === 'i') {
        const t = Array.from(node.childNodes).map(inlineText).join('').trim();
        return t ? `*${t}*` : '';
      }
      if (tag === 'a') {
        const t = Array.from(node.childNodes).map(inlineText).join('').trim() || cleanText(node.getAttribute('aria-label') || '', 80);
        const href = node.href || node.getAttribute('href') || '';
        if (!t) return '';
        return href && !href.startsWith('javascript:') ? `[${t}](${href})` : t;
      }
      if (tag === 'img') {
        const alt = cleanText(node.getAttribute('alt') || '', 80);
        const src = node.src || node.getAttribute('src') || '';
        return alt && src ? `![${alt}](${src})` : alt;
      }
      return Array.from(node.childNodes).map(inlineText).join('');
    }

    function collapseBlankLines(text) {
      const out = [];
      let blankRun = 0;
      for (const line of String(text || '').split('\n')) {
        if (line.trim() === '') {
          blankRun++;
          if (blankRun <= 1) out.push('');
        } else {
          blankRun = 0;
          out.push(line);
        }
      }
      return out.join('\n').trim();
    }

    function walkBlock(node) {
      if (!node || isInternalNode(node)) return;
      if (node.nodeType === 3) {
        const t = collapseWhitespace(node.nodeValue || '');
        if (t) lines.push(t);
        return;
      }
      if (node.nodeType !== 1) return;
      const tag = node.tagName.toLowerCase();
      if (['script', 'style', 'noscript', 'svg', 'canvas', 'template', 'head'].includes(tag)) return;
      const style = window.getComputedStyle(node);
      if (style.display === 'none' || style.visibility === 'hidden') return;

      if (tag.length === 2 && tag[0] === 'h' && tag[1] >= '1' && tag[1] <= '6') {
        const level = Number(tag[1]);
        const text = cleanText(inlineText(node), 300);
        if (text) lines.push(`\n${'#'.repeat(level)} ${text}\n`);
        return;
      }
      if (tag === 'pre') {
        const raw = String(node.textContent || '').trim();
        if (raw) lines.push(`\n\`\`\`\n${raw}\n\`\`\`\n`);
        return;
      }
      if (tag === 'blockquote') {
        const text = cleanText(inlineText(node), 600);
        if (text) lines.push(`\n> ${text}\n`);
        return;
      }
      if (tag === 'li') {
        const text = cleanText(inlineText(node), 500);
        if (text) lines.push(`- ${text}`);
        return;
      }
      if (tag === 'table') {
        const rows = Array.from(node.querySelectorAll('tr'));
        rows.slice(0, 60).forEach((tr, rIdx) => {
          const cells = Array.from(tr.querySelectorAll('th,td')).map((c) =>
            cleanText(inlineText(c), 120).split('|').join('\\|')
          );
          if (cells.some(Boolean)) {
            lines.push(`| ${cells.join(' | ')} |`);
            if (rIdx === 0) {
              lines.push(`| ${cells.map(() => '---').join(' | ')} |`);
            }
          }
        });
        lines.push('');
        return;
      }
      if (['p', 'section', 'article', 'main', 'header', 'footer', 'nav', 'div', 'ul', 'ol'].includes(tag)) {
        const hasBlockChildren = Array.from(node.children).some((c) =>
          [
            'p', 'div', 'section', 'article', 'ul', 'ol', 'li', 'table', 'pre',
            'blockquote', 'h1', 'h2', 'h3', 'h4', 'h5', 'h6',
          ].includes(c.tagName.toLowerCase())
        );
        if (!hasBlockChildren && (tag === 'p' || tag === 'div')) {
          const text = collapseWhitespace(inlineText(node));
          if (text) lines.push(text);
          return;
        }
        Array.from(node.childNodes).forEach(walkBlock);
        if (tag === 'p' || tag === 'section' || tag === 'article') lines.push('');
        return;
      }
      const fallback = collapseWhitespace(inlineText(node));
      if (fallback) lines.push(fallback);
    }

    const mainRoot =
      root.querySelector('main, article, [role="main"]') ||
      root.body ||
      root.documentElement ||
      root;
    walkBlock(mainRoot);
    const joined = collapseBlankLines(lines.join('\n'));
    if (joined.length >= 40 || mainRoot === root.body) {
      return joined;
    }
    lines.length = 0;
    walkBlock(root.body || root.documentElement || root);
    return collapseBlankLines(lines.join('\n'));
  }

  async function dispatch(cmd = {}) {
    const action = String(cmd.action || 'ui_tree').toLowerCase();

    if (action === 'takeover_mask') {
      if (typeof cmd.paused === 'boolean') {
        state.pausedByHuman = cmd.paused;
        if (!state.pausedByHuman) {
          const waiters = state.resumeWaiters.splice(0, state.resumeWaiters.length);
          waiters.forEach((fn) => fn());
        }
      }
      return updateStatusBanner(cmd.status_text || 'OpenZ Browser Active', cmd.enabled !== false);
    }

    if (action === 'inspect_overlay') {
      state.overlayEnabled = cmd.enabled !== false;
      if (!state.overlayEnabled) {
        clearInspectOverlay();
        return { overlay_enabled: false };
      }
      const snap = computeSnapshot({ max_elements: cmd.max_elements || 180, include_records: true });
      renderInspectOverlay(snap.elements || []);
      return { overlay_enabled: true, element_count: snap.element_count };
    }

    await waitIfPausedByHuman();

    if (action === 'ui_tree' || action === 'snapshot' || action === 'a11y' || action === 'dom') {
      const snap = computeSnapshot({
        query: cmd.query,
        role: cmd.role,
        max_elements: cmd.max_elements || 180,
        viewport_only: !!cmd.viewport_only,
        include_records: !!cmd.include_records,
      });
      if (cmd.diff_only) {
        return {
          url: snap.url,
          title: snap.title,
          viewport: snap.viewport,
          element_count: snap.element_count,
          ui_diff: snap.ui_diff,
          scrollable_containers: snap.scrollable_containers,
        };
      }
      return snap;
    }

    if (action === 'render' || action === 'markdown' || action === 'html' || action === 'text') {
      const fmt = String(cmd.format || (action === 'html' ? 'html' : action === 'text' ? 'text' : 'markdown')).toLowerCase();
      let content = '';
      if (fmt === 'html') {
        content = document.documentElement ? document.documentElement.outerHTML : '';
      } else if (fmt === 'text') {
        content = document.body ? String(document.body.innerText || '').trim() : '';
      } else {
        content = domToMarkdown(document);
      }
      return {
        url: String(location.href || ''),
        final_url: String(location.href || ''),
        title: String(document.title || ''),
        ready_state: String(document.readyState || ''),
        format: fmt,
        output: content,
      };
    }

    if (action === 'click') {
      let cx = typeof cmd.x === 'number' ? cmd.x : null;
      let cy = typeof cmd.y === 'number' ? cmd.y : null;
      let el = null;

      if (cx !== null && cy !== null) {
        el = document.elementFromPoint(cx, cy);
      } else {
        el = resolveTargetElement(cmd);
      }
      if (!el) {
        throw new Error(
          `Click target not found (index=${cmd.index ?? 'none'}, selector=${cmd.selector ?? 'none'}, text=${cmd.text ?? 'none'}). Run browser with action="ui_tree" to inspect current [index] elements.`
        );
      }
      el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      const r = el.getBoundingClientRect();
      cx = r.left + r.width / 2;
      cy = r.top + r.height / 2;
      showActionPulse(cx, cy, `click ${elementLabel(el)}`);

      const clickedLabel = elementLabel(el) || el.tagName.toLowerCase();
      const anchor = el.closest ? el.closest('a[href]') : null;
      const rawHref = anchor ? String(anchor.getAttribute('href') || '').trim() : '';
      const mayNavigate =
        (rawHref !== '' &&
          !rawHref.startsWith('#') &&
          !rawHref.toLowerCase().startsWith('javascript:') &&
          String(anchor.getAttribute('target') || '').toLowerCase() !== '_blank') ||
        Boolean(
          (el.tagName && el.type === 'submit') ||
            (el.closest && el.closest('button[type="submit"], input[type="submit"]'))
        );

      const win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
      ['pointerdown', 'mousedown', 'pointerup', 'mouseup'].forEach((evName) => {
        el.dispatchEvent(
          new win.MouseEvent(evName, {
            bubbles: true,
            cancelable: true,
            clientX: cx,
            clientY: cy,
          })
        );
      });
      if (typeof el.focus === 'function') el.focus();
      if (typeof el.click === 'function') {
        el.click();
      } else {
        el.dispatchEvent(new win.MouseEvent('click', { bubbles: true, cancelable: true, clientX: cx, clientY: cy }));
      }
      await settleFrames(mayNavigate ? 160 : 90);
      const snap = computeSnapshot({});
      return {
        clicked: clickedLabel,
        coords: { x: Math.round(cx), y: Math.round(cy) },
        may_navigate: mayNavigate,
        url: snap.url,
        title: snap.title,
        ui_diff: snap.ui_diff,
      };
    }

    if (action === 'type' || action === 'fill') {
      const el = resolveTargetElement(cmd);
      if (!el) {
        throw new Error(
          `Input target not found (index=${cmd.index ?? 'none'}, selector=${cmd.selector ?? 'none'}). Run browser with action="ui_tree" first.`
        );
      }
      const text = String(cmd.text ?? cmd.value ?? '');
      el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      const r = el.getBoundingClientRect();
      showActionPulse(r.left + r.width / 2, r.top + r.height / 2, 'input');
      if (typeof el.focus === 'function') el.focus();
      setNativeControlValue(el, text);

      if (cmd.submit) {
        const win = (el.ownerDocument && el.ownerDocument.defaultView) || window;
        el.dispatchEvent(new win.KeyboardEvent('keydown', { key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true }));
        el.dispatchEvent(new win.KeyboardEvent('keyup', { key: 'Enter', code: 'Enter', keyCode: 13, bubbles: true }));
        if (el.form && typeof el.form.requestSubmit === 'function') {
          el.form.requestSubmit();
        } else if (el.form) {
          el.form.submit();
        }
      }
      await settleFrames(cmd.submit ? 160 : 80);
      const snap = computeSnapshot({});
      return {
        typed_into: elementLabel(el) || el.tagName.toLowerCase(),
        submitted: !!cmd.submit,
        url: snap.url,
        title: snap.title,
        ui_diff: snap.ui_diff,
      };
    }

    if (action === 'fill_form') {
      const fields = Array.isArray(cmd.fields) ? cmd.fields : [];
      const updated = [];
      for (const field of fields) {
        const target = resolveTargetElement(field);
        if (!target) continue;
        if (target.tagName && target.tagName.toLowerCase() === 'select') {
          const val = String(field.value ?? field.text ?? '');
          const matched = Array.from(target.options).find(
            (o) => o.value === val || o.text.trim().toLowerCase() === val.toLowerCase()
          );
          if (matched) target.value = matched.value;
          target.dispatchEvent(new Event('change', { bubbles: true }));
        } else {
          setNativeControlValue(target, String(field.value ?? field.text ?? ''));
        }
        updated.push(elementLabel(target) || target.tagName.toLowerCase());
      }
      if (cmd.submit && fields.length > 0) {
        const lastEl = resolveTargetElement(fields[fields.length - 1]);
        if (lastEl && lastEl.form) {
          if (typeof lastEl.form.requestSubmit === 'function') lastEl.form.requestSubmit();
          else lastEl.form.submit();
        }
      }
      await settleFrames(cmd.submit ? 160 : 90);
      const snap = computeSnapshot({});
      return {
        updated_fields: updated,
        submitted: !!cmd.submit,
        url: snap.url,
        title: snap.title,
        ui_diff: snap.ui_diff,
      };
    }

    if (action === 'select_option' || action === 'select') {
      const el = resolveTargetElement(cmd);
      if (!el || !el.options) {
        throw new Error('Target <select> element not found.');
      }
      const wanted = String(cmd.value ?? cmd.option ?? cmd.text ?? '').trim();
      const opt = Array.from(el.options).find(
        (o) =>
          o.value === wanted ||
          o.text.trim() === wanted ||
          o.text.trim().toLowerCase().includes(wanted.toLowerCase())
      );
      if (!opt) {
        throw new Error(`Option "${wanted}" not found in <select>.`);
      }
      el.value = opt.value;
      opt.selected = true;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      await settleFrames(70);
      const snap = computeSnapshot({});
      return {
        selected_value: opt.value,
        selected_label: opt.text.trim(),
        ui_diff: snap.ui_diff,
      };
    }

    if (action === 'hover') {
      const el = resolveTargetElement(cmd);
      if (!el) throw new Error('Hover target element not found.');
      el.scrollIntoView({ block: 'center', inline: 'center', behavior: 'instant' });
      const r = el.getBoundingClientRect();
      const cx = r.left + r.width / 2;
      const cy = r.top + r.height / 2;
      showActionPulse(cx, cy, 'hover');
      ['pointerover', 'mouseover', 'pointerenter', 'mouseenter', 'mousemove'].forEach((ev) => {
        el.dispatchEvent(new MouseEvent(ev, { bubbles: true, clientX: cx, clientY: cy }));
      });
      await settleFrames(70);
      const snap = computeSnapshot({});
      return { hovered: elementLabel(el), ui_diff: snap.ui_diff };
    }

    if (action === 'scroll') {
      const direction = String(cmd.direction || 'down').toLowerCase();
      const amount = Number(cmd.amount || Math.round((window.innerHeight || 800) * 0.75));
      let container = window;
      if (cmd.scroll_id && state.scrollables.has(String(cmd.scroll_id))) {
        container = state.scrollables.get(String(cmd.scroll_id));
      } else if (cmd.selector) {
        const found = document.querySelector(String(cmd.selector));
        if (found) container = found;
      }

      const isWin = container === window;
      if (direction === 'top') {
        if (isWin) window.scrollTo(0, 0);
        else container.scrollTop = 0;
      } else if (direction === 'bottom') {
        if (isWin) window.scrollTo(0, document.documentElement.scrollHeight);
        else container.scrollTop = container.scrollHeight;
      } else {
        const dx = direction === 'right' ? amount : direction === 'left' ? -amount : 0;
        const dy = direction === 'up' ? -amount : direction === 'down' ? amount : 0;
        if (isWin) window.scrollBy(dx, dy);
        else {
          container.scrollLeft += dx;
          container.scrollTop += dy;
        }
      }
      await settleFrames(80);
      const snap = computeSnapshot({});
      return {
        direction,
        viewport: snap.viewport,
        scrollable_containers: snap.scrollable_containers,
        ui_diff: snap.ui_diff,
      };
    }

    if (action === 'press_key' || action === 'press') {
      const key = String(cmd.key || cmd.text || 'Enter');
      const target = document.activeElement || document.body || document.documentElement;
      ['keydown', 'keypress', 'keyup'].forEach((ev) => {
        target.dispatchEvent(new KeyboardEvent(ev, { key, code: key, bubbles: true, cancelable: true }));
      });
      await settleFrames(70);
      const snap = computeSnapshot({});
      return { key, ui_diff: snap.ui_diff };
    }

    if (action === 'wait_for' || action === 'wait') {
      const timeoutMs = Math.min(Math.max(Number(cmd.timeout_ms || 8000), 100), 30000);
      const gone = !!cmd.gone;
      const selector = cmd.selector ? String(cmd.selector) : null;
      const textNeedle = cmd.text ? String(cmd.text).toLowerCase() : null;
      if (!selector && !textNeedle) {
        const sleepMs = Math.min(Number(cmd.timeout_ms || 3000), 15000);
        await new Promise((r) => setTimeout(r, sleepMs));
        const snap = computeSnapshot({});
        return { matched: true, slept_ms: sleepMs, url: snap.url, media: snap.media, ui_diff: snap.ui_diff };
      }
      const deadline = Date.now() + timeoutMs;
      while (Date.now() < deadline) {
        let present = false;
        if (selector) {
          try {
            present = !!document.querySelector(selector);
          } catch (_) {}
        } else if (textNeedle) {
          const bodyEl = document.body || document.documentElement;
          const bodyTxt = String((bodyEl && (bodyEl.innerText || '') + ' ' + (bodyEl.textContent || '')) || '').toLowerCase();
          present = bodyTxt.includes(textNeedle);
        }
        if ((present && !gone) || (!present && gone)) {
          const snap = computeSnapshot({});
          return { matched: true, url: snap.url, ui_diff: snap.ui_diff };
        }
        await new Promise((r) => setTimeout(r, 120));
      }
      throw new Error(
        `Timed out after ${timeoutMs}ms waiting for ${gone ? 'disappearance of ' : ''}${selector || cmd.text}`
      );
    }

    if (action === 'eval_js' || action === 'eval' || action === 'evaluate' || action === 'js') {
      const script = String(cmd.script || '');
      const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
      let val;
      try {
        // First try expression form so `document.title` or `1 + 2` returns naturally
        const exprFn = new AsyncFunction(`return (${script});`);
        val = await exprFn();
      } catch (syntaxOrEvalErr) {
        if (syntaxOrEvalErr instanceof SyntaxError) {
          const stmtFn = new AsyncFunction(script);
          val = await stmtFn();
        } else {
          throw syntaxOrEvalErr;
        }
      }
      const output =
        typeof val === 'string'
          ? val
          : val === undefined
            ? 'undefined'
            : JSON.stringify(val);
      return {
        url: String(location.href || ''),
        title: String(document.title || ''),
        output,
        value: val === undefined ? null : val,
      };
    }

    if (action === 'element_rect') {
      const el = resolveTargetElement(cmd);
      if (!el) return { found: false };
      const r = el.getBoundingClientRect();
      return {
        found: true,
        rect: {
          x: Math.max(0, Math.round(r.left + window.scrollX)),
          y: Math.max(0, Math.round(r.top + window.scrollY)),
          vx: Math.max(0, Math.round(r.left)),
          vy: Math.max(0, Math.round(r.top)),
          w: Math.max(1, Math.round(r.width)),
          h: Math.max(1, Math.round(r.height)),
        },
      };
    }

    if (action === 'inject_css') {
      const style = document.createElement('style');
      style.textContent = String(cmd.css || '');
      (document.head || document.documentElement).appendChild(style);
      await settleFrames(50);
      return { injected: true };
    }

    if (action === 'logs') {
      const entries = state.logs.slice(-(cmd.limit || 80));
      if (cmd.clear) state.logs.length = 0;
      return { count: entries.length, logs: entries };
    }

    if (action === 'network') {
      const entries = state.network.slice(-(cmd.limit || 80));
      if (cmd.clear) state.network.length = 0;
      return { count: entries.length, requests: entries };
    }

    throw new Error(`Unsupported page action: ${action}`);
  }

  window.__openzBrowser = {
    __version: 1,
    dispatch,
    computeSnapshot,
    clearInspectOverlay,
  };
})();
