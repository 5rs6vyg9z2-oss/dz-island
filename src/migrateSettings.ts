const oldPrefix = "nectar-";
const newPrefix = "dz-island-";

try {
  for (const key of Object.keys(localStorage)) {
    if (!key.startsWith(oldPrefix)) continue;
    const value = localStorage.getItem(key);
    const newKey = newPrefix + key.slice(oldPrefix.length);
    if (value !== null && localStorage.getItem(newKey) === null) {
      localStorage.setItem(newKey, value);
    }
  }
} catch {
  // Settings from the native profile remain available if storage is disabled.
}
