export type ThemeChoice = 'system' | 'light' | 'dark';

/**
 * Sets the theme on the root element. `system` removes the override so the
 * `prefers-color-scheme` rules in styles.css decide.
 */
export function applyTheme(theme: ThemeChoice): void {
  const root = document.documentElement;
  if (theme === 'system') {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = theme;
  }
}
