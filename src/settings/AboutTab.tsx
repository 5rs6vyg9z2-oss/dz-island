import { emit } from "@tauri-apps/api/event";
import { Compass, FileDown, Upload, RotateCcw, Trash2 } from "lucide-react";
import { SettingRow } from "./SettingRow";
import { useT } from "../i18n";

interface AboutTabProps {
  appVersion: string;
  exportStatus: string;
  importStatus: string;
  handleExportSettings: () => void;
  handleImportSettings: () => void;
  resetStatus: string;
  handleResetSettings: () => void;
  uninstallStatus: string;
  uninstallError?: string;
  handleUninstallNectar: () => void;
}

export function AboutTab({
  appVersion,
  exportStatus,
  importStatus,
  handleExportSettings,
  handleImportSettings,
  resetStatus,
  handleResetSettings,
  uninstallStatus,
  uninstallError,
  handleUninstallNectar,
}: AboutTabProps) {
  const t = useT();

  const getExportLabel = () => {
    if (exportStatus === "exporting") return t("about.export.working");
    if (exportStatus === "success") return t("about.export.done");
    return t("about.export");
  };

  const getImportLabel = () => {
    if (importStatus === "importing") return t("about.import.working");
    if (importStatus === "success") return t("about.import.done");
    return t("about.import");
  };

  const getResetLabel = () => {
    if (resetStatus === "resetting") return t("about.reset.working");
    if (resetStatus === "confirm") return t("about.confirmAgain");
    return t("about.reset");
  };

  const getResetDesc = () =>
    resetStatus === "confirm" ? t("about.reset.confirm.desc") : t("about.reset.desc");

  const getUninstallLabel = () => {
    if (uninstallStatus === "uninstalling") return t("about.uninstall.working");
    if (uninstallStatus === "confirm") return t("about.confirmAgain");
    if (uninstallStatus === "error") return t("about.uninstall.error");
    return t("about.uninstall");
  };

  const getUninstallDesc = () => {
    if (uninstallStatus === "confirm") return t("about.uninstall.confirm.desc");
    if (uninstallStatus === "error") return uninstallError || t("about.uninstall.error.desc");
    return t("about.uninstall.desc");
  };


  return (
    <div className="about-tab-container">
      <div className="about-header">
        <img src="/nectar.png" className="about-logo" alt="Serginho, mascote do DZ Island" />
        <h1 className="about-title">DZ Island</h1>
        <p className="about-version">{t("about.version", { version: appVersion || "—" })}</p>
      </div>

      <div className="setting-group-label setting-group-label--spaced">
        {t("about.data.group")}
      </div>
      <div className="setting-group">
        <SettingRow icon={Compass} label={t("about.tour")} desc={t("about.tour.desc")} action onClick={() => emit("start-tour")} />
        <SettingRow icon={FileDown} label={getExportLabel()} desc={t("about.export.desc")} action onClick={handleExportSettings} />
        <SettingRow icon={Upload} label={getImportLabel()} desc={t("about.import.desc")} action onClick={handleImportSettings} />
        <SettingRow
          icon={RotateCcw}
          label={getResetLabel()}
          desc={getResetDesc()}
          action
          divider={false}
          danger={resetStatus === "confirm"}
          onClick={handleResetSettings}
        />
      </div>

      <div className="setting-group-label setting-group-label--spaced">
        {t("about.danger.group")}
      </div>
      <div className="setting-group">
        <SettingRow
          icon={Trash2}
          label={getUninstallLabel()}
          desc={getUninstallDesc()}
          action
          divider={false}
          danger={uninstallStatus === "confirm" || uninstallStatus === "error"}
          onClick={handleUninstallNectar}
        />
      </div>

      <div className="about-footer">
        <p>{t("about.footer")}</p>
      </div>
    </div>
  );
}
