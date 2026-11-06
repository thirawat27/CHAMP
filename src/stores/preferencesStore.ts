/**
 * Preferences Store - Zustand
 *
 * Application language (i18n) and sound effects.
 */

import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";
import { Language, getTranslation, Translations } from "../i18n/translations";
import { isTauriRuntime } from "../utils/tauri";

interface LanguageState {
  /** Current language */
  language: Language;
  /** Sound effects enabled */
  soundEnabled: boolean;
  /** Translation data */
  t: Translations;

  // Actions
  /** Set language */
  setLanguage: (lang: Language) => void;
  /** Toggle sound effects */
  toggleSound: () => void;
  /** Set sound enabled state */
  setSoundEnabled: (enabled: boolean) => void;
}

const DEFAULT_LANGUAGE: Language = "en";

/**
 * Store for language and sound preferences.
 *
 * Persistence is handled by zustand's `persist` middleware, which writes to
 * localStorage under the key "language-storage" using the shape
 * `{ state: { language, soundEnabled }, version: n }`. Only `language` and
 * `soundEnabled` are persisted (via `partialize`); `t` is derived from the
 * restored `language` on rehydrate.
 */
export const usePreferencesStore = create<LanguageState>()(
  persist(
    (set, get) => ({
      language: DEFAULT_LANGUAGE,
      soundEnabled: true,
      t: getTranslation(DEFAULT_LANGUAGE),

      setLanguage: (lang: Language) => {
        set({
          language: lang,
          t: getTranslation(lang),
        });
        // Save to backend settings (persist handles localStorage)
        if (isTauriRuntime()) {
          invoke("save_language_setting", { language: lang }).catch(console.error);
        }
      },

      toggleSound: () => {
        const newState = !get().soundEnabled;
        set({ soundEnabled: newState });
        // Save to backend settings (persist handles localStorage)
        if (isTauriRuntime()) {
          invoke("save_sound_setting", { enabled: newState }).catch(console.error);
        }
      },

      setSoundEnabled: (enabled: boolean) => {
        set({ soundEnabled: enabled });
        // Save to backend settings (persist handles localStorage)
        if (isTauriRuntime()) {
          invoke("save_sound_setting", { enabled }).catch(console.error);
        }
      },
    }),
    {
      name: "language-storage",
      storage: createJSONStorage(() => localStorage),
      // Persist only language + soundEnabled (not `t` or the action functions).
      partialize: (s) => ({ language: s.language, soundEnabled: s.soundEnabled }),
      // Ensure translations match the restored language after rehydration.
      onRehydrateStorage: () => (state) => {
        if (state) {
          state.t = getTranslation(state.language);
        }
      },
    }
  )
);

/** Current translations for event handlers and other code outside a render. */
export const getTranslations = (): Translations => usePreferencesStore.getState().t;

/**
 * Hook to get current translations
 */
export function useTranslation(): { t: Translations; language: Language } {
  const { t, language } = usePreferencesStore();
  return { t, language };
}

/**
 * Initialize language from backend settings
 */
export async function initializeLanguage(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  try {
    const settings = await invoke<{ language: Language; sound_enabled: boolean }>(
      "get_language_settings"
    );
    const store = usePreferencesStore.getState();

    if (settings.language && settings.language !== store.language) {
      store.setLanguage(settings.language);
    }

    if (settings.sound_enabled !== undefined && settings.sound_enabled !== store.soundEnabled) {
      store.setSoundEnabled(settings.sound_enabled);
    }
  } catch (error) {
    console.error("Failed to load language settings:", error);
  }
}
