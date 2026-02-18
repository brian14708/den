import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useSyncExternalStore,
} from "react";

type Theme = "light" | "dark" | "system";

interface ThemeProviderProps {
  children: React.ReactNode;
  defaultTheme?: Theme;
  storageKey?: string;
}

interface ThemeProviderState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  resolvedTheme: "light" | "dark";
}

const ThemeProviderContext = createContext<ThemeProviderState | undefined>(
  undefined,
);

const mediaQuery =
  typeof window !== "undefined"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : undefined;

function useSystemDark(): boolean {
  const subscribe = useCallback((cb: () => void) => {
    mediaQuery?.addEventListener("change", cb);
    return () => mediaQuery?.removeEventListener("change", cb);
  }, []);
  const getSnapshot = useCallback(() => mediaQuery?.matches ?? false, []);
  return useSyncExternalStore(subscribe, getSnapshot, () => false);
}

function useStoredTheme(
  storageKey: string,
  defaultTheme: Theme,
): [Theme, (t: Theme) => void] {
  const subscribe = useCallback(
    (cb: () => void) => {
      const handler = (e: StorageEvent) => {
        if (e.key === storageKey) cb();
      };
      window.addEventListener("storage", handler);
      return () => window.removeEventListener("storage", handler);
    },
    [storageKey],
  );
  const getSnapshot = useCallback(
    () => (localStorage.getItem(storageKey) as Theme) || defaultTheme,
    [storageKey, defaultTheme],
  );
  const getServerSnapshot = useCallback(() => defaultTheme, [defaultTheme]);
  const theme = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
  const setTheme = useCallback(
    (newTheme: Theme) => {
      localStorage.setItem(storageKey, newTheme);
      // Dispatch a storage event so our own subscriber picks up the change
      window.dispatchEvent(new StorageEvent("storage", { key: storageKey }));
    },
    [storageKey],
  );
  return [theme, setTheme];
}

export function ThemeProvider({
  children,
  defaultTheme = "system",
  storageKey = "den-theme",
}: ThemeProviderProps) {
  const [theme, setTheme] = useStoredTheme(storageKey, defaultTheme);
  const systemDark = useSystemDark();

  const resolvedTheme = useMemo<"light" | "dark">(() => {
    if (theme === "system") return systemDark ? "dark" : "light";
    return theme;
  }, [theme, systemDark]);

  // Sync class to <html> — the only true side effect
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add(resolvedTheme);
  }, [resolvedTheme]);

  const value = useMemo(
    () => ({ theme, setTheme, resolvedTheme }),
    [theme, setTheme, resolvedTheme],
  );

  return (
    <ThemeProviderContext.Provider value={value}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeProviderContext);
  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}
