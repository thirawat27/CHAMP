import champLogo from "../../assets/champ-logo.png";
import { useTranslation } from "../../stores/preferencesStore";

export interface SetupStepInfo {
  title: string;
  copy: string;
}

export function SetupRail({ steps, current }: { steps: SetupStepInfo[]; current: number }) {
  const { t } = useTranslation();

  return (
    <aside className="setup-rail">
      <div className="setup-brand">
        <img className="setup-brand-logo" src={champLogo} alt="" />
        <h1>CHAMP</h1>
      </div>
      <p>{t.wizardStackDescription}</p>
      <div className="setup-steps">
        {steps.map((step, index) => {
          const number = index + 1;
          const stateClass = current === number ? "active" : current > number ? "done" : "";
          return (
            <div key={step.title} className={`setup-step ${stateClass}`}>
              <span className="setup-step-index">{number}</span>
              <span>
                <span className="setup-step-title">{step.title}</span>
                <span className="setup-step-copy">{step.copy}</span>
              </span>
            </div>
          );
        })}
      </div>
    </aside>
  );
}
