// Guard against re-injection
if (!window.__taskerRecording) {
    window.__taskerRecording = true;
    window.__taskerPaused = false;

    const { invoke } = window.__TAURI__.core;

    // Element indexing
    let elementIndex = new WeakMap();
    let currentIndex = 0;

    function getElementIndex(el) {
        if (!elementIndex.has(el)) {
            elementIndex.set(el, currentIndex++);
        }
        return elementIndex.get(el);
    }

    // Get comprehensive selector info
    function getElementInfo(el) {
        if (!el || !el.tagName) return null;
        return {
            index: getElementIndex(el),
            selector: getSelector(el),
            xpath: getXPath(el),
            tagName: el.tagName.toLowerCase(),
            text: el.textContent?.slice(0, 100) || '',
            attributes: {
                id: el.id || null,
                name: el.getAttribute('name'),
                class: el.className || null,
                type: el.getAttribute('type'),
                placeholder: el.getAttribute('placeholder'),
                ariaLabel: el.getAttribute('aria-label'),
                role: el.getAttribute('role'),
                href: el.getAttribute('href'),
            },
            rect: el.getBoundingClientRect().toJSON(),
        };
    }

    function getSelector(el) {
        if (el.id) return '#' + el.id;
        if (el.getAttribute('data-testid')) return `[data-testid="${el.getAttribute('data-testid')}"]`;
        if (el.name) return `[name="${el.name}"]`;
        if (el.className && typeof el.className === 'string') {
            const classes = el.className.trim().split(/\s+/).slice(0, 2).join('.');
            if (classes) return el.tagName.toLowerCase() + '.' + classes;
        }
        return el.tagName.toLowerCase();
    }

    function getXPath(el) {
        if (el.id) return `//*[@id="${el.id}"]`;
        const parts = [];
        while (el && el.nodeType === Node.ELEMENT_NODE) {
            let idx = 1;
            for (let sib = el.previousSibling; sib; sib = sib.previousSibling) {
                if (sib.nodeType === Node.ELEMENT_NODE && sib.tagName === el.tagName) idx++;
            }
            parts.unshift(`${el.tagName.toLowerCase()}[${idx}]`);
            el = el.parentNode;
        }
        return '/' + parts.join('/');
    }

    function emit(actionType, data) {
        if (window.__taskerPaused) return;
        invoke('on_recording_event', {
            actionType,
            data: {
                ...data,
                url: window.location.href,
                timestamp: Date.now(),
            }
        }).catch(err => console.error('Failed to emit recording event:', err));
    }

    // === NAVIGATION ===

    // Track navigate (URL changes)
    const originalPushState = history.pushState;
    const originalReplaceState = history.replaceState;
    history.pushState = function(...args) {
        originalPushState.apply(this, args);
        emit('navigate', { url: window.location.href });
    };
    history.replaceState = function(...args) {
        originalReplaceState.apply(this, args);
        emit('navigate', { url: window.location.href });
    };
    window.addEventListener('popstate', () => {
        emit('go_back', { url: window.location.href });
    });

    // Track search (form submissions with search-like inputs)
    document.addEventListener('submit', (e) => {
        const form = e.target;
        const searchInput = form.querySelector('input[type="search"], input[name*="search"], input[name="q"]');
        if (searchInput) {
            emit('search', { query: searchInput.value });
        }
    }, true);

    // === PAGE INTERACTION ===

    // Track clicks
    document.addEventListener('click', (e) => {
        const el = e.target;
        emit('click', {
            element: getElementInfo(el),
            coordinates: { x: e.clientX, y: e.clientY },
        });
    }, true);

    // Track input (text entry)
    let inputTimeout = null;
    document.addEventListener('input', (e) => {
        const el = e.target;
        if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
            clearTimeout(inputTimeout);
            inputTimeout = setTimeout(() => {
                emit('input', {
                    element: getElementInfo(el),
                    text: el.value,
                });
            }, 500);
        }
    }, true);

    // Track scroll
    let scrollTimeout = null;
    let lastScrollY = window.scrollY;
    document.addEventListener('scroll', () => {
        clearTimeout(scrollTimeout);
        scrollTimeout = setTimeout(() => {
            const direction = window.scrollY > lastScrollY ? 'down' : 'up';
            const amount = Math.abs(window.scrollY - lastScrollY);
            if (amount > 100) { // Only track significant scrolls
                emit('scroll', { direction, amount, scrollY: window.scrollY });
            }
            lastScrollY = window.scrollY;
        }, 200);
    }, true);

    // Track special keys
    document.addEventListener('keydown', (e) => {
        const specialKeys = ['Enter', 'Escape', 'Tab', 'Backspace', 'Delete', 'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'];
        if (specialKeys.includes(e.key) || e.ctrlKey || e.metaKey) {
            emit('send_keys', {
                key: e.key,
                modifiers: {
                    ctrl: e.ctrlKey,
                    alt: e.altKey,
                    shift: e.shiftKey,
                    meta: e.metaKey,
                },
                element: e.target ? getElementInfo(e.target) : null,
            });
        }
    }, true);

    // Track file uploads
    document.addEventListener('change', (e) => {
        const el = e.target;
        if (el.type === 'file' && el.files?.length) {
            emit('upload_file', {
                element: getElementInfo(el),
                files: Array.from(el.files).map(f => ({ name: f.name, size: f.size, type: f.type })),
            });
        }
    }, true);

    // === FORM CONTROLS ===

    // Track dropdown/select changes
    document.addEventListener('change', (e) => {
        const el = e.target;
        if (el.tagName === 'SELECT') {
            const options = Array.from(el.options).map(o => ({ value: o.value, text: o.text, selected: o.selected }));
            emit('select_dropdown', {
                element: getElementInfo(el),
                value: el.value,
                selectedText: el.options[el.selectedIndex]?.text,
                options,
            });
        }
    }, true);

    // === TAB MANAGEMENT ===

    // Track focus/blur for tab switches (limited in scope due to browser security)
    document.addEventListener('visibilitychange', () => {
        if (document.visibilityState === 'visible') {
            emit('switch_tab', { visible: true });
        }
    });

    // Track window.open calls
    const originalOpen = window.open;
    window.open = function(url, target, features) {
        emit('new_tab', { url, target });
        return originalOpen.call(this, url, target, features);
    };

    // Track window close attempts
    window.addEventListener('beforeunload', (e) => {
        emit('close_tab', { url: window.location.href });
    });

    // === CONTENT EXTRACTION ===

    // Track text selection (potential extract action)
    document.addEventListener('mouseup', () => {
        const selection = window.getSelection()?.toString().trim();
        if (selection && selection.length > 10) {
            emit('extract', {
                selectedText: selection.slice(0, 500),
                selectionRange: {
                    startContainer: window.getSelection()?.anchorNode?.parentElement ?
                        getSelector(window.getSelection().anchorNode.parentElement) : null,
                }
            });
        }
    });

    // === CONTEXT MENU (right-click actions) ===
    document.addEventListener('contextmenu', (e) => {
        emit('context_menu', {
            element: getElementInfo(e.target),
            coordinates: { x: e.clientX, y: e.clientY },
        });
    });

    // === INITIALIZATION ===
    emit('script_loaded', {
        title: document.title,
        readyState: document.readyState,
    });

    // Re-emit on full page load
    if (document.readyState !== 'complete') {
        window.addEventListener('load', () => {
            emit('page_loaded', { title: document.title });
        });
    }

    console.log('[Tasker] Recording script initialized');
}
