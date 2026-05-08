/**
 * Custom React Hooks for CHAMP Application
 * 
 * This module exports all custom hooks used throughout the application.
 * These hooks encapsulate reusable stateful logic and side effects.
 */

export { useServices } from "./useServices";
export type { UseServicesReturn } from "./useServices";

export { useAppConfig } from "./useAppConfig";
export type { UseAppConfigReturn, AppPaths } from "./useAppConfig";

export { useNotifications } from "./useNotifications";
export type { UseNotificationsReturn, DashboardNotice, NoticeTone, NoticeAction } from "./useNotifications";
