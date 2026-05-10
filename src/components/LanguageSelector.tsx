/**
 * Language Selector Component
 *
 * Shows language options with flag icons (Thailand/USA SVG).
 * Compact design for minimal space usage.
 */

import { Language } from "../i18n/translations";
import { useLanguageStore } from "../stores/languageStore";
import { AudioManager } from "../utils/audioManager";
import USAFlag from "../assets/USA.svg";
import ThailandFlag from "../assets/thailand.svg";

interface LanguageSelectorProps {
  variant?: "compact" | "full" | "toggle";
  showLabel?: boolean;
}

export function LanguageSelector({ variant = "compact", showLabel = false }: LanguageSelectorProps) {
  const { language, setLanguage } = useLanguageStore();

  const handleLanguageChange = (lang: Language) => {
    if (lang !== language) {
      AudioManager.playToggle();
      setLanguage(lang);
    }
  };

  const toggleLanguage = () => {
    const nextLang = language === "th" ? "en" : "th";
    AudioManager.playToggle();
    setLanguage(nextLang);
  };

  const options: { lang: Language; label: string; flag: string; alt: string }[] = [
    { lang: "th", label: "ไทย", flag: ThailandFlag, alt: "Thai flag" },
    { lang: "en", label: "EN", flag: USAFlag, alt: "USA flag" },
  ];

  // Toggle variant - single flag button that toggles language
  if (variant === "toggle") {
    const currentOption = options.find((o) => o.lang === language) || options[1];
    return (
      <button
        onClick={toggleLanguage}
        onMouseEnter={() => AudioManager.playHover()}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: "46px",
          height: "36px",
          padding: "3px",
          border: "1px solid var(--border-color)",
          borderRadius: "4px",
          backgroundColor: "var(--bg-card-secondary)",
          cursor: "pointer",
          transition: "all 0.15s ease",
        }}
        title={language === "th" ? "Switch to English" : "เปลี่ยนเป็นภาษาไทย"}
        aria-label={language === "th" ? "Switch to English" : "Switch to Thai"}
      >
        <img
          src={currentOption.flag}
          alt={currentOption.alt}
          style={{
            width: "100%",
            height: "100%",
            objectFit: "cover",
            borderRadius: "2px",
            display: "block",
          }}
        />
      </button>
    );
  }

  if (variant === "compact") {
    return (
      <div
        style={{
          display: "flex",
          gap: "6px",
          alignItems: "center",
          backgroundColor: "var(--bg-card-secondary)",
          padding: "4px",
          borderRadius: "8px",
          border: "1px solid var(--border-color)",
        }}
      >
        {options.map((option) => (
          <button
            key={option.lang}
            onClick={() => handleLanguageChange(option.lang)}
            onMouseEnter={() => AudioManager.playHover()}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: "40px",
              height: "28px",
              padding: "3px",
              border: "none",
              borderRadius: "4px",
              backgroundColor: language === option.lang ? "var(--primary-color)" : "transparent",
              cursor: "pointer",
              transition: "all 0.15s ease",
              opacity: language === option.lang ? 1 : 0.7,
              boxShadow: language === option.lang ? "0 1px 3px rgba(0,0,0,0.2)" : "none",
            }}
            title={option.lang === "th" ? "ภาษาไทย" : "English"}
            aria-label={option.lang === "th" ? "Switch to Thai" : "Switch to English"}
          >
            <img
              src={option.flag}
              alt={option.alt}
              style={{
                width: "100%",
                height: "100%",
                objectFit: "cover",
                borderRadius: "2px",
                display: "block",
              }}
            />
          </button>
        ))}
      </div>
    );
  }

  // Full variant with labels
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
      {showLabel && (
        <span
          style={{
            fontSize: "0.875rem",
            fontWeight: 500,
            color: "var(--text-primary)",
          }}
        >
          Language / ภาษา
        </span>
      )}
      <div style={{ display: "flex", gap: "8px" }}>
        {options.map((option) => (
          <button
            key={option.lang}
            onClick={() => handleLanguageChange(option.lang)}
            onMouseEnter={() => AudioManager.playHover()}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              padding: "8px 12px",
              border: `1px solid ${language === option.lang ? "var(--primary-color)" : "var(--border-color)"}`,
              borderRadius: "6px",
              backgroundColor: language === option.lang ? "var(--bg-card)" : "var(--bg-card-secondary)",
              cursor: "pointer",
              transition: "all 0.15s ease",
              fontSize: "0.875rem",
              color: language === option.lang ? "var(--primary-color)" : "var(--text-primary)",
              fontWeight: language === option.lang ? 500 : 400,
            }}
          >
            <img
              src={option.flag}
              alt={option.alt}
              style={{
                width: "24px",
                height: "16px",
                objectFit: "cover",
                borderRadius: "2px",
                display: "block",
              }}
            />
            <span>{option.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

export default LanguageSelector;
