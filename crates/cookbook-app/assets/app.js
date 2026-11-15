const debounceTimers = new Map();

const cookbook = (globalThis.cookbook ??= {});

const closeMenus = (except) => {
    for (const menu of document.querySelectorAll("details[data-menu][open]")) {
        if (menu !== except) {
            menu.open = false;
        }
    }
};

document.addEventListener("click", (event) => {
    if (event.target instanceof Node) {
        const menu = event.target.closest("details[data-menu]");
        closeMenus(menu);
    }
});

document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element)) {
        return;
    }

    const opener = event.target.closest("[data-dialog-open]");
    if (opener === null) {
        return;
    }

    const dialog = document.getElementById(opener.getAttribute("data-dialog-open") ?? "");
    if (!(dialog instanceof HTMLDialogElement)) {
        return;
    }

    event.preventDefault();
    if (!dialog.open) {
        dialog.showModal();
    }
});

document.addEventListener("click", (event) => {
    const dialog = event.target;
    if (!(dialog instanceof HTMLDialogElement) || !dialog.open) {
        return;
    }

    const bounds = dialog.getBoundingClientRect();
    const outside =
        event.clientX < bounds.left ||
        event.clientX > bounds.right ||
        event.clientY < bounds.top ||
        event.clientY > bounds.bottom;

    if (outside) {
        dialog.close();
    }
});

document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
        closeMenus();
    }
});

cookbook.debounce = (key, delayMs, callback) => {
    if (key == null) {
        return;
    }

    const pending = debounceTimers.get(key);
    if (pending !== undefined) {
        clearTimeout(pending);
    }

    debounceTimers.set(
        key,
        setTimeout(() => {
            debounceTimers.delete(key);
            callback();
        }, delayMs),
    );
};
