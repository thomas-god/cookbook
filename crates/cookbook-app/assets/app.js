const debounceTimers = new Map();

const cookbook = (globalThis.cookbook ??= {});

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
