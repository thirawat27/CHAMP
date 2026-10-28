import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { LogDoctorProgress, LogDoctorStatus } from "../types/services";
import { listenIfTauri } from "../utils/tauri";

export interface LogDoctorModelController {
  status: LogDoctorStatus | null;
  /** Download percent while the model is downloading, otherwise null. */
  progress: number | null;
  error: string | null;
  download: () => Promise<boolean>;
  remove: () => Promise<void>;
}

/** Status and install/remove actions for the optional Log Doctor AI model. */
export function useLogDoctorModel(): LogDoctorModelController {
  const [status, setStatus] = useState<LogDoctorStatus | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    invoke<LogDoctorStatus>("get_log_doctor_status")
      .then((value) => {
        if (!active) return;
        setStatus(value);
        if (value.downloading) setProgress(0);
      })
      .catch((statusError) => console.error("Failed to load Log Doctor status:", statusError));

    const unlisten = listenIfTauri<LogDoctorProgress>("log-doctor-progress", (event) => {
      setProgress(event.payload.percent);
    });

    return () => {
      active = false;
      unlisten.then((dispose) => dispose());
    };
  }, []);

  const download = useCallback(async () => {
    setError(null);
    setProgress(0);
    try {
      setStatus(await invoke<LogDoctorStatus>("download_log_doctor_model"));
      return true;
    } catch (downloadError) {
      setError(String(downloadError));
      return false;
    } finally {
      setProgress(null);
    }
  }, []);

  const remove = useCallback(async () => {
    setError(null);
    try {
      setStatus(await invoke<LogDoctorStatus>("remove_log_doctor_model"));
    } catch (removeError) {
      setError(String(removeError));
    }
  }, []);

  return { status, progress, error, download, remove };
}

export const formatModelSize = (bytes: number): string => `${Math.round(bytes / 1024 / 1024)} MB`;
