/**
 * Language toggle: switches the UI between Thai and English.
 */

import { Language } from "../i18n/translations";
import { useLanguageStore } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";

const LANGUAGES: { lang: Language; label: string }[] = [
  { lang: "th", label: "TH" },
  { lang: "en", label: "EN" },
];

export function LanguageSelector() {
  const { language, setLanguage } = useLanguageStore();

  return (
    <div
      className="lang-toggle"
      role="radiogroup"
      aria-label={language === "th" ? "ภาษา" : "Language"}
    >
      {LANGUAGES.map((option) => (
        <button
          key={option.lang}
          type="button"
          role="radio"
          aria-checked={language === option.lang}
          className={language === option.lang ? "active" : undefined}
          onClick={() => {
            if (option.lang === language) return;
            AudioManager.playToggle();
            setLanguage(option.lang);
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export default LanguageSelector;
