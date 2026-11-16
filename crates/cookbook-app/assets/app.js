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

cookbook.downloadForm = async (event) => {
    event.preventDefault();

    const form = event.currentTarget;
    if (!(form instanceof HTMLFormElement) || form.dataset.submitting === "true") {
        return;
    }

    const button = form.querySelector("[data-download-submit]");
    const spinner = form.querySelector("[data-download-spinner]");

    form.dataset.submitting = "true";
    form.setAttribute("aria-busy", "true");
    if (button instanceof HTMLButtonElement) {
        button.disabled = true;
    }
    if (spinner instanceof HTMLElement) {
        spinner.classList.remove("hidden");
    }

    try {
        const response = await fetch(form.action, {
            method: form.method || "POST",
            body: new URLSearchParams(new FormData(form)),
            credentials: "same-origin",
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }

        const blob = await response.blob();
        const disposition = response.headers.get("Content-Disposition") ?? "";
        const match = /filename="([^"]+)"/i.exec(disposition);
        const filename = match?.[1] ?? "recipe.pdf";
        const url = URL.createObjectURL(blob);
        const link = document.createElement("a");

        link.href = url;
        link.download = filename;
        document.body.appendChild(link);
        link.click();
        link.remove();
        setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (error) {
        console.error("download failed", error);
        window.alert(form.getAttribute("data-download-error") ?? "Download failed.");
    } finally {
        delete form.dataset.submitting;
        form.removeAttribute("aria-busy");
        if (button instanceof HTMLButtonElement) {
            button.disabled = false;
        }
        if (spinner instanceof HTMLElement) {
            spinner.classList.add("hidden");
        }
    }
};

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
