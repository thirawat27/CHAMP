/**
 * Language Store - Zustand
 *
 * Manages application language (i18n) using Zustand.
 */

import { create } from "zustand";
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

// Load saved language from localStorage
const getSavedLanguage = (): Language => {
  try {
    const saved = localStorage.getItem("language-storage");
    if (saved) {
      const parsed = JSON.parse(saved);
      return parsed.state?.language || "en";
    }
  } catch {
    // Ignore localStorage errors
  }
  return "en";
};

const initialLanguage = getSavedLanguage();

/**
 * Language store for managing i18n
 */
export const useLanguageStore = create<LanguageState>()((set, get) => ({
  language: initialLanguage,
  soundEnabled: (() => {
    try {
      const saved = localStorage.getItem("language-storage");
      if (saved) {
        const parsed = JSON.parse(saved);
        return parsed.state?.soundEnabled ?? true;
      }
    } catch {
      // Ignore localStorage errors
    }
    return true;
  })(),
  t: getTranslation(initialLanguage),

  setLanguage: (lang: Language) => {
    set({
      language: lang,
      t: getTranslation(lang),
    });
    // Save to localStorage
    try {
      const current = JSON.parse(localStorage.getItem("language-storage") || "{}");
      current.state = { ...current.state, language: lang };
      localStorage.setItem("language-storage", JSON.stringify(current));
    } catch {
      // Ignore localStorage errors
    }
    // Save to backend settings
    invoke("save_language_setting", { language: lang }).catch(console.error);
  },

  toggleSound: () => {
    const newState = !get().soundEnabled;
    set({ soundEnabled: newState });
    // Save to localStorage
    try {
      const current = JSON.parse(localStorage.getItem("language-storage") || "{}");
      current.state = { ...current.state, soundEnabled: newState };
      localStorage.setItem("language-storage", JSON.stringify(current));
    } catch {
      // Ignore localStorage errors
    }
    // Save to backend settings
    invoke("save_sound_setting", { enabled: newState }).catch(console.error);
  },

  setSoundEnabled: (enabled: boolean) => {
    set({ soundEnabled: enabled });
    // Save to localStorage
    try {
      const current = JSON.parse(localStorage.getItem("language-storage") || "{}");
      current.state = { ...current.state, soundEnabled: enabled };
      localStorage.setItem("language-storage", JSON.stringify(current));
    } catch {
      // Ignore localStorage errors
    }
    // Save to backend settings
    invoke("save_sound_setting", { enabled }).catch(console.error);
  },

  translate: (key: keyof Translations) => {
    return get().t[key];
  },
}));

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
