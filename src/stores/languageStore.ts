/**
 * Language Store - Zustand
 *
 * Manages application language (i18n) using Zustand.
 */

import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import { invoke } from "@tauri-apps/api/core";
import { Language, getTranslation, Translations } from "../i18n/translations";

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
  /** Get translation for a key */
  translate: (key: keyof Translations) => string;
}

const DEFAULT_LANGUAGE: Language = "en";
const isTauriRuntime = () => "__TAURI_INTERNALS__" in window;

/**
 * Language store for managing i18n.
 *
 * Persistence is handled by zustand's `persist` middleware, which writes to
 * localStorage under the key "language-storage" using the shape
 * `{ state: { language, soundEnabled }, version: n }`. Only `language` and
 * `soundEnabled` are persisted (via `partialize`); `t` is derived from the
 * restored `language` on rehydrate.
 */
export const useLanguageStore = create<LanguageState>()(
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

      translate: (key: keyof Translations) => {
        return get().t[key];
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

/**
 * Hook to get current translations
 */
export function useTranslation(): { t: Translations; language: Language } {
  const { t, language } = useLanguageStore();
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
    const settings = await invoke<{ language: Language; sound_enabled: boolean }>("get_language_settings");
    const store = useLanguageStore.getState();

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
