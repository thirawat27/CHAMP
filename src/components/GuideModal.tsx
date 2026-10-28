import { AlertTriangle, BookOpen, Lightbulb, Search, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { GUIDE_SECTIONS, GuideBlock, GuideSection, Localized } from "../manual/content";
import { Language } from "../i18n/translations";
import { useTranslation } from "../stores/languageStore";
import { withInlineCode } from "../utils/inlineCode";

interface GuideModalProps {
  onClose: () => void;
  initialSection?: string;
}

const text = (value: Localized | string, language: Language) =>
  typeof value === "string" ? value : value[language];

/** All searchable words of a section in one language. */
function sectionText(section: GuideSection, language: Language): string {
  const parts: string[] = [section.title[language], section.summary[language]];
  for (const block of section.blocks) {
    switch (block.kind) {
      case "p":
      case "tip":
      case "warning":
        parts.push(block.text[language]);
        break;
      case "steps":
      case "list":
        parts.push(...block.items.map((item) => item[language]));
        break;
      case "code":
        parts.push(block.code, block.label?.[language] ?? "");
        break;
      case "table":
        parts.push(...block.rows.flat().map((cell) => text(cell, language)));
        break;
      case "keys":
        parts.push(...block.items.map((item) => `${item.keys} ${item.text[language]}`));
        break;
    }
  }
  return parts.join(" ").toLowerCase();
}

function Block({ block, language }: { block: GuideBlock; language: Language }) {
  switch (block.kind) {
    case "p":
      return <p>{withInlineCode(block.text[language])}</p>;
    case "steps":
      return (
        <ol className="guide-steps">
          {block.items.map((item) => (
            <li key={item.en}>{withInlineCode(item[language])}</li>
          ))}
        </ol>
      );
    case "list":
      return (
        <ul className="guide-list">
          {block.items.map((item) => (
            <li key={item.en}>{withInlineCode(item[language])}</li>
          ))}
        </ul>
      );
    case "code":
      return (
        <figure className="guide-code">
          {block.label && <figcaption>{block.label[language]}</figcaption>}
          <pre>{block.code}</pre>
        </figure>
      );
    case "tip":
      return (
        <aside className="guide-note tip">
          <Lightbulb size={16} />
          <p>{withInlineCode(block.text[language])}</p>
        </aside>
      );
    case "warning":
      return (
        <aside className="guide-note warning">
          <AlertTriangle size={16} />
          <p>{withInlineCode(block.text[language])}</p>
        </aside>
      );
    case "table":
      return (
        <div className="guide-table-wrap">
          <table className="guide-table">
            <thead>
              <tr>
                {block.head.map((cell, index) => (
                  <th key={index}>{text(cell, language)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {row.map((cell, cellIndex) => (
                    <td key={cellIndex}>{withInlineCode(text(cell, language))}</td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
    case "keys":
      return (
        <dl className="guide-keys">
          {block.items.map((item) => (
            <div key={item.keys}>
              <dt>
                {item.keys.split(" ").map((key) => (
                  <kbd key={key}>{key}</kbd>
                ))}
              </dt>
              <dd>{item.text[language]}</dd>
            </div>
          ))}
        </dl>
      );
  }
}

/** The full in-app user guide: contents, search, and every topic. */
export function GuideModal({ onClose, initialSection }: GuideModalProps) {
  const { t, language } = useTranslation();
  const [query, setQuery] = useState("");
  const [activeId, setActiveId] = useState(initialSection ?? GUIDE_SECTIONS[0].id);
  const contentRef = useRef<HTMLDivElement>(null);

  const sections = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return GUIDE_SECTIONS;
    return GUIDE_SECTIONS.filter((section) => sectionText(section, language).includes(needle));
  }, [language, query]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.code === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onClose]);

  useEffect(() => {
    if (!initialSection) return;
    document.getElementById(`guide-${initialSection}`)?.scrollIntoView({ block: "start" });
  }, [initialSection]);

  const jumpTo = (id: string) => {
    setActiveId(id);
    document.getElementById(`guide-${id}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  // Highlight the topic currently at the top of the reading area.
  const handleScroll = () => {
    const container = contentRef.current;
    if (!container) return;
    const top = container.getBoundingClientRect().top + 24;
    let current = sections[0]?.id;
    for (const section of sections) {
      const element = document.getElementById(`guide-${section.id}`);
      if (element && element.getBoundingClientRect().top <= top) current = section.id;
    }
    if (current && current !== activeId) setActiveId(current);
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <section
        className="guide-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="guide-title"
        onClick={(event) => event.stopPropagation()}
        data-testid="guide"
      >
        <header className="modal-header">
          <div className="guide-heading">
            <span className="guide-heading-icon" aria-hidden="true">
              <BookOpen size={18} />
            </span>
            <h2 id="guide-title">{t.manualTitle}</h2>
          </div>
          <button type="button" className="icon-button" onClick={onClose} aria-label={t.close}>
            <X size={18} />
          </button>
        </header>

        <div className="guide-layout">
          <nav className="guide-nav" aria-label={t.manualContents}>
            <label className="guide-search">
              <Search size={15} />
              <input
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder={t.manualSearch}
                aria-label={t.manualSearch}
              />
            </label>
            <ul>
              {sections.map((section) => {
                const Icon = section.icon;
                return (
                  <li key={section.id}>
                    <button
                      type="button"
                      className={section.id === activeId ? "active" : undefined}
                      onClick={() => jumpTo(section.id)}
                    >
                      <Icon size={15} />
                      <span>{section.title[language]}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </nav>

          <div className="guide-content" ref={contentRef} onScroll={handleScroll}>
            {sections.length === 0 && <p className="guide-empty">{t.manualNoResults}</p>}
            {sections.map((section) => {
              const Icon = section.icon;
              return (
                <article key={section.id} id={`guide-${section.id}`} className="guide-section">
                  <header>
                    <span className="guide-section-icon" aria-hidden="true">
                      <Icon size={18} />
                    </span>
                    <div>
                      <h3>{section.title[language]}</h3>
                      <p>{section.summary[language]}</p>
                    </div>
                  </header>
                  {section.blocks.map((block, index) => (
                    <Block key={index} block={block} language={language} />
                  ))}
                </article>
              );
            })}
          </div>
        </div>
      </section>
    </div>
  );
}
