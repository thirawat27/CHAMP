/**
 * In-app user guide content (Thai and English).
 *
 * Text may contain `inline code` in backticks. Keep every topic in sync with
 * the real behaviour of the app; the guide is the only user manual.
 */

import {
  ArrowUpCircle,
  Bot,
  Database,
  FileCode2,
  Folder,
  FolderTree,
  Keyboard,
  LayoutDashboard,
  LifeBuoy,
  Lock,
  LucideIcon,
  Play,
  Rocket,
  Settings2,
  ShieldCheck,
  Stethoscope,
  TerminalSquare,
} from "lucide-react";

export interface Localized {
  th: string;
  en: string;
}

export type GuideBlock =
  | { kind: "p"; text: Localized }
  | { kind: "steps"; items: Localized[] }
  | { kind: "list"; items: Localized[] }
  | { kind: "code"; label?: Localized; code: string }
  | { kind: "tip"; text: Localized }
  | { kind: "warning"; text: Localized }
  | { kind: "table"; head: Array<Localized | string>; rows: Array<Array<Localized | string>> }
  | { kind: "keys"; items: Array<{ keys: string; text: Localized }> };

export interface GuideSection {
  id: string;
  icon: LucideIcon;
  title: Localized;
  summary: Localized;
  blocks: GuideBlock[];
}

const l = (th: string, en: string): Localized => ({ th, en });

