/**
 * Translations - ระบบภาษาไทยและอังกฤษ
 */

export type Language = "th" | "en";

export interface Translations {
  // Common
  loading: string;
  cancel: string;
  save: string;
  close: string;
  open: string;
  settings: string;
  help: string;
  error: string;
  success: string;
  warning: string;
  info: string;
  confirm: string;
  yes: string;
  no: string;
  retry: string;
  delete: string;
  edit: string;
  add: string;
  remove: string;
  refresh: string;
  search: string;
  filter: string;
  sort: string;
  back: string;
  next: string;
  previous: string;
  finish: string;
  done: string;
  install: string;
  installed: string;
  available: string;
  active: string;
  inactive: string;
  enabled: string;
  disabled: string;
  version: string;
  status: string;
  name: string;
  description: string;
  type: string;
  action: string;
  actions: string;
  start: string;
  stop: string;
  restart: string;
  starting: string;
  stopping: string;
  restarting: string;
  running: string;
  stopped: string;
  failed: string;
  pending: string;
  inProgress: string;
  completed: string;

  // App
  appName: string;
  appDescription: string;

  // Dashboard
  dashboard: string;
  services: string;
  serviceStatus: string;
  allServices: string;
  webServer: string;
  php: string;
  mysql: string;
  database: string;
  phpMyAdmin: string;
  adminer: string;
  caddy: string;
  phpFPM: string;
  mariaDB: string;
  projects: string;
  logs: string;
  website: string;
  databaseTool: string;
  openWebsite: string;
  openDatabaseTool: string;
  openProjectsFolder: string;
  openLogsFolder: string;
  startAllServices: string;
  stopAllServices: string;
  restartAllServices: string;
  allServicesStarted: string;
  allServicesStopped: string;
  allServicesRestarted: string;
  serviceStarted: string;
  serviceStopped: string;
  serviceRestarted: string;
  serviceFailedToStart: string;
  serviceFailedToStop: string;
  serviceFailedToRestart: string;

  // Settings
  settingsTitle: string;
  settingsDescription: string;
  ports: string;
  port: string;
  httpPort: string;
  phpPort: string;
  mysqlPort: string;
  checkPorts: string;
  portAvailable: string;
  portInUse: string;
  phpVersions: string;
  phpVersion: string;
  activePhpRuntime: string;
  installSelected: string;
  switchPhp: string;
  working: string;
  workspace: string;
  projectsFolder: string;
  selectProjectsFolder: string;
  startup: string;
  autoStartServices: string;
  autoStartDescription: string;
  language: string;
  selectLanguage: string;
  thai: string;
  english: string;
  soundEffects: string;
  enableSoundEffects: string;
  soundEffectsDescription: string;
  databaseToolSelect: string;
  webDatabaseManager: string;

  // Keyboard Shortcuts
  keyboardShortcuts: string;
  shortcutsTitle: string;
  shortcutsDescription: string;
  shortcutStart: string;
  shortcutRestart: string;
  shortcutStop: string;
  shortcutWebsite: string;
  shortcutDatabase: string;
  shortcutProjects: string;
  shortcutLogs: string;
  shortcutSettings: string;
  shortcutHelp: string;
  quickAccess: string;

  // Notifications
  notificationInfo: string;
  notificationSuccess: string;
  notificationError: string;
  copiedToClipboard: string;
  copyError: string;

  // First Run Wizard
  welcome: string;
  welcomeTitle: string;
  welcomeDescription: string;
  selectPackages: string;
  packagesDescription: string;
  downloadProgress: string;
  downloading: string;
  extracting: string;
  finalizing: string;
  downloadComplete: string;
  installationComplete: string;
  readyToUse: string;
  startUsingChamp: string;

  // Errors
  genericError: string;
  loadError: string;
  saveError: string;
  networkError: string;
  unknownError: string;
  validationError: string;
  notFound: string;
  accessDenied: string;

  // Footer / Status
  ready: string;
  processing: string;
  showShortcuts: string;
}

