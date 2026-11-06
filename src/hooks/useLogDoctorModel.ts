import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import { create } from "zustand";
import { LogDoctorProgress, LogDoctorStatus } from "../types/services";
import { listenIfTauri } from "../utils/tauri";

interface LogDoctorModelController {
  status: LogDoctorStatus | null;
  /** Download percent while the model is downloading, otherwise null. */
  progress: number | null;
  error: string | null;
  download: () => Promise<boolean>;
  remove: () => Promise<void>;
}

/**
 * Log Doctor and Settings can be open together. They share one copy of the
 * model state, so a download started in one finishes in both.
 */
const useModelState = create<Pick<LogDoctorModelController, "status" | "progress">>(() => ({
  status: null,
  progress: null,
}));

/** Status and install/remove actions for the Log Doctor AI model (installed by default). */
export function useLogDoctorModel(): LogDoctorModelController {
  const status = useModelState((state) => state.status);
  const progress = useModelState((state) => state.progress);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    invoke<LogDoctorStatus>("get_log_doctor_status")
      .then((value) => {
        if (!active) return;
        useModelState.setState((state) => ({
          status: value,
          progress: value.downloading ? (state.progress ?? 0) : state.progress,
        }));
      })
      .catch((statusError) => console.error("Failed to load Log Doctor status:", statusError));

    const unlisten = listenIfTauri<LogDoctorProgress>("log-doctor-progress", (event) => {
      useModelState.setState({ progress: event.payload.percent });
    });
    // The model is downloaded in the background on app start by default.
    const unlistenStatus = listenIfTauri<LogDoctorStatus>("log-doctor-status", (event) => {
      useModelState.setState({ status: event.payload, progress: null });
    });

    return () => {
      active = false;
      unlisten.then((dispose) => dispose());
      unlistenStatus.then((dispose) => dispose());
    };
  }, []);

  const download = useCallback(async () => {
    setError(null);
    useModelState.setState({ progress: 0 });
    try {
      useModelState.setState({
        status: await invoke<LogDoctorStatus>("download_log_doctor_model"),
      });
      return true;
    } catch (downloadError) {
      setError(String(downloadError));
      return false;
    } finally {
      useModelState.setState({ progress: null });
    }
  }, []);

  const remove = useCallback(async () => {
    setError(null);
    try {
      useModelState.setState({
        status: await invoke<LogDoctorStatus>("remove_log_doctor_model"),
      });
    } catch (removeError) {
      setError(String(removeError));
    }
  }, []);

  return { status, progress, error, download, remove };
}
