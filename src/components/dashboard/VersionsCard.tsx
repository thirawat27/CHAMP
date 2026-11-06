import { ArrowUpCircle, LoaderCircle, Package } from "lucide-react";
import { useTranslation } from "../../stores/preferencesStore";
import { InstalledPhpVersion } from "../../types/services";

interface VersionsCardProps {
  installedVersions: Record<string, string>;
  databaseToolName: string;
  isAdminerSelected: boolean;
  phpUpdate: InstalledPhpVersion | null;
  phpUpdating: boolean;
  operationBusy: boolean;
  onUpdatePhp: () => void;
}

export function VersionsCard({
  installedVersions,
  databaseToolName,
  isAdminerSelected,
  phpUpdate,
  phpUpdating,
  operationBusy,
  onUpdatePhp,
}: VersionsCardProps) {
  const { t } = useTranslation();

  const rows = (
    [
      ["Caddy", installedVersions.caddy],
      ["PHP", installedVersions.php],
      ["MySQL", installedVersions.mysql],
      ["PostgreSQL", installedVersions.postgresql],
      [
        databaseToolName,
        // Both tools can be on disk after switching engines; show the active one.
        isAdminerSelected ? installedVersions.adminer : installedVersions.phpmyadmin,
      ],
      ["ngrok", installedVersions.ngrok],
    ] as Array<[string, unknown]>
  ).filter(
    (entry): entry is [string, string] => typeof entry[1] === "string" && entry[1].length > 0
  );

  if (rows.length === 0) return null;

  return (
    <section className="rail-card" aria-label={t.installedVersionsLabel}>
      <header className="rail-card-head">
        <Package size={16} />
        <h2>{t.installedVersionsLabel}</h2>
      </header>
      {phpUpdate && (
        <div className="version-update">
          <ArrowUpCircle size={15} />
          <span>
            PHP {phpUpdate.version} → <strong>{phpUpdate.latest_version}</strong>
          </span>
          <button
            type="button"
            className="btn-primary compact"
            onClick={onUpdatePhp}
            disabled={operationBusy}
          >
            {phpUpdating ? <LoaderCircle size={14} className="spin-icon" /> : null}
            {phpUpdating ? t.working : t.phpUpdateNow}
          </button>
        </div>
      )}
      <dl className="version-list">
        {rows.map(([name, version]) => (
          <div key={name}>
            <dt>{name}</dt>
            <dd>{version}</dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