export const GUIDE_SECTIONS: GuideSection[] = [
  {
    id: "start",
    icon: Rocket,
    title: l("เริ่มต้นใช้งาน", "Getting started"),
    summary: l(
      "CHAMP คืออะไร และติดตั้งครั้งแรกอย่างไร",
      "What CHAMP is and how the first setup works"
    ),
    blocks: [
      {
        kind: "p",
        text: l(
          "CHAMP คือชุดเว็บเซิร์ฟเวอร์สำหรับพัฒนาเว็บบนเครื่องตัวเอง ประกอบด้วย Caddy (เว็บเซิร์ฟเวอร์), PHP-FPM, MySQL หรือ PostgreSQL และ phpMyAdmin หรือ Adminer ทุกอย่างติดตั้งในโฟลเดอร์ผู้ใช้ ไม่ต้องใช้สิทธิ์ Administrator และไม่แตะไฟล์ระบบ",
          "CHAMP is a local web development stack: Caddy (web server), PHP-FPM, MySQL or PostgreSQL, and phpMyAdmin or Adminer. Everything installs in your user folder, needs no administrator rights, and leaves system files alone."
        ),
      },
      {
        kind: "steps",
        items: [
          l(
            "เปิด CHAMP ครั้งแรก จะเจอหน้าตั้งค่า (Setup wizard) กด เริ่มต้นใช้งาน",
            "Open CHAMP. The setup wizard appears; choose Get started."
          ),
          l(
            "เลือกเวอร์ชัน PHP, ฐานข้อมูล (MySQL หรือ PostgreSQL ทำหน้าที่เดียวกัน เลือกได้ตัวเดียว) พร้อมเวอร์ชัน และตัวจัดการเว็บ (phpMyAdmin ใช้ได้กับ MySQL เท่านั้น ส่วน Adminer ใช้ได้ทั้งสองแบบ) ค่าเริ่มต้นคือเวอร์ชันล่าสุดเสมอ",
            "Pick a PHP version, a database (MySQL or PostgreSQL fill the same role; choose one) and its version, and a web manager (phpMyAdmin works with MySQL only; Adminer works with both). The defaults are always the newest releases."
          ),
          l(
            "CHAMP ตรวจ library ที่ระบบต้องมี (สำคัญบน Linux) แล้วดาวน์โหลดทุกส่วนพร้อมกัน",
            "CHAMP checks required system libraries (mainly on Linux), then downloads every component in parallel."
          ),
          l(
            "เสร็จแล้วกด ไปที่หน้าหลัก แล้วกด เริ่ม (Ctrl+S) เพื่อเปิดทั้ง stack",
            "When it finishes, open the dashboard and press Start (Ctrl+S) to run the whole stack."
          ),
          l(
            "เปิด http://localhost:8080 ในเบราว์เซอร์ จะเห็นหน้าเริ่มต้นของ CHAMP ที่บอกที่อยู่โฟลเดอร์ projects และลิงก์ไปเว็บไซต์ที่มีอยู่",
            "Open http://localhost:8080 in your browser to see CHAMP's start page with your projects folder and links to your sites."
          ),
        ],
      },
      {
        kind: "tip",
        text: l(
          "ถ้าเคยติดตั้งไว้แล้ว หน้า wizard จะมีปุ่ม ใช้ของที่มีอยู่ เพื่อข้ามการดาวน์โหลด",
          "If components are already installed, the wizard offers Use existing to skip the download."
        ),
      },
    ],
  },
  {
    id: "dashboard",
    icon: LayoutDashboard,
    title: l("หน้าหลัก", "The dashboard"),
    summary: l("อ่านสถานะทั้ง stack ได้ในพริบตา", "Read the whole stack at a glance"),
    blocks: [
      {
        kind: "p",
        text: l(
          "แถบ เส้นทาง request ด้านบนแสดงทางที่ request เดินจากเบราว์เซอร์ → Caddy → PHP-FPM → ฐานข้อมูล เส้นเชื่อมจะเรืองแสงเมื่อทั้งสองฝั่งทำงาน ถ้าจุดไหนเป็นสีแดง แปลว่า request จะหยุดตรงนั้น",
          "The request path at the top shows how a request travels: browser → Caddy → PHP-FPM → database. A link glows when both ends are running; a red node is where requests stop."
        ),
      },
      {
        kind: "table",
        head: [l("ไฟสถานะ", "Status light"), l("ความหมาย", "Meaning")],
        rows: [
          [l("เขียว", "Green"), l("ทำงานอยู่", "Running")],
          [
            l("เหลืองกะพริบ", "Blinking amber"),
            l("กำลังเริ่มหรือกำลังหยุด", "Starting or stopping"),
          ],
          [
            l("แดง", "Red"),
            l("เกิดข้อผิดพลาด กด วิเคราะห์ เพื่อดูสาเหตุ", "Error; press Diagnose to see why"),
          ],
          [l("วงกลมว่าง", "Empty ring"), l("หยุดอยู่", "Stopped")],
        ],
      },
      {
        kind: "list",
        items: [
          l(
            "การ์ดบริการแสดงพอร์ตตัวใหญ่ ที่อยู่สำหรับเชื่อมต่อ ข้อความ error ล่าสุด และปุ่มควบคุม",
            "Each service card shows its port, its connection address, the last error, and its controls."
          ),
          l(
            "ฐานข้อมูลที่ไม่ได้อยู่ใน stack ปัจจุบันจะจางลง (เช่น PostgreSQL เมื่อใช้ phpMyAdmin)",
            "The database that is not part of the current stack is dimmed (for example PostgreSQL while you use phpMyAdmin)."
          ),
          l(
            "แถบขวามี HTTPS Preview, ทางลัด และเวอร์ชันที่ติดตั้ง",
            "The right column holds HTTPS Preview, shortcuts, and installed versions."
          ),
          l(
            "แถบล่างแสดงจำนวนบริการที่ทำงาน โฟลเดอร์ข้อมูล และ CPU / RAM / เครือข่ายของเครื่อง",
            "The bottom bar shows running services, the data folder, and CPU / RAM / network usage."
          ),
        ],
      },
    ],
  },
  {
    id: "services",
    icon: Play,
    title: l("เปิด ปิด และรีสตาร์ทบริการ", "Starting and stopping services"),
    summary: l("ควบคุมทั้ง stack หรือทีละบริการ", "Control the whole stack or one service"),
    blocks: [
      {
        kind: "list",
        items: [
          l(
            "ปุ่ม เริ่ม / รีสตาร์ท / หยุด ที่แถบบนควบคุมทั้ง stack (Caddy + PHP-FPM + ฐานข้อมูลที่เลือก)",
            "Start / Restart / Stop in the title bar control the whole stack (Caddy + PHP-FPM + the selected database)."
          ),
          l("ปุ่มบนการ์ดควบคุมทีละบริการ", "Buttons on a card control that service only."),
          l(
            "ถ้าพอร์ตถูกใช้อยู่ CHAMP จะหาพอร์ตว่างถัดไปให้อัตโนมัติและแจ้งพอร์ตใหม่ (บันทึกไว้ใน logs/port-fallback.log)",
            "If a port is busy, CHAMP picks the next free one automatically and tells you the new port (also written to logs/port-fallback.log)."
          ),
          l(
            "การปิดหน้าต่างไม่ได้หยุดบริการ CHAMP ยังอยู่ที่ถาดระบบ ถ้าจะปิดจริงให้เลือก Quit CHAMP จากไอคอนถาดระบบ",
            "Closing the window keeps services running in the system tray. Choose Quit CHAMP from the tray icon to stop everything."
          ),
          l(
            "เปิด เริ่มบริการอัตโนมัติ ใน Settings ถ้าต้องการให้ stack ทำงานทันทีที่เปิด CHAMP",
            "Turn on auto-start in Settings to run the stack as soon as CHAMP opens."
          ),
        ],
      },
    ],
  },
  {
    id: "projects",
    icon: Folder,
    title: l("โปรเจกต์และเว็บไซต์", "Projects and websites"),
    summary: l("วางไฟล์ตรงไหน และเปิดที่ URL อะไร", "Where files go and which URL serves them"),
    blocks: [
      {
        kind: "p",
        text: l(
          "ทุกโฟลเดอร์ในโฟลเดอร์ projects คือหนึ่งเว็บไซต์ เปิดได้ที่ `http://localhost:8080/<ชื่อโฟลเดอร์>/` กด Ctrl+O เพื่อเปิดโฟลเดอร์ projects",
          "Every folder inside the projects folder is a website served at `http://localhost:8080/<folder>/`. Press Ctrl+O to open the projects folder."
        ),
      },
      {
        kind: "code",
        label: l("ตัวอย่างโครงสร้าง", "Example layout"),
        code: "projects/\n  shop/\n    index.php      → http://localhost:8080/shop/\n  blog/\n    index.html     → http://localhost:8080/blog/",
      },
      {
        kind: "tip",
        text: l(
          "หน้า http://localhost:8080/ แสดงรายการเว็บไซต์ทุกโฟลเดอร์ที่มี index.html หรือ index.php ถ้าอยากให้หน้าแรกเป็นเว็บของคุณเอง ให้วาง index.html หรือ index.php ไว้ในโฟลเดอร์ projects โดยตรง หน้านี้จะถูกแทนที่ทันที",
          "http://localhost:8080/ lists every folder that has an index.html or index.php. To make the root your own site, put an index.html or index.php directly in the projects folder; it replaces the start page immediately."
        ),
      },
      {
        kind: "p",
        text: l(
          "CHAMP แปลงกฎ .htaccess ที่ใช้บ่อย (rewrite ไป index.php, DirectoryIndex, ErrorDocument, URL ไม่มี .php) เป็นกฎของ Caddy ให้อัตโนมัติ เฟรมเวิร์กส่วนใหญ่จึงใช้ได้ทันที",
          "CHAMP converts common .htaccess rules (front-controller rewrites to index.php, DirectoryIndex, ErrorDocument, extensionless .php URLs) into Caddy rules, so most frameworks work as they are."
        ),
      },
      {
        kind: "steps",
        items: [
          l(
            "WordPress: แตกไฟล์ไว้ที่ projects/wordpress",
            "WordPress: extract it to projects/wordpress."
          ),
          l(
            "เปิด phpMyAdmin (Ctrl+D) แล้วสร้างฐานข้อมูล เช่น wordpress",
            "Open phpMyAdmin (Ctrl+D) and create a database such as wordpress."
          ),
          l(
            "เปิด http://localhost:8080/wordpress/ แล้วกรอก host `127.0.0.1:3306` ผู้ใช้ `root` รหัสผ่านว่าง",
            "Open http://localhost:8080/wordpress/ and enter host `127.0.0.1:3306`, user `root`, and an empty password."
          ),
        ],
      },
      {
        kind: "tip",
        text: l(
          "Laravel: เปิดที่ `http://localhost:8080/<โปรเจกต์>/public/` และตั้ง `DB_HOST=127.0.0.1` ใน .env",
          "Laravel: open `http://localhost:8080/<project>/public/` and set `DB_HOST=127.0.0.1` in .env."
        ),
      },
    ],
  },
  {
    id: "databases",
    icon: Database,
    title: l("ฐานข้อมูล", "Databases"),
    summary: l(
      "ข้อมูลเชื่อมต่อ MySQL / PostgreSQL และการ import",
      "MySQL / PostgreSQL credentials and imports"
    ),
    blocks: [
      {
        kind: "table",
        head: [l("ค่า", "Setting"), "MySQL", "PostgreSQL"],
        rows: [
          [l("Host", "Host"), "127.0.0.1", "127.0.0.1"],
          [l("พอร์ต", "Port"), "3306", "5432"],
          [l("ผู้ใช้", "User"), "root", "postgres"],
          [
            l("รหัสผ่าน", "Password"),
            l("(ว่าง)", "(empty)"),
            l("(ว่าง, trust เฉพาะในเครื่อง)", "(empty, local trust)"),
          ],
          [l("เครื่องมือเว็บ", "Web tool"), "/phpmyadmin, /adminer", "/adminer"],
        ],
      },
      {
        kind: "p",
        text: l(
          "สลับฐานข้อมูล (MySQL | PostgreSQL) และตัวจัดการเว็บ (phpMyAdmin | Adminer) ได้จากการ์ดฐานข้อมูลบนหน้าหลักหรือใน Settings phpMyAdmin ใช้ได้กับ MySQL เท่านั้น ถ้าเลือก PostgreSQL จะมีแค่ Adminer ให้เลือก ถ้าตัวที่เลือกยังไม่ได้ติดตั้ง CHAMP จะดาวน์โหลดให้ เมื่อเปิด Adminer จากเครื่องนี้ ระบบจะกรอก server, user และรหัสผ่านว่างแล้วล็อกอินให้อัตโนมัติ (ไม่ทำงานผ่าน HTTPS Preview)",
          "Switch the database (MySQL | PostgreSQL) and the web manager (phpMyAdmin | Adminer) from the database card on the dashboard or in Settings. phpMyAdmin works with MySQL only, so PostgreSQL offers Adminer alone. CHAMP downloads the selected one if it is missing. Opened from this computer, Adminer fills in the server, user, and empty password and signs in automatically (not through HTTPS Preview)."
        ),
      },
      {
        kind: "code",
        label: l("เชื่อมต่อจาก PHP (PDO)", "Connect from PHP (PDO)"),
        code: "$mysql = new PDO('mysql:host=127.0.0.1;port=3306;dbname=app;charset=utf8mb4', 'root', '');\n$pgsql = new PDO('pgsql:host=127.0.0.1;port=5432;dbname=app', 'postgres', '');",
      },
      {
        kind: "warning",
        text: l(
          "ใช้ `127.0.0.1` แทน `localhost` เสมอ บน macOS/Linux คำว่า localhost จะทำให้ PHP ไปหาไฟล์ socket แทน TCP แล้วเชื่อมต่อไม่ได้",
          "Always use `127.0.0.1` instead of `localhost`. On macOS/Linux, localhost makes PHP look for a socket file instead of TCP and the connection fails."
        ),
      },
      {
        kind: "code",
        label: l(
          "Import ไฟล์ใหญ่ผ่าน Terminal (Ctrl+T)",
          "Import a large dump from the terminal (Ctrl+T)"
        ),
        code: "mysql -h 127.0.0.1 -P 3306 -u root app < dump.sql\npsql -h 127.0.0.1 -p 5432 -U postgres app < dump.sql",
      },
    ],
  },
  {
    id: "php",
    icon: FileCode2,
    title: l("เวอร์ชัน PHP", "PHP versions"),
    summary: l("ติดตั้ง อัปเดต และสลับ PHP", "Install, update, and switch PHP"),
    blocks: [
      {
        kind: "p",
        text: l(
          "CHAMP ดึงรายการ PHP จาก php.net และ windows.php.net โดยตรง จึงได้ patch ล่าสุดของแต่ละสาขาเสมอ (เช่น 8.5.x) บน Windows ไฟล์ทุกตัวถูกตรวจ SHA-256 ก่อนติดตั้ง สาขาที่หมดการดูแลแล้วยังเลือกได้สำหรับโปรเจกต์เก่าแต่จะติดป้าย EOL",
          "CHAMP reads PHP releases straight from php.net and windows.php.net, so each branch (for example 8.5.x) always installs its newest patch release. On Windows every download is checked against its SHA-256. Unsupported branches stay available for legacy projects and are marked EOL."
        ),
      },
      {
        kind: "steps",
        items: [
          l(
            "เปิด Settings → เวอร์ชัน PHP แล้วเลือกเวอร์ชัน",
            "Open Settings → PHP versions and choose a version."
          ),
          l(
            "กด ติดตั้งที่เลือก (การติดตั้งไม่เปลี่ยน PHP ที่ใช้อยู่)",
            "Press Install selected (installing does not change the active PHP)."
          ),
          l(
            "กด สลับ PHP เพื่อใช้เวอร์ชันนั้น CHAMP จะรีสตาร์ท PHP-FPM และ Caddy ให้",
            "Press Switch PHP to use it. CHAMP restarts PHP-FPM and Caddy for you."
          ),
        ],
      },
      {
        kind: "p",
        text: l(
          "เมื่อ PHP ที่ใช้อยู่มี patch ใหม่ หน้าหลักจะขึ้น PHP 8.5.7 → 8.5.12 พร้อมปุ่ม อัปเดต CHAMP จะหยุด PHP ชั่วคราว ติดตั้งเวอร์ชันใหม่ แล้วเปิดกลับให้",
          "When the active PHP has a newer patch release, the dashboard shows PHP 8.5.7 → 8.5.12 with an Update button. CHAMP pauses PHP, installs the new build, and starts it again."
        ),
      },
      {
        kind: "tip",
        text: l(
          "php.ini ถูกสร้างใหม่ทุกครั้งที่เริ่ม PHP (memory_limit 256M, upload 100M, max_execution_time 300) ถ้าต้องการค่าอื่นเฉพาะสคริปต์ ให้ใช้ `ini_set()` ในโค้ด หรือรัน `php -d memory_limit=-1 script.php` ใน Terminal",
          "php.ini is regenerated every time PHP starts (memory_limit 256M, uploads 100M, max_execution_time 300). For other values in one script use `ini_set()`, or run `php -d memory_limit=-1 script.php` in the terminal."
        ),
      },
    ],
  },
  {
    id: "terminal",
    icon: TerminalSquare,
    title: l("Terminal", "Terminal"),
    summary: l("คำสั่ง php / mysql / psql พร้อมใช้", "php, mysql, and psql ready to use"),
    blocks: [
      {
        kind: "p",
        text: l(
          "กด Ctrl+T หรือ ทางลัด → Terminal จะเปิดหน้าต่างคำสั่งในโฟลเดอร์ projects โดยใส่โฟลเดอร์ของ PHP, MySQL และ PostgreSQL ไว้ใน PATH แล้ว",
          "Press Ctrl+T or Shortcuts → Terminal to open a terminal in the projects folder with PHP, MySQL, and PostgreSQL on PATH."
        ),
      },
      {
        kind: "code",
        code: "php -v\nphp -S 127.0.0.1:8000        # PHP built-in server\nmysql -h 127.0.0.1 -u root\npsql -h 127.0.0.1 -U postgres",
      },
      {
        kind: "tip",
        text: l(
          "Composer ไม่ได้ติดมากับ CHAMP ติดตั้งได้จากเว็บ getcomposer.org แล้วใช้ PHP ของ CHAMP รัน",
          "Composer is not bundled. Install it from getcomposer.org; it runs with CHAMP's PHP."
        ),
      },
    ],
  },
  {
    id: "https",
    icon: ShieldCheck,
    title: l("HTTPS Preview", "HTTPS Preview"),
    summary: l(
      "ลิงก์ HTTPS สาธารณะชั่วคราวสำหรับทดสอบ",
      "A temporary public HTTPS link for testing"
    ),
    blocks: [
      {
        kind: "p",
        text: l(
          "HTTPS Preview ใช้ ngrok สร้างลิงก์ `https://….ngrok-free.app` ที่ชี้มาที่เว็บในเครื่อง เหมาะกับทดสอบบนมือถือ ส่งให้ลูกค้าดู หรือทดสอบ webhook",
          "HTTPS Preview uses ngrok to create an `https://….ngrok-free.app` link to your local site. Use it to test on a phone, show a client, or receive webhooks."
        ),
      },
      {
        kind: "steps",
        items: [
          l(
            "ครั้งแรก สมัครบัญชี ngrok ฟรี คัดลอก authtoken แล้ววางในช่อง ngrok authtoken ของ HTTPS Preview แล้วกด บันทึก",
            "The first time, sign up for a free ngrok account, copy your authtoken, paste it into the ngrok authtoken field in HTTPS Preview, and press Save."
          ),
          l(
            "กด เริ่ม HTTPS (CHAMP จะเปิด stack ให้ถ้ายังไม่ได้เปิด)",
            "Press Start HTTPS (CHAMP starts the stack first if needed)."
          ),
          l(
            "รอจนตรวจโดเมนเสร็จ ปุ่ม เปิด และ คัดลอก จะขึ้นเมื่อลิงก์ใช้งานได้จริง",
            "Wait for the domain check. Open and Copy appear once the link really works."
          ),
          l(
            "กด หยุด HTTPS เมื่อใช้เสร็จ บัญชีฟรีของ ngrok ให้โดเมนประจำบัญชี และผู้เข้าชมผ่านเบราว์เซอร์จะเห็นหน้าเตือนของ ngrok หนึ่งครั้งก่อนเข้าเว็บ",
            "Press Stop HTTPS when you are done. A free ngrok account gets one domain of its own, and browser visitors see an ngrok warning page once before your site."
          ),
        ],
      },
      {
        kind: "warning",
        text: l(
          "ทุกคนที่มีลิงก์เข้าเว็บในเครื่องคุณได้ รวมถึง phpMyAdmin/Adminer อย่าเปิดทิ้งไว้ และอย่าใช้กับข้อมูลจริง",
          "Anyone with the link can reach your local site, including phpMyAdmin/Adminer. Do not leave it running or use it with real data."
        ),
      },
    ],
  },
  {
    id: "doctor",
    icon: Stethoscope,
    title: l("Log Doctor", "Log Doctor"),
    summary: l(
      "บอกสาเหตุที่บริการล้ม พร้อมวิธีแก้",
      "Explains why a service failed and how to fix it"
    ),
    blocks: [
      {
        kind: "p",
        text: l(
          "เมื่อการ์ดบริการเป็นสีแดง กด วิเคราะห์ (หรือ ทางลัด → Log Doctor) CHAMP จะอ่าน log ล่าสุด เทียบกับคลังปัญหาที่รู้จักกว่า 25 แบบ แล้วแสดงสาเหตุ ขั้นตอนแก้ บรรทัด log ที่เป็นหลักฐาน และปุ่มแก้ด่วน เช่น เปิด Settings หรือ Start PHP-FPM",
          "When a card turns red, press Diagnose (or Shortcuts → Log Doctor). CHAMP reads the latest logs, compares them with more than 25 known problems, and shows the cause, fix steps, the matching log line, and a quick action such as Open Settings or Start PHP-FPM."
        ),
      },
      {
        kind: "table",
        head: [l("ป้าย", "Badge"), l("ความหมาย", "Meaning")],
        rows: [
          [
            l("error ที่รู้จัก", "Known error"),
            l(
              "ข้อความ error ตรงกับปัญหาที่รู้จักทุกตัวอักษร",
              "The log matches a known error message exactly"
            ),
          ],
          [
            l("AI วิเคราะห์", "AI match"),
            l(
              "โมเดล AI จับความหมายได้ว่าใกล้เคียงปัญหานี้",
              "The AI model recognised a reworded description of this problem"
            ),
          ],
        ],
      },
      {
        kind: "p",
        text: l(
          "โมเดล AI (potion-base-8M ประมาณ 8 ล้านพารามิเตอร์ 30 MB) เป็นตัวเลือกเสริม เปิดได้จากหน้า Log Doctor หรือ Settings ทำงานออฟไลน์ทั้งหมด ไม่มีข้อมูลออกจากเครื่อง ไม่เปิดก็ยังตรวจ error ที่รู้จักได้ตามปกติ",
          "The AI model (potion-base-8M, about 8 million parameters, 30 MB) is optional. Turn it on from Log Doctor or Settings. It runs fully offline and nothing leaves your computer; without it, known errors are still recognised."
        ),
      },
    ],
  },
  {
    id: "agent",
    icon: Bot,
    title: l("AI Agent Connector (MCP)", "AI agent connector (MCP)"),
    summary: l(
      "ให้ Claude Code, Codex, Cursor, VS Code และ Claude Desktop ใช้ CHAMP",
      "Let Claude Code, Codex, Cursor, VS Code, and Claude Desktop use CHAMP"
    ),
    blocks: [
      {
        kind: "p",
        text: l(
          "CHAMP มี MCP server ในตัว เมื่อเปิด AI coding agent จะรู้พอร์ตฐานข้อมูล URL และโฟลเดอร์ของคุณ ได้บรรทัด .env พร้อมใช้ อ่าน log ใช้ Log Doctor สร้างฐานข้อมูล สั่งเปิด/ปิดบริการ และเปิด HTTPS Preview ได้เอง",
          "CHAMP includes an MCP server. Once on, AI coding agents can learn your database ports, URLs, and folders, get ready-made .env lines, read logs, run Log Doctor, create databases, start or stop services, and open HTTPS Preview."
        ),
      },
      {
        kind: "steps",
        items: [
          l("Settings → AI Agent Connector → เปิด", "Settings → AI agent connector → turn it on."),
          l(
            "1. เลือกโปรแกรม AI ที่ใช้ (Claude Code, Codex, Cursor, VS Code หรือ Claude Desktop)",
            "1. Pick your AI tool (Claude Code, Codex, Cursor, VS Code, or Claude Desktop)."
          ),
          l(
            "2. กด คัดลอก แล้วรันใน terminal หรือวางในไฟล์ที่บอกไว้ จากนั้นเปิดโปรแกรม AI ใหม่",
            "2. Press Copy, then run it in a terminal or paste it into the file shown, and restart the AI tool."
          ),
          l(
            "3. กด ทดสอบการเชื่อมต่อ ถ้าผ่านจะเห็นจำนวน tools ที่ agent ใช้ได้",
            "3. Press Test connection. On success it lists the tools agents can use."
          ),
          l(
            'ถาม agent ได้เลย เช่น "สร้างฐานข้อมูล my_app แล้วต่อ Laravel ให้หน่อย" หรือ "ทำไม MySQL เปิดไม่ขึ้น"',
            'Ask the agent, for example "create a my_app database and connect my Laravel app" or "why won\'t MySQL start?"'
          ),
        ],
      },
      {
        kind: "code",
        label: l("ตัวอย่างคำสั่ง Claude Code", "Claude Code command"),
        code: 'claude mcp add --transport http --scope user champ http://127.0.0.1:47823/mcp --header "Authorization: Bearer <token>"',
      },
      {
        kind: "table",
        head: [l("เครื่องมือ", "Tool"), l("ทำอะไร", "What it does")],
        rows: [
          [
            "get_status",
            l("สถานะ พอร์ต และ error ของทุกบริการ", "State, port, and error of every service"),
          ],
          [
            "get_environment",
            l(
              "ข้อมูลเชื่อมต่อฐานข้อมูล URL โฟลเดอร์ และเวอร์ชัน",
              "Database credentials, URLs, folders, and versions"
            ),
          ],
          ["list_projects", l("รายชื่อโปรเจกต์พร้อม URL", "Projects with their URLs")],
          ["read_logs", l("อ่าน log ล่าสุดของบริการ", "Recent log lines of a service")],
          ["diagnose_service", l("รัน Log Doctor", "Run Log Doctor")],
          [
            "list_databases",
            l("รายชื่อฐานข้อมูลบน MySQL/PostgreSQL", "Databases on MySQL/PostgreSQL"),
          ],
          [
            "get_https_preview",
            l("สถานะและ URL ของ HTTPS Preview", "HTTPS Preview status and URL"),
          ],
          [
            "start/stop/restart_*",
            l(
              "ควบคุมบริการ (ปิดได้ด้วยโหมดอ่านอย่างเดียว)",
              "Control services (disabled in read-only mode)"
            ),
          ],
          [
            "create_database",
            l(
              "สร้างฐานข้อมูลเปล่า ไม่ลบหรือเขียนทับของเดิม (ปิดในโหมดอ่านอย่างเดียว)",
              "Create an empty database; never drops or overwrites (disabled in read-only mode)"
            ),
          ],
          [
            "start/stop_https_preview",
            l(
              "เปิด/ปิด URL สาธารณะผ่าน ngrok (ปิดในโหมดอ่านอย่างเดียว)",
              "Open or close the public ngrok URL (disabled in read-only mode)"
            ),
          ],
        ],
      },
      {
        kind: "warning",
        text: l(
          "server ฟังเฉพาะ 127.0.0.1 ต้องมี token และไม่รับ request จากหน้าเว็บ token อยู่ในคำสั่งที่คัดลอก ถ้าหลุดให้กด สร้าง token ใหม่ ทุกครั้งที่ agent สั่งงานจะมีแจ้งเตือนในหน้าหลัก",
          "The server listens on 127.0.0.1 only, requires the token, and refuses requests from web pages. Copied commands contain the token; create a new one if it leaks. Every agent action shows a notification on the dashboard."
        ),
      },
    ],
  },
  {
    id: "updates",
    icon: ArrowUpCircle,
    title: l("อัปเดตโปรแกรม", "Updating CHAMP"),
    summary: l("อัปเดตตัวเองจาก GitHub อย่างปลอดภัย", "Safe self-updates from GitHub"),
    blocks: [
      {
        kind: "list",
        items: [
          l(
            "CHAMP ตรวจ GitHub Releases วันละครั้งตอนเปิดโปรแกรม (ปิดได้ใน Settings → อัปเดต)",
            "CHAMP checks GitHub Releases once a day at startup (turn this off in Settings → Updates)."
          ),
          l(
            "เมื่อมีเวอร์ชันใหม่ แถบด้านบนจะขึ้น กด ติดตั้งและรีสตาร์ท บริการทั้งหมดจะหยุดระหว่างติดตั้ง",
            "When a release is available a banner appears. Choose Install and restart; all services stop during the install."
          ),
          l(
            "ตัวติดตั้งทุกไฟล์ต้องมีลายเซ็นจาก key ของ CHAMP ไฟล์ที่ถูกแก้ไขจะติดตั้งไม่ได้",
            "Every installer must be signed with CHAMP's release key; tampered files are refused."
          ),
          l(
            "กด ตรวจหาอัปเดต ใน Settings เพื่อตรวจทันที",
            "Use Check for updates in Settings to check right away."
          ),
        ],
      },
    ],
  },
  {
    id: "settings",
    icon: Settings2,
    title: l("การตั้งค่า", "Settings"),
    summary: l("พอร์ต เครื่องมือ และพฤติกรรมของแอป", "Ports, tools, and app behaviour"),
    blocks: [
      {
        kind: "list",
        items: [
          l(
            "พอร์ต: เปลี่ยนพอร์ตของแต่ละบริการ กด ตรวจสอบพอร์ต เพื่อดูว่าว่างไหม พอร์ตต้องไม่ซ้ำกัน",
            "Ports: change each service's port and use Check ports to see which are free. Ports must be unique."
          ),
          l(
            "เวอร์ชัน PHP และเครื่องมือฐานข้อมูล: ดูหัวข้อ PHP และ ฐานข้อมูล",
            "PHP versions and database tool: see the PHP and Databases topics."
          ),
          l(
            "Log Doctor AI, AI Agent Connector และ อัปเดต: ดูหัวข้อของแต่ละส่วน",
            "Log Doctor AI, AI agent connector, and Updates: see their topics."
          ),
          l("เสียงเอฟเฟกต์และการเริ่มบริการอัตโนมัติ", "Sound effects and auto-start."),
          l(
            "ภาษา: สลับ TH / EN ที่มุมขวาบนได้ทุกเมื่อ",
            "Language: switch TH / EN at the top right at any time."
          ),
        ],
      },
      {
        kind: "tip",
        text: l(
          "การเปลี่ยนในหน้า Settings จะมีผลเมื่อกด บันทึก ถ้า stack ทำงานอยู่ CHAMP จะรีสตาร์ทให้ด้วยพอร์ตใหม่",
          "Settings changes apply when you press Save. If the stack is running, CHAMP restarts it on the new ports."
        ),
      },
    ],
  },
  {
    id: "shortcuts",
    icon: Keyboard,
    title: l("คีย์ลัด", "Keyboard shortcuts"),
    summary: l("ทำงานได้ทุกภาษาของแป้นพิมพ์", "Work in any keyboard layout"),
    blocks: [
      {
        kind: "keys",
        items: [
          { keys: "Ctrl S", text: l("เริ่ม stack", "Start the stack") },
          { keys: "Ctrl R", text: l("รีสตาร์ท stack", "Restart the stack") },
          { keys: "Ctrl X", text: l("หยุด stack", "Stop the stack") },
          { keys: "Ctrl W", text: l("เปิดเว็บไซต์", "Open the website") },
          { keys: "Ctrl D", text: l("เปิด phpMyAdmin / Adminer", "Open phpMyAdmin / Adminer") },
          { keys: "Ctrl O", text: l("เปิดโฟลเดอร์ projects", "Open the projects folder") },
          { keys: "Ctrl L", text: l("เปิดโฟลเดอร์ logs", "Open the logs folder") },
          { keys: "Ctrl T", text: l("เปิด Terminal", "Open a terminal") },
          { keys: "Ctrl ,", text: l("เปิด/ปิด Settings", "Toggle Settings") },
          { keys: "F1", text: l("เปิดคู่มือนี้", "Open this guide") },
          { keys: "?", text: l("แสดงคีย์ลัด", "Show shortcuts") },
          { keys: "Esc", text: l("ปิดหน้าต่างหรือแจ้งเตือน", "Close a dialog or notification") },
        ],
      },
      {
        kind: "tip",
        text: l("บน macOS ใช้ ⌘ แทน Ctrl", "On macOS use ⌘ instead of Ctrl."),
      },
    ],
  },
  {
    id: "folders",
    icon: FolderTree,
    title: l("โฟลเดอร์ข้อมูลและโหมดพกพา", "Data folders and portable mode"),
    summary: l(
      "ไฟล์อยู่ที่ไหน และย้ายไปใช้ที่อื่นอย่างไร",
      "Where files live and how to move them"
    ),
    blocks: [
      {
        kind: "table",
        head: [l("โฟลเดอร์", "Folder"), l("เก็บอะไร", "Contents")],
        rows: [
          ["projects/", l("เว็บไซต์ของคุณ", "Your websites")],
          ["mysql/data/, postgresql/data/", l("ข้อมูลฐานข้อมูล", "Database files")],
          ["logs/", l("log ของทุกบริการ", "Service logs")],
          [
            "config/",
            l(
              "settings.json และไฟล์ตั้งค่าที่ CHAMP สร้างขึ้น",
              "settings.json and the configuration files CHAMP generates"
            ),
          ],
          ["runtime/", l("โปรแกรม Caddy, PHP, ฐานข้อมูล", "Caddy, PHP, and database binaries")],
          [
            "models/",
            l("โมเดล AI ของ Log Doctor (ถ้าติดตั้ง)", "Log Doctor AI model (when installed)"),
          ],
        ],
      },
      {
        kind: "p",
        text: l(
          "ค่าเริ่มต้นอยู่ที่ %LOCALAPPDATA%\\CHAMP (Windows) หรือโฟลเดอร์ข้อมูลผู้ใช้ของระบบ ดูที่อยู่จริงได้ที่แถบล่าง",
          "By default data lives in %LOCALAPPDATA%\\CHAMP (Windows) or the system's user data folder; the bottom bar shows the exact path."
        ),
      },
      {
        kind: "list",
        items: [
          l(
            "โหมดพกพา: วางไฟล์ชื่อ `champ-portable` ไว้ข้างตัวโปรแกรม ข้อมูลจะอยู่ในโฟลเดอร์ data ข้างกัน",
            "Portable mode: put a file named `champ-portable` next to the app; data then lives in a data folder beside it."
          ),
          l(
            "หรือกำหนดเองด้วยตัวแปร `CHAMP_DATA_DIR`",
            "Or choose a folder with the `CHAMP_DATA_DIR` environment variable."
          ),
          l(
            "ต้องการติดตั้งใหม่ทั้งหมด: กด Ctrl+Shift+D → Reset Installation (ลบเฉพาะ runtime ไม่ลบ projects และข้อมูลฐานข้อมูล)",
            "To reinstall from scratch: press Ctrl+Shift+D → Reset installation (removes runtime only; projects and database data stay)."
          ),
        ],
      },
    ],
  },
  {
    id: "troubleshooting",
    icon: LifeBuoy,
    title: l("แก้ปัญหาที่พบบ่อย", "Troubleshooting"),
    summary: l("อาการที่เจอบ่อยและวิธีแก้เร็ว", "Common symptoms and quick fixes"),
    blocks: [
      {
        kind: "table",
        head: [l("อาการ", "Symptom"), l("วิธีแก้", "Fix")],
        rows: [
          [
            l("บริการเป็นสีแดง", "A service is red"),
            l(
              "กด วิเคราะห์ บนการ์ดเพื่อให้ Log Doctor บอกสาเหตุ",
              "Press Diagnose on the card and follow Log Doctor"
            ),
          ],
          [
            l("พอร์ตถูกใช้", "Port already in use"),
            l(
              "เปลี่ยนพอร์ตใน Settings หรือปิด XAMPP/IIS/MySQL ที่ติดตั้งในระบบ",
              "Change the port in Settings or stop XAMPP / IIS / a system MySQL"
            ),
          ],
          [
            l("PHP ไม่ขึ้น โค้ด -1073741515", "PHP exits with -1073741515"),
            l(
              "ติดตั้ง Microsoft Visual C++ Redistributable x64",
              "Install the Microsoft Visual C++ Redistributable x64"
            ),
          ],
          [
            l("เว็บขึ้น 502 Bad Gateway", "502 Bad Gateway"),
            l(
              "PHP-FPM ไม่ทำงาน กด Start ที่การ์ด PHP-FPM",
              "PHP-FPM is not running; start it from its card"
            ),
          ],
          [
            l("MySQL ไม่ขึ้นหลังไฟดับ", "MySQL fails after a power cut"),
            l(
              "สำรองโฟลเดอร์ mysql/data ก่อน แล้วดูวิธีใน Log Doctor",
              "Back up mysql/data first, then follow Log Doctor"
            ),
          ],
          [
            l("macOS ไม่ยอมเปิดไฟล์", "macOS blocks a binary"),
            l(
              "รัน `xattr -dr com.apple.quarantine` กับโฟลเดอร์ runtime",
              "Run `xattr -dr com.apple.quarantine` on the runtime folder"
            ),
          ],
          [
            l("Linux ขาด library", "Missing Linux library"),
            l(
              "ติดตั้งตามคำสั่งที่ wizard หรือ Log Doctor แนะนำ",
              "Install what the wizard or Log Doctor lists"
            ),
          ],
        ],
      },
      {
        kind: "tip",
        text: l(
          "log ทุกไฟล์อยู่ในโฟลเดอร์ logs (Ctrl+L) ถ้าต้องการแจ้งปัญหา แนบไฟล์ log ที่เกี่ยวข้องไปที่ GitHub ของโปรเจกต์",
          "All logs are in the logs folder (Ctrl+L). When reporting a problem, attach the relevant log to an issue on the project's GitHub."
        ),
      },
    ],
  },
  {
    id: "security",
    icon: Lock,
    title: l("ความปลอดภัยและประสิทธิภาพ", "Security and performance"),
    summary: l(
      "CHAMP ปกป้องเครื่องคุณและใช้ทรัพยากรอย่างไร",
      "How CHAMP protects your machine and saves resources"
    ),
    blocks: [
      {
        kind: "list",
        items: [
          l(
            "ทุกบริการฟังเฉพาะในเครื่อง (127.0.0.1) ยกเว้นตอนเปิด HTTPS Preview เอง",
            "Every service listens locally (127.0.0.1) unless you start HTTPS Preview yourself."
          ),
          l(
            "แอปเปิดได้เฉพาะโฟลเดอร์ใน CHAMP และลิงก์ http(s) เท่านั้น",
            "The app can only open folders inside CHAMP and http(s) links."
          ),
          l(
            "อัปเดตต้องมีลายเซ็น และ PHP บน Windows ถูกตรวจ SHA-256",
            "Updates must be signed, and PHP downloads on Windows are checked against SHA-256."
          ),
          l("ไม่มีการเก็บสถิติหรือส่งข้อมูลการใช้งาน", "No telemetry or usage data is collected."),
          l(
            "ตัวเลข CPU/RAM อัปเดตทุก 2.5 วินาทีและหยุดเมื่อซ่อนหน้าต่าง บริการทำงานเป็นโปรเซสแยกและหยุดได้ทันที",
            "CPU/RAM stats refresh every 2.5 seconds and pause when the window is hidden. Services run as separate processes you can stop at any time."
          ),
        ],
      },
    ],
  },
];