const th: Translations = {
  // Common
  loading: "กำลังโหลด...",
  cancel: "ยกเลิก",
  save: "บันทึก",
  close: "ปิด",
  open: "เปิด",
  settings: "ตั้งค่า",
  help: "ช่วยเหลือ",
  error: "ข้อผิดพลาด",
  success: "สำเร็จ",
  warning: "คำเตือน",
  info: "ข้อมูล",
  confirm: "ยืนยัน",
  yes: "ใช่",
  no: "ไม่",
  retry: "ลองใหม่",
  delete: "ลบ",
  edit: "แก้ไข",
  add: "เพิ่ม",
  remove: "นำออก",
  refresh: "รีเฟรช",
  search: "ค้นหา",
  filter: "กรอง",
  sort: "เรียง",
  back: "ย้อนกลับ",
  next: "ถัดไป",
  previous: "ก่อนหน้า",
  finish: "เสร็จสิ้น",
  done: "เสร็จแล้ว",
  install: "ติดตั้ง",
  installed: "ติดตั้งแล้ว",
  available: "พร้อมใช้งาน",
  active: "ใช้งานอยู่",
  inactive: "ไม่ใช้งาน",
  enabled: "เปิดใช้งาน",
  disabled: "ปิดใช้งาน",
  version: "เวอร์ชัน",
  status: "สถานะ",
  name: "ชื่อ",
  description: "คำอธิบาย",
  type: "ประเภท",
  action: "การกระทำ",
  actions: "การกระทำ",
  start: "เริ่ม",
  stop: "หยุด",
  restart: "รีสตาร์ท",
  starting: "กำลังเริ่ม...",
  stopping: "กำลังหยุด...",
  restarting: "กำลังรีสตาร์ท...",
  running: "กำลังทำงาน",
  stopped: "หยุดทำงาน",
  failed: "ล้มเหลว",
  pending: "รอดำเนินการ",
  inProgress: "กำลังดำเนินการ",
  completed: "เสร็จสมบูรณ์",

  // App
  appName: "CHAMP",
  appDescription: "เว็บเซิร์ฟเวอร์สำหรับนักพัฒนา",

  // Dashboard
  dashboard: "แดชบอร์ด",
  services: "บริการ",
  serviceStatus: "สถานะบริการ",
  allServices: "บริการทั้งหมด",
  webServer: "เว็บเซิร์ฟเวอร์",
  php: "PHP",
  mysql: "MySQL",
  database: "ฐานข้อมูล",
  phpMyAdmin: "phpMyAdmin",
  adminer: "Adminer",
  caddy: "Caddy",
  phpFPM: "PHP-FPM",
  mariaDB: "MariaDB",
  projects: "โปรเจกต์",
  logs: "บันทึก",
  website: "เว็บไซต์",
  databaseTool: "เครื่องมือฐานข้อมูล",
  openWebsite: "เปิดเว็บไซต์",
  openDatabaseTool: "เปิดเครื่องมือฐานข้อมูล",
  openProjectsFolder: "เปิดโฟลเดอร์โปรเจกต์",
  openLogsFolder: "เปิดโฟลเดอร์บันทึก",
  startAllServices: "เริ่มบริการทั้งหมด",
  stopAllServices: "หยุดบริการทั้งหมด",
  restartAllServices: "รีสตาร์ทบริการทั้งหมด",
  allServicesStarted: "เริ่มบริการทั้งหมดแล้ว",
  allServicesStopped: "หยุดบริการทั้งหมดแล้ว",
  allServicesRestarted: "รีสตาร์ทบริการทั้งหมดแล้ว",
  serviceStarted: "เริ่มบริการแล้ว",
  serviceStopped: "หยุดบริการแล้ว",
  serviceRestarted: "รีสตาร์ทบริการแล้ว",
  serviceFailedToStart: "ไม่สามารถเริ่มบริการได้",
  serviceFailedToStop: "ไม่สามารถหยุดบริการได้",
  serviceFailedToRestart: "ไม่สามารถรีสตาร์ทบริการได้",

  // Settings
  settingsTitle: "การตั้งค่า",
  settingsDescription: "พอร์ต โฟลเดอร์โปรเจกต์ และพฤติกรรมการเริ่มต้น",
  ports: "พอร์ต",
  port: "พอร์ต",
  httpPort: "HTTP",
  phpPort: "PHP FastCGI",
  mysqlPort: "MySQL",
  checkPorts: "ตรวจสอบพอร์ต",
  portAvailable: "พร้อมใช้งาน",
  portInUse: "กำลังใช้งาน",
  phpVersions: "เวอร์ชัน PHP",
  phpVersion: "เวอร์ชัน PHP",
  activePhpRuntime: "PHP ที่ใช้งานอยู่",
  installSelected: "ติดตั้งที่เลือก",
  switchPhp: "สลับ PHP",
  working: "กำลังทำงาน...",
  workspace: "พื้นที่ทำงาน",
  projectsFolder: "โฟลเดอร์โปรเจกต์",
  selectProjectsFolder: "เลือกโฟลเดอร์โปรเจกต์",
  startup: "การเริ่มต้น",
  autoStartServices: "เริ่มบริการอัตโนมัติ",
  autoStartDescription: "เริ่มบริการทั้งหมดเมื่อเปิด CHAMP",
  language: "ภาษา",
  selectLanguage: "เลือกภาษา",
  thai: "ไทย",
  english: "English",
  soundEffects: "เสียงเอฟเฟกต์",
  enableSoundEffects: "เปิดใช้งานเสียงเอฟเฟกต์",
  soundEffectsDescription: "เล่นเสียงเมื่อกดปุ่มและดำเนินการต่างๆ",
  databaseToolSelect: "เครื่องมือฐานข้อมูล",
  webDatabaseManager: "ตัวจัดการฐานข้อมูลบนเว็บ",

  // Keyboard Shortcuts
  keyboardShortcuts: "คีย์ลัด",
  shortcutsTitle: "คีย์ลัดแป้นพิมพ์",
  shortcutsDescription: "คีย์ลัดที่ใช้ได้ทั่วทั้งแอป",
  shortcutStart: "เริ่มบริการทั้งหมด",
  shortcutRestart: "รีสตาร์ทบริการทั้งหมด",
  shortcutStop: "หยุดบริการทั้งหมด",
  shortcutWebsite: "เปิดเว็บไซต์ (localhost)",
  shortcutDatabase: "เปิดเครื่องมือฐานข้อมูล",
  shortcutProjects: "เปิดโฟลเดอร์โปรเจกต์",
  shortcutLogs: "เปิดโฟลเดอร์บันทึก",
  shortcutSettings: "เปิด/ปิดการตั้งค่า",
  shortcutHelp: "แสดงคีย์ลัด",
  quickAccess: "เปิดใช้งานด่วน",

  // Notifications
  notificationInfo: "ข้อมูล",
  notificationSuccess: "สำเร็จ",
  notificationError: "ข้อผิดพลาด",
  copiedToClipboard: "คัดลอกไปยังคลิปบอร์ดแล้ว",
  copyError: "ไม่สามารถคัดลอกข้อผิดพลาดได้",

  // First Run Wizard
  welcome: "ยินดีต้อนรับ",
  welcomeTitle: "ยินดีต้อนรับสู่ CHAMP",
  welcomeDescription: "เลือกแพ็คเกจที่ต้องการติดตั้งสำหรับเว็บเซิร์ฟเวอร์ของคุณ",
  selectPackages: "เลือกแพ็คเกจ",
  packagesDescription: "เลือกเวอร์ชันที่ต้องการติดตั้ง",
  downloadProgress: "ความคืบหน้าการดาวน์โหลด",
  downloading: "กำลังดาวน์โหลด",
  extracting: "กำลังแตกไฟล์",
  finalizing: "กำลังสร้างขั้นตอนสุดท้าย",
  downloadComplete: "ดาวน์โหลดเสร็จสมบูรณ์",
  installationComplete: "การติดตั้งเสร็จสมบูรณ์",
  readyToUse: "CHAMP พร้อมใช้งานแล้ว!",
  startUsingChamp: "เริ่มใช้งาน CHAMP",

  // Errors
  genericError: "เกิดข้อผิดพลาด",
  loadError: "ไม่สามารถโหลดข้อมูลได้",
  saveError: "ไม่สามารถบันทึกข้อมูลได้",
  networkError: "ข้อผิดพลาดเครือข่าย",
  unknownError: "ข้อผิดพลาดที่ไม่รู้จัก",
  validationError: "ข้อมูลไม่ถูกต้อง",
  notFound: "ไม่พบข้อมูล",
  accessDenied: "การเข้าถึงถูกปฏิเสธ",

  // Footer / Status
  ready: "พร้อม",
  processing: "กำลังประมวลผล",
  showShortcuts: "กด ? เพื่อดูคีย์ลัด",
};

