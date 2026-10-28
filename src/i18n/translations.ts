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
  no: string;
  retry: string;
  remove: string;
  refresh: string;
  more: string;
  search: string;
  filter: string;
  back: string;
  next: string;
  done: string;
  install: string;
  installed: string;
  available: string;
  active: string;
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

  // App
  appName: string;
  appDescription: string;

  // Dashboard
  dashboard: string;
  services: string;
  php: string;
  mysql: string;
  database: string;
  phpMyAdmin: string;
  adminer: string;
  caddy: string;
  projects: string;
  logs: string;
  website: string;
  databaseTool: string;
  openWebsite: string;
  openDatabaseTool: string;
  openProjectsFolder: string;
  openLogsFolder: string;
  network: string;
  phpUpdateTo: string;
  phpUpdateNow: string;
  phpUpdating: string;
  phpUpdated: string;
  manual: string;
  manualTitle: string;
  manualSearch: string;
  manualNoResults: string;
  manualContents: string;
  stack: string;
  requestPath: string;
  browser: string;
  shortcuts: string;
  notInStack: string;
  agentActivityTitle: string;
  agentActivityDone: string;
  updateAvailableTitle: string;
  updateRestartNote: string;
  updateInstalling: string;
  updateInstall: string;
  updateLater: string;
  updatesTitle: string;
  updateCheck: string;
  updateChecking: string;
  updateCurrent: string;
  updateLastChecked: string;
  updateNeverChecked: string;
  updateReleaseNotes: string;
  updateFailed: string;
  autoCheckUpdates: string;
  agentConnectorTitle: string;
  agentConnectorDescription: string;
  agentConnectorEnable: string;
  agentConnectorRunning: string;
  agentConnectorStopped: string;
  agentConnectorReadOnly: string;
  agentConnectorReadOnlyHint: string;
  agentConnectorControlHint: string;
  agentConnectorPort: string;
  agentConnectorEndpoint: string;
  agentConnectorClaudeCode: string;
  agentConnectorJson: string;
  agentConnectorCopy: string;
  agentConnectorCopied: string;
  agentConnectorTokenHint: string;
  agentConnectorRegenerate: string;
  diagnose: string;
  logDoctor: string;
  logDoctorDescription: string;
  logDoctorReading: string;
  logDoctorRecheck: string;
  logDoctorNoMatch: string;
  logDoctorNoMatchHint: string;
  logDoctorNoLogs: string;
  logDoctorEvidence: string;
  logDoctorSteps: string;
  logDoctorLogLines: string;
  logDoctorKnownError: string;
  logDoctorAiMatch: string;
  logDoctorAiReady: string;
  logDoctorAiOff: string;
  logDoctorEnableAi: string;
  logDoctorDownloading: string;
  logDoctorRemoveAi: string;
  logDoctorAiTitle: string;
  logDoctorAiSettingsDescription: string;
  logDoctorAiNotInstalled: string;
  doctorActionOpenSettings: string;
  doctorActionOpenUrl: string;
  doctorActionStartPhp: string;
  doctorActionOpenMysqlData: string;
  doctorActionOpenPostgresqlData: string;
  httpsPreview: string;
  publicHttpsDomain: string;
  httpsTunnelDescription: string;
  httpsTunnelStart: string;
  httpsTunnelStop: string;
  httpsTunnelOpen: string;
  httpsTunnelCopy: string;
  httpsTunnelStarting: string;
  httpsTunnelStarted: string;
  httpsTunnelStopped: string;
  httpsTunnelWaitingForDomain: string;
  httpsTunnelValidating: string;
  httpsTunnelError: string;
  httpsTunnelDevOnly: string;
  terminal: string;
  openTerminal: string;
  openConfigFolder: string;
  startAllServices: string;
  stopAllServices: string;
  restartAllServices: string;
  serviceStarted: string;
  serviceStopped: string;
  serviceRestarted: string;

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
  startup: string;
  autoStartServices: string;
  autoStartDescription: string;
  language: string;
  thai: string;
  soundEffects: string;
  enableSoundEffects: string;
  soundEffectsDescription: string;
  databaseToolSelect: string;
  webDatabaseManager: string;
  runtime: string;
  openRuntimeFolder: string;
  sourceRepository: string;
  stackStartingTitle: string;
  stackStartingMessage: string;
  stackStartedTitle: string;
  stackStartedMessage: string;
  stackRestartingTitle: string;
  stackRestartingMessage: string;
  stackRestartedTitle: string;
  stackRestartedMessage: string;
  stackStoppingTitle: string;
  stackStoppingMessage: string;
  stackStoppedTitle: string;
  stackStoppedMessage: string;
  serviceStartingTitle: string;
  serviceStartingMessage: string;
  serviceRestartingTitle: string;
  serviceRestartingMessage: string;
  serviceStoppingTitle: string;
  serviceStoppingMessage: string;
  dashboardRefreshingStatus: string;
  fallbackPortsUsed: string;
  commandFailed: string;
  openFolderFailed: string;
  openTerminalFailed: string;
  installedSuffix: string;
  installedVersionsLabel: string;
  settingsSaved: string;
  failedToLoadSettings: string;
  failedToSaveSettings: string;
  phpVersionInstalled: string;
  activePhpChanged: string;
  failedToInstallPhpVersion: string;
  failedToSwitchPhpVersion: string;
  databaseUsedWhenStartingStack: string;
  runtimeCatalogUpdateFailed: string;
  ltsBadge: string;
  stableBadge: string;
  eolBadge: string;
  debugMenu: string;
  viewDownloadFolder: string;
  resetInstallation: string;
  resetInstallationConfirm: string;
  resetFailed: string;
  openDownloadFolderFailed: string;
  showFirstRunWizard: string;

  // Keyboard Shortcuts
  shortcutsTitle: string;
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
  copiedToClipboard: string;
  copyFailed: string;

  // First Run Wizard
  welcome: string;
  downloading: string;
  extracting: string;
  installationComplete: string;
  wizardRuntimeOverview: string;
  wizardPackagesStep: string;
  wizardChooseVersions: string;
  wizardChecksStep: string;
  wizardSystemReadiness: string;
  wizardReviewStep: string;
  wizardKeepOrReplace: string;
  wizardInstallStep: string;
  wizardDownloadRuntime: string;
  wizardStackDescription: string;
  wizardWelcomeBody: string;
  wizardUserSpaceRuntime: string;
  wizardUserSpaceRuntimeCopy: string;
  wizardDevFirstPorts: string;
  wizardDevFirstPortsCopy: string;
  wizardDatabaseToolReady: string;
  wizardDatabaseToolReadyCopy: string;
  wizardExistingDetected: string;
  wizardEstimatedDownloadSize: string;
  wizardVariesByPlatform: string;
  wizardUseExisting: string;
  wizardDownloadFresh: string;
  wizardGetStarted: string;
  wizardReadUserManual: string;
  wizardPackageIntro: string;
  wizardDownloadAndInstall: string;
  wizardMissingSystemDependencies: string;
  wizardInstallCommand: string;
  wizardDependencyRetryCopy: string;
  wizardRetryCheck: string;
  wizardInstallationSummary: string;
  wizardKeepAndInstall: string;
  wizardInstallAll: string;
  wizardDownloadVerb: string;
  wizardExtractVerb: string;
  wizardInstalling: string;
  wizardPreparing: string;
  wizardRuntimeInstalled: string;
  wizardReadyToStartStack: string;
  wizardContinueToDashboard: string;
  wizardNoExistingInstallation: string;
  wizardRecommended: string;
  wizardLoadingPackages: string;
  wizardFailedToLoadPackages: string;
  wizardDefault: string;
  wizardNote: string;
  wizardEolNote: string;

  // Errors
  genericError: string;
  unknownError: string;

  // Footer / Status
  ready: string;
  processing: string;
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
  no: "ไม่",
  retry: "ลองใหม่",
  remove: "นำออก",
  refresh: "รีเฟรช",
  more: "เพิ่มเติม",
  search: "ค้นหา",
  filter: "กรอง",
  back: "ย้อนกลับ",
  next: "ถัดไป",
  done: "เสร็จแล้ว",
  install: "ติดตั้ง",
  installed: "ติดตั้งแล้ว",
  available: "พร้อมใช้งาน",
  active: "ใช้งานอยู่",
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

  // App
  appName: "CHAMP",
  appDescription: "เว็บเซิร์ฟเวอร์สำหรับนักพัฒนา",

  // Dashboard
  dashboard: "แดชบอร์ด",
  services: "บริการ",
  php: "PHP",
  mysql: "MySQL",
  database: "ฐานข้อมูล",
  phpMyAdmin: "phpMyAdmin",
  adminer: "Adminer",
  caddy: "Caddy",
  projects: "โปรเจกต์",
  logs: "บันทึก",
  website: "เว็บไซต์",
  databaseTool: "เครื่องมือฐานข้อมูล",
  openWebsite: "เปิดเว็บไซต์",
  openDatabaseTool: "เปิดเครื่องมือฐานข้อมูล",
  openProjectsFolder: "เปิดโฟลเดอร์โปรเจกต์",
  openLogsFolder: "เปิดโฟลเดอร์ Log",
  network: "เครือข่าย",
  phpUpdateTo: "อัปเดตเป็น {version}",
  phpUpdateNow: "อัปเดต",
  phpUpdating: "กำลังอัปเดต PHP",
  phpUpdated: "อัปเดต PHP เป็น {version} แล้ว",
  manual: "คู่มือ",
  manualTitle: "คู่มือการใช้งาน CHAMP",
  manualSearch: "ค้นหาในคู่มือ",
  manualNoResults: "ไม่พบหัวข้อที่ตรงกับคำค้น",
  manualContents: "สารบัญ",
  stack: "stack",
  requestPath: "เส้นทาง request",
  browser: "เบราว์เซอร์",
  shortcuts: "ทางลัด",
  notInStack: "ไม่ได้ใช้ใน stack ปัจจุบัน เปลี่ยนเครื่องมือฐานข้อมูลใน Settings เพื่อใช้ตัวนี้",
  agentActivityTitle: "AI agent สั่งงาน CHAMP",
  agentActivityDone: "{action}: {target}",
  updateAvailableTitle: "CHAMP v{version} พร้อมติดตั้ง",
  updateRestartNote: "บริการทั้งหมดจะหยุดระหว่างที่ CHAMP รีสตาร์ท",
  updateInstalling: "กำลังดาวน์โหลดและติดตั้ง CHAMP จะรีสตาร์ทเอง",
  updateInstall: "ติดตั้งและรีสตาร์ท",
  updateLater: "ไว้ทีหลัง",
  updatesTitle: "อัปเดต",
  updateCheck: "ตรวจหาอัปเดต",
  updateChecking: "กำลังตรวจ...",
  updateCurrent: "คุณใช้เวอร์ชันล่าสุดแล้ว",
  updateLastChecked: "ตรวจล่าสุด {time}",
  updateNeverChecked: "ยังไม่เคยตรวจหาอัปเดต",
  updateReleaseNotes: "มีอะไรใหม่",
  updateFailed: "อัปเดตไม่สำเร็จ",
  autoCheckUpdates: "ตรวจหาอัปเดตจาก GitHub วันละครั้งตอนเปิดโปรแกรม",
  agentConnectorTitle: "AI Agent Connector",
  agentConnectorDescription:
    "เปิด MCP server ในเครื่อง ให้ AI coding agent อย่าง Claude Code, Cursor หรือ Codex ดูสถานะ อ่าน log ใช้ Log Doctor และสั่งเปิด/ปิดบริการได้ ฟังเฉพาะ 127.0.0.1 และต้องใช้ token",
  agentConnectorEnable: "เปิด AI Agent Connector",
  agentConnectorRunning: "ทำงานอยู่",
  agentConnectorStopped: "ไม่ได้ทำงาน",
  agentConnectorReadOnly: "อ่านอย่างเดียว",
  agentConnectorReadOnlyHint: "agent ดูสถานะ log และผลวิเคราะห์ได้ แต่สั่งเปิด/ปิดบริการไม่ได้",
  agentConnectorControlHint:
    "agent สั่ง start, stop และ restart บริการได้ ทุกครั้งที่สั่งจะมีแจ้งเตือนในหน้าหลัก",
  agentConnectorPort: "พอร์ต",
  agentConnectorEndpoint: "Endpoint",
  agentConnectorClaudeCode: "Claude Code (รันใน terminal)",
  agentConnectorJson: "Cursor และ agent อื่น (JSON)",
  agentConnectorCopy: "คัดลอก",
  agentConnectorCopied: "คัดลอกแล้ว",
  agentConnectorTokenHint: "คำสั่งที่คัดลอกมี token ลับอยู่ อย่าแชร์ ถ้าหลุดให้สร้าง token ใหม่",
  agentConnectorRegenerate: "สร้าง token ใหม่",
  diagnose: "วิเคราะห์",
  logDoctor: "Log Doctor",
  logDoctorDescription: "อ่าน log ของบริการ แล้วบอกสาเหตุที่น่าจะเป็นพร้อมวิธีแก้",
  logDoctorReading: "กำลังอ่าน log...",
  logDoctorRecheck: "ตรวจอีกครั้ง",
  logDoctorNoMatch: "ยังไม่พบสาเหตุที่รู้จัก",
  logDoctorNoMatchHint: "บรรทัดด้านล่างคือคำเตือนล่าสุดใน log เปิดโฟลเดอร์ Log เพื่อดูไฟล์เต็ม",
  logDoctorNoLogs: "ยังไม่มี log ของบริการนี้ ลอง Start บริการก่อนแล้วตรวจอีกครั้ง",
  logDoctorEvidence: "บรรทัด log ที่ตรงกัน",
  logDoctorSteps: "วิธีแก้",
  logDoctorLogLines: "บรรทัด log ล่าสุด",
  logDoctorKnownError: "error ที่รู้จัก",
  logDoctorAiMatch: "AI วิเคราะห์",
  logDoctorAiReady: "โมเดล AI {name} ({params} พารามิเตอร์) พร้อมใช้ ทำงานออฟไลน์ในเครื่อง",
  logDoctorAiOff:
    "โหมด error ที่รู้จัก เปิด AI ออฟไลน์เพื่อให้เข้าใจข้อความ error ที่เขียนต่างออกไปด้วย",
  logDoctorEnableAi: "เปิดใช้ AI ({size})",
  logDoctorDownloading: "กำลังดาวน์โหลดโมเดล AI {percent}%",
  logDoctorRemoveAi: "ลบโมเดล AI",
  logDoctorAiTitle: "Log Doctor AI",
  logDoctorAiSettingsDescription:
    "โมเดลขนาดเล็กแบบออฟไลน์ (~8M พารามิเตอร์) ช่วยให้ Log Doctor เข้าใจข้อความ error ได้ดีขึ้น ไม่มีข้อมูลถูกส่งออกจากเครื่อง ถ้าไม่ติดตั้ง Log Doctor ยังตรวจ error ที่รู้จักได้ตามปกติ",
  logDoctorAiNotInstalled: "ยังไม่ติดตั้ง",
  doctorActionOpenSettings: "เปิด Settings",
  doctorActionOpenUrl: "เปิดหน้าดาวน์โหลด",
  doctorActionStartPhp: "Start PHP-FPM",
  doctorActionOpenMysqlData: "เปิดโฟลเดอร์ข้อมูล MySQL",
  doctorActionOpenPostgresqlData: "เปิดโฟลเดอร์ข้อมูล PostgreSQL",
  httpsPreview: "พรีวิว HTTPS",
  publicHttpsDomain: "โดเมน HTTPS ฟรีสำหรับทดสอบเว็บ",
  httpsTunnelDescription:
    "สร้างลิงก์ HTTPS สาธารณะอัตโนมัติผ่าน trycloudflare.com สำหรับทดสอบและแชร์งานชั่วคราว",
  httpsTunnelStart: "เริ่ม HTTPS",
  httpsTunnelStop: "หยุด HTTPS",
  httpsTunnelOpen: "เปิดลิงก์ HTTPS",
  httpsTunnelCopy: "คัดลอก",
  httpsTunnelStarting: "กำลังสร้าง HTTPS tunnel",
  httpsTunnelStarted: "สร้าง HTTPS tunnel แล้ว",
  httpsTunnelStopped: "หยุด HTTPS tunnel แล้ว",
  httpsTunnelWaitingForDomain: "กำลังรอโดเมนฟรีจาก Cloudflare...",
  httpsTunnelValidating: "ได้โดเมนแล้ว กำลังตรวจสอบ DNS และ HTTPS ให้พร้อมใช้งาน...",
  httpsTunnelError: "HTTPS tunnel ล้มเหลว",
  httpsTunnelDevOnly: "ลิงก์นี้เหมาะสำหรับทดสอบและพัฒนาเท่านั้น",
  terminal: "เทอร์มินัล",
  openTerminal: "เปิดเทอร์มินัลในโฟลเดอร์โปรเจกต์",
  openConfigFolder: "เปิดโฟลเดอร์ config",
  startAllServices: "เริ่มบริการทั้งหมด",
  stopAllServices: "หยุดบริการทั้งหมด",
  restartAllServices: "รีสตาร์ทบริการทั้งหมด",
  serviceStarted: "เริ่มบริการแล้ว",
  serviceStopped: "หยุดบริการแล้ว",
  serviceRestarted: "รีสตาร์ทบริการแล้ว",

  // Settings
  settingsTitle: "การตั้งค่า",
  settingsDescription: "พอร์ต เวอร์ชัน PHP เครื่องมือฐานข้อมูล Log Doctor และการเริ่มต้น",
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
  startup: "การเริ่มต้น",
  autoStartServices: "เริ่มบริการอัตโนมัติ",
  autoStartDescription: "เริ่มบริการทั้งหมดเมื่อเปิด CHAMP",
  language: "ภาษา",
  thai: "ไทย",
  soundEffects: "เสียงเอฟเฟกต์",
  enableSoundEffects: "เปิดใช้งานเสียงเอฟเฟกต์",
  soundEffectsDescription: "เล่นเสียงเมื่อกดปุ่มและดำเนินการต่างๆ",
  databaseToolSelect: "เครื่องมือฐานข้อมูล",
  webDatabaseManager: "ตัวจัดการฐานข้อมูลบนเว็บ",
  runtime: "รันไทม์",
  openRuntimeFolder: "เปิดโฟลเดอร์รันไทม์",
  sourceRepository: "ซอร์สโค้ดบน GitHub",
  stackStartingTitle: "กำลังเริ่มสแต็ก",
  stackStartingMessage: "CHAMP กำลังเริ่มสแต็กที่เลือก อาจใช้เวลาสักครู่",
  stackStartedTitle: "เริ่มสแต็กแล้ว",
  stackStartedMessage: "คำสั่งเริ่มสแต็กทำงานเสร็จแล้ว กำลังรีเฟรชสถานะ",
  stackRestartingTitle: "กำลังรีสตาร์ทสแต็ก",
  stackRestartingMessage: "บริการกำลังหยุดและเริ่มใหม่ แดชบอร์ดจะอัปเดตอัตโนมัติ",
  stackRestartedTitle: "รีสตาร์ทสแต็กแล้ว",
  stackRestartedMessage: "คำสั่งรีสตาร์ททำงานเสร็จแล้ว กำลังรีเฟรชสถานะ",
  stackStoppingTitle: "กำลังหยุดสแต็ก",
  stackStoppingMessage: "บริการกำลังปิดตัว อาจใช้เวลาสักครู่",
  stackStoppedTitle: "หยุดสแต็กแล้ว",
  stackStoppedMessage: "ส่งคำสั่งหยุดไปยังบริการทั้งหมดแล้ว",
  serviceStartingTitle: "กำลังเริ่มบริการ",
  serviceStartingMessage: "บริการกำลังเริ่ม สถานะจะอัปเดตอัตโนมัติ",
  serviceRestartingTitle: "กำลังรีสตาร์ทบริการ",
  serviceRestartingMessage: "บริการกำลังรีสตาร์ท สถานะจะอัปเดตอัตโนมัติ",
  serviceStoppingTitle: "กำลังหยุดบริการ",
  serviceStoppingMessage: "บริการกำลังหยุด สถานะจะอัปเดตอัตโนมัติ",
  dashboardRefreshingStatus: "แดชบอร์ดกำลังรีเฟรชสถานะบริการ",
  fallbackPortsUsed: "ใช้พอร์ตสำรอง: {ports}",
  commandFailed: "คำสั่งล้มเหลว",
  openFolderFailed: "เปิดโฟลเดอร์ไม่สำเร็จ",
  openTerminalFailed: "เปิดเทอร์มินัลไม่สำเร็จ",
  installedSuffix: "ติดตั้งแล้ว",
  installedVersionsLabel: "เวอร์ชันที่ติดตั้ง",
  settingsSaved: "บันทึกการตั้งค่าแล้ว",
  failedToLoadSettings: "โหลดการตั้งค่าไม่สำเร็จ",
  failedToSaveSettings: "บันทึกการตั้งค่าไม่สำเร็จ",
  phpVersionInstalled: "ติดตั้งเวอร์ชัน PHP แล้ว",
  activePhpChanged: "เปลี่ยน PHP ที่ใช้งานอยู่แล้ว",
  failedToInstallPhpVersion: "ติดตั้งเวอร์ชัน PHP ไม่สำเร็จ",
  failedToSwitchPhpVersion: "สลับเวอร์ชัน PHP ไม่สำเร็จ",
  databaseUsedWhenStartingStack: "จะใช้ {name} เมื่อเริ่มสแต็กหลัก",
  runtimeCatalogUpdateFailed: "ตรวจสอบเวอร์ชันรันไทม์ล่าสุดไม่สำเร็จ ใช้รายการในเครื่องแทน",
  ltsBadge: "LTS",
  stableBadge: "Stable",
  eolBadge: "EOL",
  debugMenu: "เมนูดีบัก",
  viewDownloadFolder: "เปิดโฟลเดอร์ดาวน์โหลด",
  resetInstallation: "รีเซ็ตการติดตั้ง",
  resetInstallationConfirm:
    "รีเซ็ตการติดตั้งหรือไม่ การทำงานนี้จะหยุดบริการทั้งหมดและลบไฟล์รันไทม์",
  resetFailed: "รีเซ็ตไม่สำเร็จ",
  openDownloadFolderFailed: "เปิดโฟลเดอร์ดาวน์โหลดไม่สำเร็จ",
  showFirstRunWizard: "แสดงตัวช่วยติดตั้งครั้งแรก",

  // Keyboard Shortcuts
  shortcutsTitle: "คีย์ลัดแป้นพิมพ์",
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
  copiedToClipboard: "คัดลอกไปยังคลิปบอร์ดแล้ว",
  copyFailed: "ไม่สามารถคัดลอกได้",

  // First Run Wizard
  welcome: "ยินดีต้อนรับ",
  downloading: "กำลังดาวน์โหลด",
  extracting: "กำลังแตกไฟล์",
  installationComplete: "การติดตั้งเสร็จสมบูรณ์",
  wizardRuntimeOverview: "ภาพรวมรันไทม์",
  wizardPackagesStep: "แพ็กเกจ",
  wizardChooseVersions: "เลือกเวอร์ชัน",
  wizardChecksStep: "ตรวจสอบ",
  wizardSystemReadiness: "ความพร้อมของระบบ",
  wizardReviewStep: "ตรวจทาน",
  wizardKeepOrReplace: "ใช้ของเดิมหรือแทนที่",
  wizardInstallStep: "ติดตั้ง",
  wizardDownloadRuntime: "ดาวน์โหลดรันไทม์",
  wizardStackDescription: "Caddy + HTTP(S) + phpMyAdmin/Adminer + MySQL/PostgreSQL + PHP",
  wizardWelcomeBody:
    "CHAMP จะติดตั้ง Caddy, PHP-FPM, ฐานข้อมูลที่เหมาะกับเครื่องมือที่เลือก และ phpMyAdmin หรือ Adminer ไว้ในโปรไฟล์ผู้ใช้ เพื่อให้สแต็กทำงานได้โดยไม่ต้องเขียนไฟล์ config ลงโฟลเดอร์ระบบที่ถูกป้องกัน",
  wizardUserSpaceRuntime: "รันไทม์ในพื้นที่ผู้ใช้",
  wizardUserSpaceRuntimeCopy: "ไม่ต้องใช้สิทธิ์แอดมินเพื่อเขียน config ที่สร้างขึ้น",
  wizardDevFirstPorts: "พอร์ตสำหรับนักพัฒนา",
  wizardDevFirstPortsCopy: "HTTP 8080, PHP 9000, MySQL 3306 หรือ PostgreSQL 5432",
  wizardDatabaseToolReady: "พร้อมใช้ phpMyAdmin",
  wizardDatabaseToolReadyCopy: "เครื่องมือฐานข้อมูลเปิดที่ /phpmyadmin เป็นค่าเริ่มต้น",
  wizardExistingDetected: "พบการติดตั้งเดิม",
  wizardEstimatedDownloadSize: "ขนาดดาวน์โหลดโดยประมาณ:",
  wizardVariesByPlatform: "ขึ้นอยู่กับแพลตฟอร์ม",
  wizardUseExisting: "ใช้ของเดิม",
  wizardDownloadFresh: "ดาวน์โหลดใหม่",
  wizardGetStarted: "เริ่มต้น",
  wizardReadUserManual: "อ่านคู่มือผู้ใช้",
  wizardPackageIntro: "เลือกเวอร์ชันของ PHP, เครื่องมือฐานข้อมูล และฐานข้อมูลหลักของเครื่องมือนั้น",
  wizardDownloadAndInstall: "ดาวน์โหลดและติดตั้ง",
  wizardMissingSystemDependencies: "ขาด dependency ของระบบ",
  wizardInstallCommand: "คำสั่งติดตั้งสำหรับดิสโทรของคุณ:",
  wizardDependencyRetryCopy:
    'หลังติดตั้ง dependency แล้ว ให้กด "ตรวจสอบอีกครั้ง" เพื่อดำเนินการต่อ',
  wizardRetryCheck: "ตรวจสอบอีกครั้ง",
  wizardInstallationSummary: "สรุปการติดตั้ง:",
  wizardKeepAndInstall: "เก็บไว้และติดตั้ง",
  wizardInstallAll: "ติดตั้งทั้งหมด",
  wizardDownloadVerb: "ดาวน์โหลด",
  wizardExtractVerb: "แตกไฟล์",
  wizardInstalling: "กำลังติดตั้ง...",
  wizardPreparing: "กำลังเตรียม...",
  wizardRuntimeInstalled: "ติดตั้งรันไทม์แล้ว",
  wizardReadyToStartStack: "CHAMP พร้อมเริ่มสแต็กเว็บในเครื่องแล้ว",
  wizardContinueToDashboard: "ไปที่แดชบอร์ด",
  wizardNoExistingInstallation:
    "ไม่พบการติดตั้งเดิม โปรดดาวน์โหลดคอมโพเนนต์รันไทม์เพื่อดำเนินการต่อ",
  wizardRecommended: "แนะนำ",
  wizardLoadingPackages: "กำลังโหลดแพ็กเกจที่มี...",
  wizardFailedToLoadPackages: "ไม่สามารถโหลดแพ็กเกจที่มีได้",
  wizardDefault: "ค่าเริ่มต้น",
  wizardNote: "หมายเหตุ",
  wizardEolNote:
    "เวอร์ชัน EOL ไม่ได้รับการซัพพอร์ตและอาจมีช่องโหว่ด้านความปลอดภัย PHP 5.5-7.3 ใช้ได้เฉพาะบน Windows",

  // Errors
  genericError: "เกิดข้อผิดพลาด",
  unknownError: "ข้อผิดพลาดที่ไม่รู้จัก",

  // Footer / Status
  ready: "พร้อม",
  processing: "กำลังประมวลผล",
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
  no: "No",
  retry: "Retry",
  remove: "Remove",
  refresh: "Refresh",
  more: "More",
  search: "Search",
  filter: "Filter",
  back: "Back",
  next: "Next",
  done: "Done",
  install: "Install",
  installed: "Installed",
  available: "Available",
  active: "Active",
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

  // App
  appName: "CHAMP",
  appDescription: "Web Server for Developers",

  // Dashboard
  dashboard: "Dashboard",
  services: "Services",
  php: "PHP",
  mysql: "MySQL",
  database: "Database",
  phpMyAdmin: "phpMyAdmin",
  adminer: "Adminer",
  caddy: "Caddy",
  projects: "Projects",
  logs: "Logs",
  website: "Website",
  databaseTool: "Database Tool",
  openWebsite: "Open Website",
  openDatabaseTool: "Open Database Tool",
  openProjectsFolder: "Open Projects Folder",
  openLogsFolder: "Open Logs Folder",
  network: "Network",
  phpUpdateTo: "Update to {version}",
  phpUpdateNow: "Update",
  phpUpdating: "Updating PHP",
  phpUpdated: "PHP updated to {version}",
  manual: "Guide",
  manualTitle: "CHAMP guide",
  manualSearch: "Search the guide",
  manualNoResults: "No topics match your search",
  manualContents: "Contents",
  stack: "stack",
  requestPath: "Request path",
  browser: "Browser",
  shortcuts: "Shortcuts",
  notInStack: "Not part of the current stack. Switch the database tool in Settings to use it.",
  agentActivityTitle: "AI agent used CHAMP",
  agentActivityDone: "{action}: {target}",
  updateAvailableTitle: "CHAMP v{version} is available",
  updateRestartNote: "All services stop while CHAMP restarts.",
  updateInstalling: "Downloading and installing. CHAMP restarts when it is done.",
  updateInstall: "Install and restart",
  updateLater: "Later",
  updatesTitle: "Updates",
  updateCheck: "Check for updates",
  updateChecking: "Checking...",
  updateCurrent: "You have the latest version.",
  updateLastChecked: "Last checked {time}",
  updateNeverChecked: "Not checked yet.",
  updateReleaseNotes: "What's new",
  updateFailed: "Update failed",
  autoCheckUpdates: "Check GitHub for updates once a day at startup",
  agentConnectorTitle: "AI agent connector",
  agentConnectorDescription:
    "Runs a local MCP server so AI coding agents such as Claude Code, Cursor, or Codex can check status, read logs, run Log Doctor, and start or stop services. It listens on 127.0.0.1 only and requires a token.",
  agentConnectorEnable: "Turn on the agent connector",
  agentConnectorRunning: "Running",
  agentConnectorStopped: "Not running",
  agentConnectorReadOnly: "Read-only",
  agentConnectorReadOnlyHint:
    "Agents can see status, logs, and diagnoses but cannot start or stop services.",
  agentConnectorControlHint:
    "Agents can start, stop, and restart services. Each action shows a notification on the dashboard.",
  agentConnectorPort: "Port",
  agentConnectorEndpoint: "Endpoint",
  agentConnectorClaudeCode: "Claude Code (run in a terminal)",
  agentConnectorJson: "Cursor and other agents (JSON)",
  agentConnectorCopy: "Copy",
  agentConnectorCopied: "Copied",
  agentConnectorTokenHint:
    "Copied commands include the secret token. Keep it private and create a new one if it leaks.",
  agentConnectorRegenerate: "Create new token",
  diagnose: "Diagnose",
  logDoctor: "Log Doctor",
  logDoctorDescription: "Reads service logs and explains the most likely cause and fix",
  logDoctorReading: "Reading logs...",
  logDoctorRecheck: "Check again",
  logDoctorNoMatch: "No known cause found",
  logDoctorNoMatchHint:
    "The lines below are the latest warnings in the log. Open the logs folder for the full files.",
  logDoctorNoLogs: "This service has no log yet. Start it, then check again.",
  logDoctorEvidence: "Matched log line",
  logDoctorSteps: "How to fix",
  logDoctorLogLines: "Recent log lines",
  logDoctorKnownError: "Known error",
  logDoctorAiMatch: "AI match",
  logDoctorAiReady: "AI model {name} ({params} parameters) is ready and runs offline",
  logDoctorAiOff: "Known-error mode. Enable the offline AI to also understand reworded errors.",
  logDoctorEnableAi: "Enable AI ({size})",
  logDoctorDownloading: "Downloading AI model {percent}%",
  logDoctorRemoveAi: "Remove AI model",
  logDoctorAiTitle: "Log Doctor AI",
  logDoctorAiSettingsDescription:
    "A small offline model (~8M parameters) that helps Log Doctor understand error messages. Nothing leaves your computer, and Log Doctor still recognises known errors without it.",
  logDoctorAiNotInstalled: "Not installed",
  doctorActionOpenSettings: "Open Settings",
  doctorActionOpenUrl: "Open download page",
  doctorActionStartPhp: "Start PHP-FPM",
  doctorActionOpenMysqlData: "Open MySQL data folder",
  doctorActionOpenPostgresqlData: "Open PostgreSQL data folder",
  httpsPreview: "HTTPS Preview",
  publicHttpsDomain: "Free HTTPS domain for website testing",
  httpsTunnelDescription:
    "Automatically creates a temporary public HTTPS link through trycloudflare.com for testing and sharing work.",
  httpsTunnelStart: "Start HTTPS",
  httpsTunnelStop: "Stop HTTPS",
  httpsTunnelOpen: "Open HTTPS link",
  httpsTunnelCopy: "Copy",
  httpsTunnelStarting: "Creating HTTPS tunnel",
  httpsTunnelStarted: "HTTPS tunnel ready",
  httpsTunnelStopped: "HTTPS tunnel stopped",
  httpsTunnelWaitingForDomain: "Waiting for a free Cloudflare domain...",
  httpsTunnelValidating: "Domain received. Validating DNS and HTTPS readiness...",
  httpsTunnelError: "HTTPS tunnel failed",
  httpsTunnelDevOnly: "This link is intended for testing and development only.",
  terminal: "Terminal",
  openTerminal: "Open Terminal in Projects",
  openConfigFolder: "Open config folder",
  startAllServices: "Start All Services",
  stopAllServices: "Stop All Services",
  restartAllServices: "Restart All Services",
  serviceStarted: "Service started",
  serviceStopped: "Service stopped",
  serviceRestarted: "Service restarted",

  // Settings
  settingsTitle: "Settings",
  settingsDescription: "Ports, PHP versions, database tool, Log Doctor, and startup",
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
  startup: "Startup",
  autoStartServices: "Start stack when CHAMP opens",
  autoStartDescription: "Automatically start all services when CHAMP opens",
  language: "Language",
  thai: "ไทย",
  soundEffects: "Sound Effects",
  enableSoundEffects: "Enable sound effects",
  soundEffectsDescription: "Play sounds when pressing buttons and performing actions",
  databaseToolSelect: "Database Tool",
  webDatabaseManager: "Web database manager",
  runtime: "Runtime",
  openRuntimeFolder: "Open Runtime Folder",
  sourceRepository: "Source repository",
  stackStartingTitle: "Starting stack",
  stackStartingMessage: "The selected CHAMP stack is starting. This can take a few seconds.",
  stackStartedTitle: "Stack started",
  stackStartedMessage: "All stack commands finished. Statuses are refreshing now.",
  stackRestartingTitle: "Restarting stack",
  stackRestartingMessage:
    "Services are stopping and starting again. The dashboard will update automatically.",
  stackRestartedTitle: "Stack restarted",
  stackRestartedMessage: "Restart command finished. Statuses are refreshing now.",
  stackStoppingTitle: "Stopping stack",
  stackStoppingMessage: "Services are shutting down. This can take a moment.",
  stackStoppedTitle: "Stack stopped",
  stackStoppedMessage: "All services have received the stop command.",
  serviceStartingTitle: "Starting service",
  serviceStartingMessage: "The service is starting. Status will update automatically.",
  serviceRestartingTitle: "Restarting service",
  serviceRestartingMessage: "The service is restarting. Status will update automatically.",
  serviceStoppingTitle: "Stopping service",
  serviceStoppingMessage: "The service is stopping. Status will update automatically.",
  dashboardRefreshingStatus: "The dashboard is refreshing service status.",
  fallbackPortsUsed: "Using fallback ports: {ports}.",
  commandFailed: "Command failed",
  openFolderFailed: "Failed to open folder",
  openTerminalFailed: "Failed to open terminal",
  installedSuffix: "installed",
  installedVersionsLabel: "Installed versions",
  settingsSaved: "Settings saved",
  failedToLoadSettings: "Failed to load settings",
  failedToSaveSettings: "Failed to save settings",
  phpVersionInstalled: "PHP version installed",
  activePhpChanged: "Active PHP version changed",
  failedToInstallPhpVersion: "Failed to install PHP version",
  failedToSwitchPhpVersion: "Failed to switch PHP version",
  databaseUsedWhenStartingStack: "{name} will be used when starting the main stack.",
  runtimeCatalogUpdateFailed: "Could not check latest runtime versions. Using local catalog.",
  ltsBadge: "LTS",
  stableBadge: "Stable",
  eolBadge: "EOL",
  debugMenu: "Debug Menu",
  viewDownloadFolder: "View Download Folder",
  resetInstallation: "Reset Installation",
  resetInstallationConfirm:
    "Reset installation? This will stop all services and delete runtime binaries.",
  resetFailed: "Failed to reset",
  openDownloadFolderFailed: "Failed to open download folder",
  showFirstRunWizard: "Show First-Run Wizard",

  // Keyboard Shortcuts
  shortcutsTitle: "Keyboard Shortcuts",
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
  copiedToClipboard: "Copied to clipboard",
  copyFailed: "Failed to copy",

  // First Run Wizard
  welcome: "Welcome",
  downloading: "Downloading",
  extracting: "Extracting",
  installationComplete: "Installation complete",
  wizardRuntimeOverview: "Runtime overview",
  wizardPackagesStep: "Packages",
  wizardChooseVersions: "Choose versions",
  wizardChecksStep: "Checks",
  wizardSystemReadiness: "System readiness",
  wizardReviewStep: "Review",
  wizardKeepOrReplace: "Keep or replace",
  wizardInstallStep: "Install",
  wizardDownloadRuntime: "Download runtime",
  wizardStackDescription: "Caddy + HTTP(S) + phpMyAdmin/Adminer + MySQL/PostgreSQL + PHP",
  wizardWelcomeBody:
    "CHAMP installs Caddy, PHP-FPM, the database that matches your selected tool, and phpMyAdmin or Adminer into your user profile so the stack can run without writing config files into protected system folders.",
  wizardUserSpaceRuntime: "User-space runtime",
  wizardUserSpaceRuntimeCopy: "No admin write access for generated configs.",
  wizardDevFirstPorts: "Dev-first ports",
  wizardDevFirstPortsCopy: "HTTP 8080, PHP 9000, MySQL 3306 or PostgreSQL 5432.",
  wizardDatabaseToolReady: "phpMyAdmin ready",
  wizardDatabaseToolReadyCopy: "Database tools open at /phpmyadmin by default.",
  wizardExistingDetected: "Existing installation detected!",
  wizardEstimatedDownloadSize: "Estimated download size:",
  wizardVariesByPlatform: "varies by platform",
  wizardUseExisting: "Use Existing",
  wizardDownloadFresh: "Download Fresh",
  wizardGetStarted: "Get Started",
  wizardReadUserManual: "Read User Manual",
  wizardPackageIntro:
    "Select the versions of PHP, the database tool, and the primary database for that tool.",
  wizardDownloadAndInstall: "Download & Install",
  wizardMissingSystemDependencies: "Missing System Dependencies",
  wizardInstallCommand: "Install command for your distribution:",
  wizardDependencyRetryCopy: 'After installing the dependencies, click "Retry Check" to continue.',
  wizardRetryCheck: "Retry Check",
  wizardInstallationSummary: "Installation summary:",
  wizardKeepAndInstall: "Keep & Install",
  wizardInstallAll: "Install All",
  wizardDownloadVerb: "Download",
  wizardExtractVerb: "Extract",
  wizardInstalling: "Installing...",
  wizardPreparing: "Preparing...",
  wizardRuntimeInstalled: "Runtime installed",
  wizardReadyToStartStack: "CHAMP is ready to start the local web stack.",
  wizardContinueToDashboard: "Continue to Dashboard",
  wizardNoExistingInstallation:
    "No existing installation found. Please download the runtime components to continue.",
  wizardRecommended: "Recommended",
  wizardLoadingPackages: "Loading available packages...",
  wizardFailedToLoadPackages: "Failed to load available packages",
  wizardDefault: "Default",
  wizardNote: "Note",
  wizardEolNote:
    "EOL versions are unsupported and may have security vulnerabilities. PHP 5.5-7.3 are available on Windows only.",

  // Errors
  genericError: "An error occurred",
  unknownError: "Unknown error",

  // Footer / Status
  ready: "Ready",
  processing: "Processing",
};

export const translations: Record<Language, Translations> = { th, en };

export function getTranslation(lang: Language): Translations {
  return translations[lang];
}