const en: Translations = {
  // Common
  loading: "Loading...",
  cancel: "Cancel",
  save: "Save",
  close: "Close",
  open: "Open",
  settings: "Settings",
  help: "Help",
  error: "Error",
  success: "Success",
  warning: "Warning",
  info: "Info",
  confirm: "Confirm",
  yes: "Yes",
  no: "No",
  retry: "Retry",
  delete: "Delete",
  edit: "Edit",
  add: "Add",
  remove: "Remove",
  refresh: "Refresh",
  search: "Search",
  filter: "Filter",
  sort: "Sort",
  back: "Back",
  next: "Next",
  previous: "Previous",
  finish: "Finish",
  done: "Done",
  install: "Install",
  installed: "Installed",
  available: "Available",
  active: "Active",
  inactive: "Inactive",
  enabled: "Enabled",
  disabled: "Disabled",
  version: "Version",
  status: "Status",
  name: "Name",
  description: "Description",
  type: "Type",
  action: "Action",
  actions: "Actions",
  start: "Start",
  stop: "Stop",
  restart: "Restart",
  starting: "Starting...",
  stopping: "Stopping...",
  restarting: "Restarting...",
  running: "Running",
  stopped: "Stopped",
  failed: "Failed",
  pending: "Pending",
  inProgress: "In Progress",
  completed: "Completed",

  // App
  appName: "CHAMP",
  appDescription: "Web Server for Developers",

  // Dashboard
  dashboard: "Dashboard",
  services: "Services",
  serviceStatus: "Service Status",
  allServices: "All Services",
  webServer: "Web Server",
  php: "PHP",
  mysql: "MySQL",
  database: "Database",
  phpMyAdmin: "phpMyAdmin",
  adminer: "Adminer",
  caddy: "Caddy",
  phpFPM: "PHP-FPM",
  mariaDB: "MariaDB",
  projects: "Projects",
  logs: "Logs",
  website: "Website",
  databaseTool: "Database Tool",
  openWebsite: "Open Website",
  openDatabaseTool: "Open Database Tool",
  openProjectsFolder: "Open Projects Folder",
  openLogsFolder: "Open Logs Folder",
  startAllServices: "Start All Services",
  stopAllServices: "Stop All Services",
  restartAllServices: "Restart All Services",
  allServicesStarted: "All services started",
  allServicesStopped: "All services stopped",
  allServicesRestarted: "All services restarted",
  serviceStarted: "Service started",
  serviceStopped: "Service stopped",
  serviceRestarted: "Service restarted",
  serviceFailedToStart: "Failed to start service",
  serviceFailedToStop: "Failed to stop service",
  serviceFailedToRestart: "Failed to restart service",

  // Settings
  settingsTitle: "Settings",
  settingsDescription: "Ports, project folder, and startup behavior",
  ports: "Ports",
  port: "Port",
  httpPort: "HTTP",
  phpPort: "PHP FastCGI",
  mysqlPort: "MySQL",
  checkPorts: "Check Ports",
  portAvailable: "Available",
  portInUse: "In use",
  phpVersions: "PHP Versions",
  phpVersion: "PHP Version",
  activePhpRuntime: "Active PHP runtime",
  installSelected: "Install Selected",
  switchPhp: "Switch PHP",
  working: "Working...",
  workspace: "Workspace",
  projectsFolder: "Projects folder",
  selectProjectsFolder: "Select projects folder",
  startup: "Startup",
  autoStartServices: "Start stack when CHAMP opens",
  autoStartDescription: "Automatically start all services when CHAMP opens",
  language: "Language",
  selectLanguage: "Select Language",
  thai: "ไทย",
  english: "English",
  soundEffects: "Sound Effects",
  enableSoundEffects: "Enable sound effects",
  soundEffectsDescription: "Play sounds when pressing buttons and performing actions",
  databaseToolSelect: "Database Tool",
  webDatabaseManager: "Web database manager",

  // Keyboard Shortcuts
  keyboardShortcuts: "Shortcuts",
  shortcutsTitle: "Keyboard Shortcuts",
  shortcutsDescription: "Shortcuts available throughout the app",
  shortcutStart: "Start all services",
  shortcutRestart: "Restart all services",
  shortcutStop: "Stop all services",
  shortcutWebsite: "Open website (localhost)",
  shortcutDatabase: "Open database tool",
  shortcutProjects: "Open projects folder",
  shortcutLogs: "Open logs folder",
  shortcutSettings: "Toggle settings panel",
  shortcutHelp: "Show keyboard shortcuts",
  quickAccess: "Quick Access",

  // Notifications
  notificationInfo: "Info",
  notificationSuccess: "Success",
  notificationError: "Error",
  copiedToClipboard: "Copied to clipboard",
  copyError: "Failed to copy error",

  // First Run Wizard
  welcome: "Welcome",
  welcomeTitle: "Welcome to CHAMP",
  welcomeDescription: "Select the packages you want to install for your web server",
  selectPackages: "Select Packages",
  packagesDescription: "Select the versions you want to install",
  downloadProgress: "Download Progress",
  downloading: "Downloading",
  extracting: "Extracting",
  finalizing: "Finalizing setup",
  downloadComplete: "Download complete",
  installationComplete: "Installation complete",
  readyToUse: "CHAMP is ready to use!",
  startUsingChamp: "Start using CHAMP",

  // Errors
  genericError: "An error occurred",
  loadError: "Failed to load data",
  saveError: "Failed to save data",
  networkError: "Network error",
  unknownError: "Unknown error",
  validationError: "Validation error",
  notFound: "Not found",
  accessDenied: "Access denied",

  // Footer / Status
  ready: "Ready",
  processing: "Processing",
  showShortcuts: "Press ? for shortcuts",
};

export const translations: Record<Language, Translations> = { th, en };

export function getTranslation(lang: Language): Translations {
  return translations[lang];
}
